//! ORCID and GitHub person collectors against a local mock API; no network.
#[path = "support/family_repository.rs"]
#[allow(dead_code)]
mod family_repository;

use {
    anyhow::Result,
    grapharch::publication::{
        dataset::Dataset,
        ontology_family::{self, FamilySource},
        render::{PublishOutcome, Publisher},
    },
    std::{
        collections::BTreeMap,
        path::Path,
        sync::{Arc, Mutex},
    },
    tokio::io::{AsyncReadExt, AsyncWriteExt},
};

type Requests = Arc<Mutex<Vec<String>>>;

/// Serve fixed JSON bodies by request target; anything else is a 404.
async fn mock(routes: BTreeMap<String, String>) -> Result<(String, Requests)> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let base = format!("http://{}/", listener.local_addr()?);
    let requests: Requests = Arc::default();
    let seen = requests.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return
            };
            let routes = routes.clone();
            let seen = seen.clone();
            tokio::spawn(async move {
                let mut head = Vec::new();
                let mut byte = [0];
                while !head.ends_with(b"\r\n\r\n") && head.len() < 8192 {
                    if stream.read_exact(&mut byte).await.is_err() {
                        return;
                    }
                    head.push(byte[0]);
                }
                let head = String::from_utf8_lossy(&head);
                let target = head
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or_default()
                    .to_owned();
                seen.lock().unwrap().push(target.clone());
                let (status, body) = match routes.get(&target) {
                    Some(body) => ("200 OK", body.clone()),
                    None => ("404 Not Found", "{}".to_owned()),
                };
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: \
                     application/json\r\nContent-Length: {}\r\nConnection: \
                     close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes()).await;
            });
        }
    });
    Ok((base, requests))
}

const ORCID: &str = "0000-0003-2309-5068";

fn definitions(base: &str) -> String {
    let project = include_str!("../examples/ekg-ontologies/project.ttl");
    format!(
        "{project}
@prefix dc: <http://purl.org/dc/elements/1.1/> .
ex:project ga:collector \
         ex:orcid, ex:github .
ex:orcid a ga:CollectorUse ; dct:title \"ORCID people\" \
         ;
    ga:collectorType ga:OrcidPersonCollector ;
    ga:source \"{base}orcid/\" \
         ;
    ga:personPredicate dc:creator, dct:creator, dct:contributor .
ex:github \
         a ga:CollectorUse ; dct:title \"GitHub contributors\" ;
    ga:collectorType \
         ga:GitHubContributorCollector ;
    ga:source \"{base}github/\" .
ex:specification \
         ga:credit ex:editors, ex:contributors .
ex:editors a ga:Credit ; ga:creditRole \
         ga:editor ; ga:creditPredicate dc:creator .
ex:contributors a ga:Credit \
         ; ga:creditRole ga:contributor ;
    ga:creditPredicate <https://schema.org/contributor> \
         .
"
    )
}

fn routes(commit: &str) -> BTreeMap<String, String> {
    BTreeMap::from([
        (
            format!("/orcid/{ORCID}/person"),
            format!(
                r#"{{"name":{{"given-names":{{"value":"Jacobus"}},"family-name":{{"value":"Geluk"}},"credit-name":null,"path":"{ORCID}","last-modified-date":{{"value":1478897088336}}}},
                "researcher-urls":{{"researcher-url":[{{"url":{{"value":"https://github.com/JGeluk"}}}}]}},
                "emails":{{"email":[{{"email":"hidden@example.org"}}]}}}}"#
            ),
        ),
        (
            format!(
                "/github/repos/EKGF/ekg-ontologies/commits?sha={commit}&per_page=100&page=1"
            ),
            r#"[{"author":{"login":"JGeluk","type":"User"}},{"author":{"login":"zoe-x","type":"User"}},
               {"author":{"login":"JGeluk","type":"User"}},{"author":{"login":"alice","type":"User"}},
               {"author":{"login":"alice","type":"User"}},{"author":{"login":"alice","type":"User"}},
               {"author":{"login":"dependabot[bot]","type":"Bot"}},{"author":null}]"#
                .into(),
        ),
        (
            "/github/users/alice".into(),
            r#"{"login":"alice","name":"Alice Example","type":"User","email":"alice@example.org"}"#.into(),
        ),
        (
            "/github/users/JGeluk".into(),
            r#"{"login":"JGeluk","name":"Jacobus Geluk","type":"User"}"#.into(),
        ),
        (
            "/github/users/zoe-x".into(),
            r#"{"login":"zoe-x","name":null,"type":"User"}"#.into(),
        ),
    ])
}

/// A Git checkout of the EKGF fixture whose origin is its GitHub repository.
fn checkout(
    root: &Path,
    extra: Option<&str>,
) -> Result<(std::path::PathBuf, String)> {
    let repository = root.join("ekg-ontologies");
    family_repository::copy_tree(
        Path::new("examples/ekg-ontologies/sources"),
        &repository,
    )?;
    if let Some(extra) = extra {
        std::fs::write(repository.join("extra.ttl"), extra)?;
    }
    // One reference uses the https spelling of the http IRIs in the sources.
    std::fs::write(
        repository.join("people.ttl"),
        format!(
            "<https://ekgf.org/ontology/use-case#UseCase> \
             <http://purl.org/dc/terms/creator> <https://orcid.org/{ORCID}> ."
        ),
    )?;
    family_repository::git(&repository, &["init", "-b", "main"])?;
    family_repository::git(&repository, &["add", "--all"])?;
    family_repository::git(&repository, &["commit", "-m", "fixture"])?;
    family_repository::git(&repository, &[
        "remote",
        "add",
        "origin",
        "https://github.com/EKGF/ekg-ontologies.git",
    ])?;
    let commit = family_repository::git(&repository, &["rev-parse", "HEAD"])?;
    Ok((repository, commit))
}

fn people(html: &str, heading: &str) -> Vec<(String, Option<String>)> {
    let doc = scraper::Html::parse_document(html);
    let terms = scraper::Selector::parse("header dt").unwrap();
    let link = scraper::Selector::parse("a").unwrap();
    let Some(term) = doc
        .select(&terms)
        .find(|term| term.text().collect::<String>() == heading)
    else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for node in term.next_siblings() {
        let Some(element) = scraper::ElementRef::wrap(node) else {
            continue
        };
        match element.value().name() {
            "dt" => break,
            "dd" => {
                names.push((
                    element.text().collect::<String>().trim().to_owned(),
                    element.select(&link).next().and_then(|a| {
                        a.value().attr("href").map(str::to_owned)
                    }),
                ))
            },
            _ => {},
        }
    }
    names
}

#[tokio::test]
async fn orcid_editors_and_github_contributors_render_from_a_restored_kg()
-> Result<()> {
    let root = tempfile::tempdir()?;
    let (repository, commit) = checkout(root.path(), None)?;
    let (base, requests) = mock(routes(&commit)).await?;
    let project = root.path().join("project.ttl");
    std::fs::write(&project, definitions(&base))?;
    let built = ontology_family::collect_repository(
        &repository,
        &project,
        "urn:ekgf-family:project",
        "urn:ekgf-family:specification",
        4,
    )
    .await?;
    let requests = requests.lock().unwrap().clone();
    assert_eq!(
        requests
            .iter()
            .filter(|target| target.starts_with("/orcid/"))
            .collect::<Vec<_>>(),
        [&format!("/orcid/{ORCID}/person")],
        "http and https spellings are one person, fetched once"
    );
    let archive = root.path().join("documentation.nq");
    built.dataset.export(archive.clone()).await?;
    let mut emails = built
        .dataset
        .select(
            "SELECT ?s WHERE { GRAPH ?g { ?s ?p ?o FILTER(CONTAINS(STR(?o), \
             \"hidden@\") || CONTAINS(STR(?o), \"alice@\")) } } LIMIT 1"
                .into(),
        )
        .await?;
    assert!(
        emails.next().await?.is_none(),
        "no API-supplied email is retained"
    );
    std::fs::remove_dir_all(&repository)?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let output = tempfile::tempdir()?;
    let PublishOutcome::Published(publication) = Publisher::new(restored)
        .with_edition(grapharch::publication::pdf_name::Edition::Default)
        .render(&built.plan, output.path())
        .await?
    else {
        panic!("expected family publication")
    };
    let html =
        std::fs::read_to_string(publication.directory.join("index.html"))?;
    assert_eq!(people(&html, "Editor:"), [(
        "Jacobus Geluk".to_owned(),
        Some(format!("https://orcid.org/{ORCID}"))
    )]);
    // JGeluk is linked from the ORCID record, so the person is listed once.
    assert_eq!(people(&html, "Contributors:"), [
        (
            "Alice Example".to_owned(),
            Some("https://github.com/alice".to_owned())
        ),
        (
            "zoe-x".to_owned(),
            Some("https://github.com/zoe-x".to_owned())
        ),
    ]);
    let typst = std::fs::read_to_string(publication.typst_source())?;
    for text in ["Jacobus Geluk", "Alice Example", "https://orcid.org/"] {
        assert!(typst.contains(text), "PDF source misses {text}");
    }
    Ok(())
}

#[tokio::test]
async fn invalid_orcid_checksums_fail_with_a_kg_diagnostic() -> Result<()> {
    let root = tempfile::tempdir()?;
    let (repository, commit) = checkout(
        root.path(),
        Some(
            "<https://ekgf.org/ontology/use-case#UseCase> \
             <http://purl.org/dc/elements/1.1/creator> \
             <http://orcid.org/0000-0003-2309-5069> .",
        ),
    )?;
    let (base, requests) = mock(routes(&commit)).await?;
    let project = root.path().join("project.ttl");
    std::fs::write(&project, definitions(&base))?;
    let dataset = Dataset::memory()?;
    let error = match ontology_family::collect_from(
        dataset.clone(),
        FamilySource::LocalRepository(&repository),
        &project,
        "urn:ekgf-family:project",
        "urn:ekgf-family:specification",
        4,
    )
    .await
    {
        Ok(_) => panic!("an invalid ORCID iD must fail the build"),
        Err(error) => format!("{error:#}"),
    };
    assert!(error.contains("checksum"), "{error}");
    let mut rows = dataset
        .select(
            "PREFIX ga:<https://ekgf.org/ontology/grapharch#> SELECT ?m WHERE \
             { GRAPH ?g { ?d a ga:Diagnostic ; ga:code \"collection-failed\" \
             ; ?p ?m FILTER(isLiteral(?m) && CONTAINS(STR(?m), \"checksum\")) \
             } } LIMIT 1"
                .into(),
        )
        .await?;
    assert!(
        rows.next().await?.is_some(),
        "diagnostic is recorded in the KG"
    );
    assert!(
        !requests
            .lock()
            .unwrap()
            .iter()
            .any(|target| target.contains("5069")),
        "an invalid iD is never requested"
    );
    Ok(())
}

#[tokio::test]
async fn failed_person_requests_fail_the_build() -> Result<()> {
    let root = tempfile::tempdir()?;
    let (repository, _) = checkout(root.path(), None)?;
    let (base, _) = mock(BTreeMap::new()).await?;
    let project = root.path().join("project.ttl");
    std::fs::write(&project, definitions(&base))?;
    let result = ontology_family::collect_repository(
        &repository,
        &project,
        "urn:ekgf-family:project",
        "urn:ekgf-family:specification",
        4,
    )
    .await;
    let Err(error) = result else {
        panic!("a 404 must not become no people")
    };
    assert!(format!("{error:#}").contains("404"), "{error:#}");
    Ok(())
}

#[tokio::test]
#[ignore = "contacts pub.orcid.org"]
async fn live_orcid_public_api_returns_the_ekgf_creator() -> Result<()> {
    let body = reqwest::Client::new()
        .get(format!(
            "https://pub.orcid.org/v3.0/{ORCID}/person"
        ))
        .header("Accept", "application/json")
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    assert!(body.contains("\"family-name\""));
    Ok(())
}
