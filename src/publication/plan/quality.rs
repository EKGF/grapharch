//! Review competing source annotations without imposing RDF cardinalities.
use {
    crate::publication::{
        dataset::{CONTROL, Dataset, identity},
        diagnostic_graph::{self, Context},
        diagnostics::{Diagnostic, Severity},
        term,
        view::RevisionView,
        writer::ContributionStream,
    },
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Quad, Term},
};

pub(super) async fn append(
    dataset: &Dataset,
    view: &RevisionView,
    revision: &NamedNode,
    project: &NamedNode,
    plan: &NamedNode,
    stream: &mut ContributionStream,
) -> Result<()> {
    let pattern = view.pattern("?subject ?predicate ?value", 0);
    let fields = [
        "http://www.w3.org/2000/01/rdf-schema#label",
        "http://www.w3.org/2004/02/skos/core#prefLabel",
        "http://purl.org/dc/terms/title",
        "http://purl.org/dc/terms/description",
        "http://www.w3.org/2004/02/skos/core#definition",
        "https://ekgf.org/ontology/annotation#markdownDescription",
    ]
    .map(|iri| format!("<{iri}>"))
    .join(" ");
    let source = format!(
        "GRAPH <{CONTROL}> {{ ?_attempt0 {} \"collect\" }}",
        term("stage")
    );
    let mut groups = dataset
        .select(format!(
            "SELECT ?subject ?predicate ?language ?datatype WHERE {{ \
             {pattern} {source} VALUES ?predicate {{ {fields} }} \
             FILTER(isIRI(?subject) && isLiteral(?value)) \
             BIND(LCASE(LANG(?value)) AS ?language) BIND(DATATYPE(?value) AS \
             ?datatype) }} GROUP BY ?subject ?predicate ?language ?datatype \
             HAVING(COUNT(DISTINCT ?value) > 1) ORDER BY ?subject ?predicate \
             ?language LIMIT 1025"
        ))
        .await?;
    let mut count = 0;
    while let Some(row) = groups.next().await? {
        count += 1;
        ensure!(
            count <= 1024,
            "publication exceeds 1024 quality issues"
        );
        let Term::NamedNode(subject) = &row["subject"] else {
            anyhow::bail!("quality subject must be an IRI")
        };
        let predicate = &row["predicate"];
        let language = &row["language"];
        let datatype = &row["datatype"];
        let values = view.pattern(&format!("{subject} {predicate} ?value"), 0);
        let location = view.pattern(
            "?artifact <http://www.w3.org/ns/prov#atLocation> ?location",
            1,
        );
        let mut rows = dataset
            .select(format!(
                "SELECT DISTINCT ?value ?origin ?location WHERE {{ {values} \
                 GRAPH <{CONTROL}> {{ ?_attempt0 {} \"collect\" ; {} ?origin \
                 }} FILTER(isLiteral(?value) && LCASE(LANG(?value)) = \
                 {language} && DATATYPE(?value) = {datatype}) \
                 BIND(IRI(STR(?origin)) AS ?artifact) OPTIONAL {{ {location} \
                 }} }} ORDER BY ?value ?origin ?location LIMIT 129",
                term("stage"),
                term("source")
            ))
            .await?;
        let mut message = format!(
            "Competing source annotations for {subject} {predicate} \
             ({language}). All values are retained for review; multiple \
             values are not automatically an RDF error."
        );
        let mut provenance = Vec::new(); // bounded: 128 value/source records per issue.
        while let Some(row) = rows.next().await? {
            ensure!(
                provenance.len() < 128,
                "quality annotation exceeds 128 value/source records"
            );
            let origin = row.get("location").unwrap_or(&row["origin"]).clone();
            message.push_str(&format!("\n{} — source: {origin}", row["value"]));
            ensure!(
                message.len() <= 256 * 1024,
                "quality issue exceeds message byte budget"
            );
            provenance.push(origin);
        }
        let job = identity("quality-issue", &[
            revision.as_str(),
            subject.as_str(),
            &predicate.to_string(),
            &language.to_string(),
        ])
        .into_string();
        let context = Context {
            graph:     plan.clone(),
            id:        plan.clone(),
            project:   project.clone(),
            about:     subject.clone(),
            revision:  revision.clone(),
            processor: NamedNode::new_unchecked(
                "urn:grapharch:quality-inspector",
            ),
            source:    subject.as_str().into(),
        };
        let diagnostic = Diagnostic {
            severity: Severity::Warning,
            code: "competing-source-annotations".into(),
            message,
            reference: subject.as_str().into(),
            source_fragment: None,
        };
        for quad in diagnostic_graph::quads(&context, &job, &[diagnostic])?
            .into_iter()
            .filter(|q| q.predicate != term("diagnosticCount"))
        {
            stream.write(quad).await?;
        }
        let id = NamedNode::new(format!("urn:grapharch:diagnostic:{job}:0"))?;
        for origin in provenance {
            stream
                .write(Quad::new(
                    id.clone(),
                    NamedNode::new_unchecked("http://purl.org/dc/terms/source"),
                    origin,
                    plan.clone(),
                ))
                .await?;
        }
    }
    retain_source_warnings(dataset, revision, project, plan, stream, count)
        .await
}

async fn retain_source_warnings(
    dataset: &Dataset,
    revision: &NamedNode,
    project: &NamedNode,
    plan: &NamedNode,
    stream: &mut ContributionStream,
    mut count: usize,
) -> Result<()> {
    let ga = "https://ekgf.org/ontology/grapharch#";
    let view = RevisionView::new(dataset.clone(), revision.clone());
    let findings = view.pattern(
        &format!(
            "?issue a <{ga}Diagnostic> ; <{ga}severity> \"warning\" ; \
             <{ga}code> ?code ; <{ga}message> ?message ; \
             <{ga}unresolvedReference> ?reference"
        ),
        0,
    );
    let fragment = view.pattern(
        &format!("?issue <{ga}sourceFragment> ?fragment"),
        1,
    );
    let mut rows = dataset
        .select(format!(
            "SELECT DISTINCT ?code ?message ?reference ?fragment WHERE {{ \
             {findings} OPTIONAL {{ {fragment} }} }} ORDER BY ?reference \
             ?code ?message LIMIT 1025"
        ))
        .await?;
    while let Some(row) = rows.next().await? {
        count += 1;
        ensure!(
            count <= 1024,
            "publication exceeds 1024 quality issues"
        );
        let text = |name: &str| -> Result<String> {
            match &row[name] {
                Term::Literal(v) => Ok(v.value().into()),
                _ => anyhow::bail!("diagnostic {name} must be a literal"),
            }
        };
        let reference = text("reference")?;
        let code = text("code")?;
        let message = text("message")?;
        let about =
            NamedNode::new(&reference).unwrap_or_else(|_| project.clone());
        let job = identity("retained-quality-issue", &[
            revision.as_str(),
            &reference,
            &code,
            &message,
        ])
        .into_string();
        let context = Context {
            graph: plan.clone(),
            id: plan.clone(),
            project: project.clone(),
            about,
            revision: revision.clone(),
            processor: NamedNode::new_unchecked(
                "urn:grapharch:quality-inspector",
            ),
            source: reference.clone(),
        };
        let source_fragment = match row.get("fragment") {
            Some(Term::NamedNode(v)) => Some(v.as_str().into()),
            None => None,
            _ => anyhow::bail!("source fragment must be an IRI"),
        };
        let diagnostic = Diagnostic {
            severity: Severity::Warning,
            code,
            message,
            reference,
            source_fragment,
        };
        for quad in diagnostic_graph::quads(&context, &job, &[diagnostic])?
            .into_iter()
            .filter(|q| q.predicate != term("diagnosticCount"))
        {
            stream.write(quad).await?;
        }
    }
    Ok(())
}
