//! Each retained SHACL query is captioned by its own constraint's text.
use {
    anyhow::Result,
    grapharch::publication::{
        collect::{Collector, RdfFile},
        dataset::Dataset,
        document::{AnnotatedTermDocumentor, Documentor},
        view::RevisionView,
        writer::GraphWriter,
    },
    oxrdf::NamedNode,
    std::{
        collections::{BTreeMap, HashSet},
        sync::Arc,
    },
};

const DPROD_SHAPE: &str =
    "https://ekgf.org/dprod/spec/develop/contracts/shapes/ScheduleShape";

const CAPTION_SHAPES: &str = r#"
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
<urn:captions> a sh:NodeShape ;
  rdfs:label "caption shape" ;
  sh:target [ a sh:SPARQLTarget ;
    rdfs:comment "Targets every subject with a status." ;
    sh:select "SELECT ?this WHERE { ?this <urn:status> ?s }" ] ;
  sh:sparql [
    sh:message "Bericht in het Nederlands."@nl, "Message in English."@en ;
    sh:select "SELECT $this WHERE { $this <urn:a> ?a }" ] ;
  sh:sparql [
    sh:description "Described constraint." ;
    rdfs:comment "Ignored comment." ;
    sh:select "SELECT $this WHERE { $this <urn:b> ?b }" ] ;
  sh:sparql [
    rdfs:comment "Commented constraint." ;
    sh:select "SELECT $this WHERE { $this <urn:c> ?c }" ] ;
  sh:sparql [ sh:select "SELECT $this WHERE { $this <urn:d> ?d }" ] .
<urn:validator> a sh:SPARQLAskValidator ;
  rdfs:label "ask validator" ;
  sh:message "Validator message."@en ;
  sh:ask "ASK { $this ?p ?o }" .
"#;

struct Fixture {
    dataset: Dataset,
    writer:  Option<GraphWriter>,
    worker:  tokio::task::JoinHandle<Result<()>>,
    _source: tempfile::TempDir,
}

impl Fixture {
    async fn new() -> Result<Self> {
        let source = tempfile::tempdir()?;
        let dataset = Dataset::memory()?;
        let (writer, worker) =
            GraphWriter::spawn(dataset.clone(), 8, 1024 * 1024)?;
        for (name, text) in [
            (
                "schedule.ttl",
                include_str!("fixtures/shacl-sparql/dprod-schedule.ttl"),
            ),
            ("captions.ttl", CAPTION_SHAPES),
        ] {
            let path = source.path().join(name);
            tokio::fs::write(&path, text).await?;
            RdfFile::new("urn:p", &format!("urn:{name}"), path)?
                .collect(&writer)
                .await?;
        }
        Ok(Self {
            dataset,
            writer: Some(writer),
            worker,
            _source: source,
        })
    }

    async fn body(&self, subject: &str, language: &str) -> Result<String> {
        let writer = self.writer.as_ref().expect("open writer");
        let documentor = AnnotatedTermDocumentor::new(
            self.dataset.clone(),
            self.dataset.snapshot_for("urn:p").await?,
            "urn:p",
            NamedNode::new(subject)?,
            language,
            Arc::new(BTreeMap::new()),
            Arc::new(HashSet::new()),
        )?
        .with_source_annotations();
        documentor.document(writer).await?;
        let snapshot = self.dataset.snapshot_for("urn:p").await?;
        RevisionView::new(self.dataset.clone(), snapshot)
            .text(&documentor.fragment_id(), "body")
            .await
    }

    async fn close(mut self) -> Result<()> {
        drop(self.writer.take());
        self.worker.await?
    }
}

fn captions(body: &str) -> Vec<String> {
    body.split("SPARQL query ")
        .skip(1)
        .filter_map(|section| {
            let section = section.split_once("\n\n")?.1;
            Some(section.split_once("\n\n")?.0.to_owned())
        })
        .collect()
}

#[tokio::test]
async fn dprod_schedule_queries_show_their_constraint_messages() -> Result<()> {
    let fixture = Fixture::new().await?;
    let body = fixture.body(DPROD_SHAPE, "en").await?;
    let captions = captions(&body);
    assert_eq!(captions.len(), 4, "{body}");
    assert!(
        captions.contains(
            &"Schedules whose format carries no timezone must declare one \
              IANA timezone."
                .to_owned()
        ),
        "{captions:?}"
    );
    assert!(
        captions.contains(
            &"POSIX crontab schedules must contain exactly five time fields."
                .to_owned()
        ),
        "{captions:?}"
    );
    assert!(!body.contains("SHACL SELECT query."), "{body}");
    fixture.close().await
}

#[tokio::test]
async fn query_captions_follow_language_and_annotation_order() -> Result<()> {
    let fixture = Fixture::new().await?;
    let english = captions(&fixture.body("urn:captions", "en").await?);
    let mut sorted = english.clone();
    sorted.sort();
    assert_eq!(sorted, [
        "**SHACL SELECT query.**",
        "Commented constraint.",
        "Described constraint.",
        "Message in English.",
        "Targets every subject with a status.",
    ],);
    let dutch = captions(&fixture.body("urn:captions", "nl").await?);
    assert!(
        dutch.contains(&"Bericht in het Nederlands.".to_owned()),
        "{dutch:?}"
    );
    assert!(!dutch.contains(&"Message in English.".to_owned()));
    let validator = captions(&fixture.body("urn:validator", "en").await?);
    assert_eq!(validator, ["Validator message."]);
    fixture.close().await
}
