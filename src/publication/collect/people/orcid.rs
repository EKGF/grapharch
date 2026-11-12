//! Public ORCID person records for ORCID iDs used as values of configured
//! person predicates.
use {
    super::{DCT, PROV, SCHEMA, api, display_text, quad},
    crate::publication::{
        collect::{Collector, SourceIdentity},
        dataset::Dataset,
        orcid::OrcidId,
        term,
        view::RevisionView,
        writer::ContributionStream,
    },
    anyhow::{Context, Result, bail, ensure},
    async_trait::async_trait,
    oxrdf::{Literal, NamedNode, Term},
    serde::Deserialize,
    sha2::{Digest, Sha256},
    std::{collections::BTreeMap, sync::Arc},
};

/// At most this many distinct people are fetched for one collector use.
const MAX_PEOPLE: usize = 64;
/// A person record is a few kilobytes; larger responses are rejected.
const MAX_RESPONSE: usize = 256 * 1024;
const MAX_PREDICATES: usize = 16;
const MAX_LINKED_ACCOUNTS: usize = 64;

pub(super) async fn collectors(
    dataset: &Dataset,
    view: &RevisionView,
    project: &str,
    processor: &NamedNode,
    endpoint: api::Endpoint,
) -> Result<Vec<Arc<dyn Collector>>> {
    let mut predicates = Vec::new(); // bounded: 16 configured predicates.
    let mut rows = view.objects(processor, "personPredicate").await?;
    while let Some(row) = rows.next().await? {
        let Term::NamedNode(predicate) = &row["value"] else {
            bail!("personPredicate must be a property IRI")
        };
        ensure!(
            predicates.len() < MAX_PREDICATES,
            "ORCID person collector exceeds {MAX_PREDICATES} person predicates"
        );
        predicates.push(predicate.to_string());
    }
    ensure!(
        !predicates.is_empty(),
        "ORCID person collector {processor} requires at least one \
         ga:personPredicate"
    );
    predicates.sort();
    let pattern = view.pattern("?subject ?predicate ?person", 0);
    let mut rows = dataset
        .select(format!(
            "SELECT DISTINCT ?person WHERE {{ VALUES ?predicate {{ {} }} \
             {pattern} FILTER(isIRI(?person) && \
             (STRSTARTS(STR(?person), \"http://{host}/\") || \
             STRSTARTS(STR(?person), \"https://{host}/\"))) }} ORDER BY \
             ?person LIMIT {}",
            predicates.join(" "),
            4 * MAX_PEOPLE + 1,
            host = OrcidId::HOST,
        ))
        .await?;
    // Invalid iDs remain distinct inputs, so their collectors record the
    // checksum failure against the offending reference.
    let mut people = BTreeMap::new(); // bounded: MAX_PEOPLE distinct people.
    let mut references = 0;
    while let Some(row) = rows.next().await? {
        references += 1;
        ensure!(
            references <= 4 * MAX_PEOPLE,
            "ORCID person discovery exceeds {} references",
            4 * MAX_PEOPLE
        );
        let Term::NamedNode(reference) = &row["person"] else {
            bail!("ORCID person reference must be an IRI")
        };
        let key = OrcidId::from_iri(reference.as_str())
            .map(|id| id.canonical().into_string())
            .unwrap_or_else(|_| reference.as_str().to_owned());
        people
            .entry(key)
            .or_insert_with(|| reference.as_str().to_owned());
        ensure!(
            people.len() <= MAX_PEOPLE,
            "ORCID person collection exceeds {MAX_PEOPLE} people"
        );
    }
    let mut work: Vec<Arc<dyn Collector>> = Vec::new();
    for reference in people.into_values() {
        let source = match OrcidId::from_iri(&reference) {
            Ok(id) => endpoint.url(&format!("{id}/person"))?.to_string(),
            Err(_) => reference.clone(),
        };
        work.push(Arc::new(OrcidPerson {
            project: project.to_owned(),
            processor: processor.as_str().to_owned(),
            endpoint: endpoint.clone(),
            reference,
            source,
        }));
    }
    Ok(work)
}

struct OrcidPerson {
    project:   String,
    processor: String,
    endpoint:  api::Endpoint,
    reference: String,
    source:    String,
}

#[derive(Deserialize)]
struct Record {
    name:            Option<Name>,
    #[serde(rename = "researcher-urls")]
    researcher_urls: Option<ResearcherUrls>,
}
#[derive(Deserialize)]
struct Name {
    #[serde(rename = "given-names")]
    given:    Option<Value<String>>,
    #[serde(rename = "family-name")]
    family:   Option<Value<String>>,
    #[serde(rename = "credit-name")]
    credit:   Option<Value<String>>,
    #[serde(rename = "last-modified-date")]
    modified: Option<Value<i64>>,
    path:     Option<String>,
}
#[derive(Deserialize)]
struct Value<T> {
    value: T,
}
#[derive(Deserialize)]
struct ResearcherUrls {
    #[serde(rename = "researcher-url", default)]
    urls: Vec<ResearcherUrl>,
}
#[derive(Deserialize)]
struct ResearcherUrl {
    url: Option<Value<String>>,
}

/// The public name and explicitly listed GitHub accounts of one ORCID record.
struct PublicPerson {
    title:    String,
    given:    Option<String>,
    family:   Option<String>,
    modified: Option<String>,
    accounts: Vec<NamedNode>,
}

impl PublicPerson {
    fn read(id: &OrcidId, body: &[u8]) -> Result<Self> {
        let record: Record = serde_json::from_slice(body)
            .with_context(|| format!("ORCID person record {id}"))?;
        let Some(name) = record.name else {
            bail!("ORCID record {id} has no public name")
        };
        if let Some(path) = &name.path {
            ensure!(
                path == id.as_str(),
                "ORCID response describes {path}, not the requested {id}"
            );
        }
        let text = |value: Option<Value<String>>, field: &str| {
            value
                .map(|value| display_text(&value.value, field))
                .transpose()
                .map(Option::flatten)
        };
        let given = text(name.given, "ORCID given names")?;
        let family = text(name.family, "ORCID family name")?;
        let credit = text(name.credit, "ORCID credit name")?;
        let title = match (&credit, &given, &family) {
            (Some(credit), ..) => credit.clone(),
            (None, Some(given), Some(family)) => format!("{given} {family}"),
            (None, Some(name), None) | (None, None, Some(name)) => name.clone(),
            (None, None, None) => {
                bail!("ORCID record {id} has no public name")
            },
        };
        let modified = name
            .modified
            .map(|value| {
                chrono::DateTime::from_timestamp_millis(value.value)
                    .map(|time| {
                        time.to_rfc3339_opts(
                            chrono::SecondsFormat::Millis,
                            true,
                        )
                    })
                    .context("ORCID name modification time is out of range")
            })
            .transpose()?;
        let mut accounts = Vec::new(); // bounded: MAX_LINKED_ACCOUNTS URLs.
        for url in record
            .researcher_urls
            .map(|urls| urls.urls)
            .unwrap_or_default()
        {
            ensure!(
                accounts.len() < MAX_LINKED_ACCOUNTS,
                "ORCID record {id} exceeds {MAX_LINKED_ACCOUNTS} linked \
                 GitHub accounts"
            );
            if let Some(account) =
                url.url.and_then(|url| github_profile(&url.value))
            {
                accounts.push(account);
            }
        }
        accounts.sort();
        accounts.dedup();
        Ok(Self { title, given, family, modified, accounts })
    }
}

/// The canonical profile IRI of a GitHub account URL listed on an ORCID
/// record. Other researcher URLs are not retained.
pub(super) fn github_profile(url: &str) -> Option<NamedNode> {
    let url = url::Url::parse(url.trim()).ok()?;
    if !matches!(url.scheme(), "http" | "https") ||
        !matches!(
            url.host_str(),
            Some("github.com" | "www.github.com")
        ) ||
        url.query().is_some() ||
        url.fragment().is_some()
    {
        return None;
    }
    let login = url.path().trim_matches('/');
    super::github::Login::try_new(login)
        .ok()
        .map(|login| login.profile())
}

#[async_trait]
impl Collector for OrcidPerson {
    fn identity(&self) -> Result<SourceIdentity<'_>> {
        Ok(SourceIdentity {
            project:   &self.project,
            processor: &self.processor,
            source:    &self.source,
        })
    }

    async fn produce(&self, output: &mut ContributionStream) -> Result<String> {
        let id = OrcidId::from_iri(&self.reference)?;
        let url = self.endpoint.url(&format!("{id}/person"))?;
        let body = api::get(
            api::Endpoint::client()?
                .get(url.clone())
                .header("Accept", "application/json"),
            MAX_RESPONSE,
        )
        .await?;
        let person = PublicPerson::read(&id, &body)?;
        let subject = id.canonical();
        let mut assertions = vec![
            quad(
                &subject,
                crate::publication::RDF_TYPE,
                NamedNode::new_unchecked(format!("{SCHEMA}Person")),
            ),
            quad(
                &subject,
                &format!("{DCT}title"),
                Literal::new_simple_literal(&person.title),
            ),
            quad(
                &subject,
                &format!("{SCHEMA}name"),
                Literal::new_simple_literal(&person.title),
            ),
            quad(&subject, term("orcid").as_str(), subject.clone()),
            quad(
                &subject,
                term("url").as_str(),
                Literal::new_simple_literal(subject.as_str()),
            ),
            quad(
                &subject,
                &format!("{PROV}wasDerivedFrom"),
                NamedNode::new(url.as_str())?,
            ),
        ]; // bounded: fixed fields plus MAX_LINKED_ACCOUNTS links.
        for (predicate, value) in
            [("givenName", &person.given), ("familyName", &person.family)]
        {
            if let Some(value) = value {
                assertions.push(quad(
                    &subject,
                    &format!("{SCHEMA}{predicate}"),
                    Literal::new_simple_literal(value),
                ));
            }
        }
        if let Some(modified) = &person.modified {
            assertions.push(quad(
                &subject,
                &format!("{DCT}modified"),
                Literal::new_typed_literal(
                    modified,
                    NamedNode::new_unchecked(
                        "http://www.w3.org/2001/XMLSchema#dateTime",
                    ),
                ),
            ));
        }
        for account in &person.accounts {
            assertions.push(quad(
                &subject,
                &format!("{SCHEMA}sameAs"),
                account.clone(),
            ));
        }
        for assertion in assertions {
            output.write(assertion).await?;
        }
        Ok(format!(
            "orcid-person-v1:{id}:{}",
            hex::encode(Sha256::digest(&body))
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{OrcidId, PublicPerson, github_profile};

    #[test]
    fn public_names_prefer_the_credit_name_and_keep_only_github_links() {
        let id = OrcidId::try_new("0000-0002-1825-0097").unwrap();
        let person = PublicPerson::read(
            &id,
            br#"{"name":{"given-names":{"value":"Josiah"},"family-name":{"value":"Carberry"},"credit-name":{"value":"J. S. Carberry"},"path":"0000-0002-1825-0097","last-modified-date":{"value":1478897088336}},
               "researcher-urls":{"researcher-url":[{"url":{"value":"https://example.org/"}},{"url":{"value":"https://github.com/JCarberry/"}}]},
               "emails":{"email":[{"email":"private@example.org"}]}}"#,
        )
        .unwrap();
        assert_eq!(person.title, "J. S. Carberry");
        assert_eq!(person.family.as_deref(), Some("Carberry"));
        assert_eq!(
            person.modified.as_deref(),
            Some("2016-11-11T20:44:48.336Z")
        );
        assert_eq!(
            person
                .accounts
                .iter()
                .map(|account| account.as_str())
                .collect::<Vec<_>>(),
            ["https://github.com/jcarberry"]
        );
        let unnamed = PublicPerson::read(&id, br#"{"name":null}"#);
        assert!(unnamed.is_err());
        let mismatched = PublicPerson::read(
            &id,
            br#"{"name":{"given-names":{"value":"A"},"path":"0000-0003-2309-5068"}}"#,
        );
        assert!(mismatched.is_err());
        assert!(github_profile("https://github.com/org/repository").is_none());
        assert!(github_profile("https://gitlab.com/someone").is_none());
    }
}
