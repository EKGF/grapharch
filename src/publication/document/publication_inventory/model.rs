//! Bounded publication inventory shared by document generation and host
//! routing.
use {
    crate::publication::{dataset::Dataset, term, view::RevisionView},
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
    serde::{Deserialize, Serialize},
};

pub(crate) const DCT: &str = "http://purl.org/dc/terms/";
pub(crate) const DOAP: &str = "http://usefulinc.com/ns/doap#";
pub(crate) const DCAT: &str = "http://www.w3.org/ns/dcat#";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Inventory {
    pub observed_at: String,
    // bounded: explicit catalogue limited to 1000 publication records.
    pub versions:    Vec<Version>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Version {
    pub slug:       String,
    pub branch:     String,
    pub label:      String,
    pub url:        String,
    pub advertised: bool,
}

impl Version {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.slug.is_empty() &&
                self.slug.len() <= 128 &&
                self.slug != "." &&
                self.slug != ".." &&
                self.slug.bytes().all(|c| {
                    c.is_ascii_alphanumeric() || b"-_.".contains(&c)
                }),
            "invalid publication slug"
        );
        ensure!(
            !self.branch.is_empty() &&
                self.branch.len() <= 256 &&
                !self.branch.chars().any(char::is_control),
            "invalid publication branch"
        );
        ensure!(
            !self.label.trim().is_empty() &&
                self.label.len() <= 512 &&
                !self.label.chars().any(char::is_control),
            "invalid publication label"
        );
        let url = url::Url::parse(&self.url)?;
        ensure!(
            matches!(url.scheme(), "http" | "https") &&
                url.host_str().is_some() &&
                url.username().is_empty() &&
                url.password().is_none() &&
                url.query().is_none() &&
                url.fragment().is_none() &&
                self.url.ends_with('/'),
            "publication URL must be a credential-free HTTP(S) directory URL"
        );
        Ok(())
    }
}

#[derive(Default)]
struct Record {
    id:         Option<NamedNode>,
    kind:       bool,
    slug:       Option<String>,
    branch:     Option<String>,
    label:      Option<String>,
    url:        Option<String>,
    advertised: Option<bool>,
}
impl Record {
    fn finish(self) -> Result<Version> {
        ensure!(
            self.kind,
            "publication inventory member must be a doap:Version"
        );
        let required = |value: Option<String>, field| {
            value.ok_or_else(|| {
                anyhow::anyhow!("publication inventory member missing {field}")
            })
        };
        let value = Version {
            slug:       required(self.slug, "identifier")?,
            branch:     required(self.branch, "branch")?,
            label:      required(self.label, "title")?,
            url:        required(self.url, "accessURL")?,
            advertised: self.advertised.ok_or_else(|| {
                anyhow::anyhow!(
                    "publication inventory member missing advertised"
                )
            })?,
        };
        value.validate()?;
        Ok(value)
    }

    fn field(&mut self, predicate: &NamedNode, object: &Term) -> Result<()> {
        if predicate.as_str() == crate::publication::RDF_TYPE {
            self.kind |=
                object == &NamedNode::new(format!("{DOAP}Version"))?.into();
            return Ok(());
        }
        if predicate == &term("advertised") {
            let Term::Literal(value) = object else {
                anyhow::bail!("advertised must be a boolean literal")
            };
            ensure!(
                value.datatype().as_str() ==
                    "http://www.w3.org/2001/XMLSchema#boolean",
                "advertised must be typed boolean"
            );
            let value = match value.value() {
                "true" | "1" => true,
                "false" | "0" => false,
                _ => anyhow::bail!("invalid advertised boolean"),
            };
            ensure!(
                self.advertised.is_none_or(|old| old == value),
                "conflicting advertised values"
            );
            self.advertised = Some(value);
            return Ok(());
        }
        let (slot, value) = if predicate.as_str() == format!("{DCAT}accessURL")
        {
            let Term::NamedNode(value) = object else {
                anyhow::bail!("accessURL must be an IRI")
            };
            (&mut self.url, value.as_str().to_owned())
        } else {
            let Term::Literal(value) = object else {
                anyhow::bail!("inventory metadata must be a literal")
            };
            let slot = match predicate.as_str() {
                p if p == format!("{DCT}identifier") => &mut self.slug,
                p if p == format!("{DCT}title") => &mut self.label,
                p if p == format!("{DOAP}branch") => &mut self.branch,
                _ => anyhow::bail!("unexpected inventory predicate"),
            };
            (slot, value.value().to_owned())
        };
        ensure!(
            slot.as_ref().is_none_or(|old| old == &value),
            "conflicting publication inventory metadata"
        );
        *slot = Some(value);
        Ok(())
    }
}

impl Inventory {
    pub async fn read(
        dataset: &Dataset,
        view: &RevisionView,
        subject: &NamedNode,
    ) -> Result<Self> {
        let Some(Term::Literal(observed_at)) = view
            .optional_by(
                subject,
                &NamedNode::new(format!("{DCT}modified"))?,
            )
            .await?
        else {
            anyhow::bail!("publication inventory requires observation time")
        };
        chrono::DateTime::parse_from_rfc3339(observed_at.value())?;
        let members =
            view.pattern(&format!("{subject} <{DCT}hasPart> ?version"), 0);
        let fields = view.pattern("?version ?predicate ?object", 1);
        let mut rows = dataset
            .select(format!(
                "SELECT DISTINCT ?version ?predicate ?object WHERE {{ \
                 {members} OPTIONAL {{ {fields} FILTER(?predicate \
                 IN(<{DCT}identifier>,<{DCT}title>,<{DOAP}branch>,\
                 <{DCAT}accessURL>,{},<{}>)) }} }} ORDER BY ?version \
                 ?predicate ?object",
                term("advertised"),
                crate::publication::RDF_TYPE
            ))
            .await?;
        let mut versions = Vec::new(); // bounded: maximum 1000 versions below.
        let mut record = Record::default();
        let mut facts = 0;
        while let Some(row) = rows.next().await? {
            facts += 1;
            ensure!(
                facts <= 16000,
                "publication inventory exceeds fact budget"
            );
            let Term::NamedNode(id) = &row["version"] else {
                anyhow::bail!("publication version must be identified")
            };
            if record.id.as_ref().is_some_and(|current| current != id) {
                ensure!(
                    versions.len() < 1000,
                    "publication inventory exceeds 1000 versions"
                );
                versions.push(std::mem::take(&mut record).finish()?);
            }
            record.id = Some(id.clone());
            if let Some(Term::NamedNode(predicate)) = row.get("predicate") {
                record.field(predicate, &row["object"])?;
            }
        }
        if record.id.is_some() {
            ensure!(
                versions.len() < 1000,
                "publication inventory exceeds 1000 versions"
            );
            versions.push(record.finish()?);
        }
        ensure!(
            !versions.is_empty(),
            "publication inventory cannot be empty"
        );
        versions.sort_by(|a, b| a.branch.cmp(&b.branch));
        let mut slugs = std::collections::HashSet::new(); // bounded: the capped versions above.
        ensure!(
            versions.iter().all(|v| slugs.insert(v.slug.clone())),
            "publication slugs collide"
        );
        Ok(Self { observed_at: observed_at.value().into(), versions })
    }
}

pub(crate) struct Job {
    pub id:      NamedNode,
    pub subject: NamedNode,
    pub page:    NamedNode,
    pub title:   String,
    pub route:   String,
    pub path:    String,
    pub order:   u32,
}

pub(crate) async fn configured_job(
    dataset: &Dataset,
    view: &RevisionView,
    project: &NamedNode,
) -> Result<Option<Job>> {
    let pattern = view.pattern(
        &format!(
            "?job a {}; {} {project}; {} \"publication-inventory\"",
            term("Documentor"),
            term("project"),
            term("kind")
        ),
        0,
    );
    let mut rows = dataset
        .select(format!(
            "SELECT DISTINCT ?job WHERE {{ {pattern} }}"
        ))
        .await?;
    let Some(row) = rows.next().await? else {
        return Ok(None)
    };
    ensure!(
        rows.next().await?.is_none(),
        "website supports one publication inventory"
    );
    let Term::NamedNode(id) = &row["job"] else {
        anyhow::bail!("inventory documentor must be identified")
    };
    let title = view
        .optional_by(id, &NamedNode::new(format!("{DCT}title"))?)
        .await?;
    let Some(Term::Literal(title)) = title else {
        anyhow::bail!("inventory page requires title")
    };
    let order =
        view.optional(id, "navigationOrder").await?.ok_or_else(|| {
            anyhow::anyhow!("inventory page requires navigationOrder")
        })?;
    Ok(Some(Job {
        id:      id.clone(),
        subject: view.node(id, "subject").await?,
        page:    view.node(id, "fragment").await?,
        title:   title.value().into(),
        route:   view.text(id, "route").await?,
        path:    view.text(id, "sourcePath").await?,
        order:   crate::publication::validation::unsigned(
            &order,
            "inventory navigation order",
        )?,
    }))
}
