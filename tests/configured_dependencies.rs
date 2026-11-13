#[path = "support/http.rs"]
mod http;

use {
    anyhow::Result,
    grapharch::publication::{
        dataset::Dataset,
        render::{PublishOutcome, Publisher},
        view::RevisionView,
        website,
    },
    oxrdf::{Literal, NamedNode},
    tokio::{io::AsyncWriteExt, net::TcpListener},
};

fn git_source_dependency(source: &str) -> String {
    format!(
        r#"<urn:deps:git-source> a ga:CollectorUse; ga:collectorType ga:GitRdfFileCollector; ga:source {}; ga:sourcePath "ontology.ttl"; ga:gitRef "main" ."#,
        Literal::new_simple_literal(source)
    )
}

#[test]
fn configured_git_source_literals_preserve_windows_paths() -> Result<()> {
    let path = r"C:\Users\RUNNER~1\AppData\Local\Temp\repository";
    let source = format!(
        "@prefix ga:<https://ekgf.org/ontology/grapharch#> .\n{}",
        git_source_dependency(path)
    );
    let quads = oxrdfio::RdfParser::from_format(oxrdfio::RdfFormat::Turtle)
        .for_slice(source.as_bytes());
    let mut observed = false;
    for quad in quads {
        let quad = quad?;
        if quad.predicate.as_str() ==
            "https://ekgf.org/ontology/grapharch#source"
        {
            assert_eq!(
                quad.object,
                Literal::new_simple_literal(path).into()
            );
            observed = true;
        }
    }
    assert!(observed);
    Ok(())
}

async fn serve(
    body: Vec<u8>,
    status: &str,
    mime: &str,
) -> Result<(String, tokio::task::JoinHandle<Result<()>>)> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let url = format!(
        "http://{}/vocabulary/terms.ttl",
        listener.local_addr()?
    );
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {mime}\r\nContent-Length: \
         {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await?;
        http::read_request_headers(&mut socket).await?;
        socket.write_all(header.as_bytes()).await?;
        socket.write_all(&body).await?;
        Ok(())
    });
    Ok((url, task))
}

async fn site(root: &std::path::Path, dependencies: &str) -> Result<()> {
    tokio::fs::write(
        root.join("index.md"),
        "---\ntitle: Home\npermalink: /\n---\n# Home\nDependencies stay in \
         the graph.\n",
    )
    .await?;
    tokio::fs::write(root.join("project.ttl"), format!(r#"
@prefix ga:<https://ekgf.org/ontology/grapharch#> .
@prefix dct:<http://purl.org/dc/terms/> .
<urn:deps:project> a ga:Project; dct:title "Dependencies"; ga:scope <urn:deps:scope>; ga:target <urn:deps:website> .
<urn:deps:scope> a ga:Scope . <urn:deps:profile> a ga:Profile .
<urn:deps:website> a ga:Target; dct:title "Dependencies"; ga:scope <urn:deps:scope>; ga:profile <urn:deps:profile>; ga:renderer <https://ekgf.org/ontology/grapharch#WebsiteRenderer>;ga:destination "site" .
{dependencies}
"#)).await?;
    Ok(())
}

#[tokio::test]
async fn configured_local_git_and_http_rdf_are_native_sources_before_publication()
-> Result<()> {
    let root = tempfile::tempdir()?;
    let git = tempfile::tempdir()?;
    tokio::fs::write(
        root.path().join("local.ttl"),
        "<urn:deps:local> a <http://www.w3.org/2002/07/owl#Class> .",
    )
    .await?;
    tokio::fs::write(
        git.path().join("ontology.ttl"),
        "<urn:deps:git> a <http://www.w3.org/2002/07/owl#Class> .",
    )
    .await?;
    for args in [
        vec!["init", "-b", "main"],
        vec!["config", "user.name", "Fixture"],
        vec!["config", "user.email", "fixture@example.org"],
        vec!["add", "."],
        vec!["commit", "-m", "fixture"],
    ] {
        let output = tokio::process::Command::new("git")
            .arg("-C")
            .arg(git.path())
            .args(args)
            .output()
            .await?;
        anyhow::ensure!(output.status.success(), "fixture git failed");
    }
    let (url, server) = serve(
        b"<Thing> a <http://www.w3.org/2002/07/owl#Class> .".to_vec(),
        "200 OK",
        "text/turtle",
    )
    .await?;
    site(root.path(), &format!(r#"
<urn:deps:project> ga:collector <urn:deps:local-source>, <urn:deps:git-source>, <urn:deps:url-source> .
<urn:deps:local-source> a ga:CollectorUse; ga:collectorType ga:RdfFileCollector; ga:source "local.ttl" .
{}
<urn:deps:url-source> a ga:CollectorUse; ga:collectorType ga:RdfUrlCollector; ga:source "{url}"; ga:format "text/turtle" .
"#, git_source_dependency(git.path().to_str().unwrap()))).await?;
    let built = website::collect(
        root.path(),
        &root.path().join("project.ttl"),
        "urn:deps:project",
        "urn:deps:website",
        3,
    )
    .await?;
    let view = RevisionView::new(
        built.dataset.clone(),
        built.dataset.snapshot_for("urn:deps:project").await?,
    );
    for iri in [
        "urn:deps:local".to_owned(),
        "urn:deps:git".to_owned(),
        url::Url::parse(&url)?.join("Thing")?.to_string(),
    ] {
        let mut rows = view
            .objects_by(
                &NamedNode::new(iri)?,
                &NamedNode::new(
                    "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
                )?,
            )
            .await?;
        assert!(
            rows.next().await?.is_some(),
            "configured source must retain its native RDF"
        );
    }
    server.await??;
    let mut provenance = built.dataset.select(format!("SELECT ?c WHERE {{ GRAPH ?g {{ ?c <https://ekgf.org/ontology/grapharch#source> \"{url}\" }} }} LIMIT 1")).await?;
    assert!(provenance.next().await?.is_some());
    drop(provenance);
    let output = tempfile::tempdir()?;
    let archive = output.path().join("kg.nq");
    built.dataset.export(archive.clone()).await?;
    root.close()?;
    git.close()?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    assert!(matches!(
        Publisher::new(restored)
            .with_edition(grapharch::publication::pdf_name::Edition::Default)
            .render(&built.plan, output.path())
            .await?,
        PublishOutcome::Published(_)
    ));
    Ok(())
}

#[tokio::test]
async fn failed_http_dependency_prevents_website_assembly() -> Result<()> {
    let root = tempfile::tempdir()?;
    let (url, server) = serve(
        b"not available".to_vec(),
        "404 Not Found",
        "text/turtle",
    )
    .await?;
    site(root.path(), &format!(r#"<urn:deps:project> ga:collector <urn:deps:bad> . <urn:deps:bad> a ga:CollectorUse; ga:collectorType ga:RdfUrlCollector;ga:source "{url}";ga:format "text/turtle" ."#)).await?;
    let error = website::collect(
        root.path(),
        &root.path().join("project.ttl"),
        "urn:deps:project",
        "urn:deps:website",
        1,
    )
    .await
    .err()
    .expect("a failed dependency must block assembly");
    server.await??;
    assert!(format!("{error:#}").contains("404"));
    Ok(())
}

#[tokio::test]
async fn rdf_url_rejects_unsupported_protocol_mismatched_format_invalid_rdf_and_oversize()
-> Result<()> {
    use grapharch::publication::{
        collect::{Collector, RdfUrl},
        writer::GraphWriter,
    };
    for source in [
        "file:///tmp/terms.ttl",
        "ftp://example.org/terms.ttl",
        "https://user:password@example.org/terms.ttl",
        "https://example.org/terms.ttl#fragment",
    ] {
        assert!(
            RdfUrl::new(
                "urn:deps:project",
                "urn:deps:url-source",
                source,
                "text/turtle"
            )
            .is_err()
        );
    }
    assert!(
        RdfUrl::new(
            "urn:deps:project",
            "urn:deps:url-source",
            "https://example.org/terms",
            "application/octet-stream"
        )
        .is_err()
    );
    // bounded: one response just over the fixed 8 MiB limit exercises header
    // rejection; the other fixtures are small malformed responses.
    for (body, mime, expected) in [
        (
            b"<urn:s> <urn:p> <urn:o> .".to_vec(),
            "application/n-triples",
            "Content-Type",
        ),
        (b"not valid turtle".to_vec(), "text/turtle", ""),
        (
            vec![b' '; 8 * 1024 * 1024 + 1],
            "text/turtle",
            "8 MiB",
        ),
    ] {
        let (url, server) = serve(body, "200 OK", mime).await?;
        let dataset = Dataset::memory()?;
        let (writer, worker) = GraphWriter::spawn(dataset, 4, 4 * 1024 * 1024)?;
        let collector = RdfUrl::new(
            "urn:deps:project",
            "urn:deps:url-source",
            &url,
            "text/turtle",
        )?;
        let error = collector
            .collect(&writer)
            .await
            .expect_err("invalid dependency response must fail");
        assert!(format!("{error:#}").contains(expected));
        // A rejected Content-Length can close the socket before the fixture
        // finishes sending its body; that server-side write failure is
        // expected.
        let _ = server.await;
        drop(writer);
        worker.await??;
    }
    Ok(())
}

#[tokio::test]
async fn configured_local_dependencies_cannot_escape_the_documentation_root()
-> Result<()> {
    let parent = tempfile::tempdir()?;
    let root = parent.path().join("docs");
    tokio::fs::create_dir_all(&root).await?;
    tokio::fs::write(
        parent.path().join("outside.ttl"),
        "<urn:s> <urn:p> <urn:o> .",
    )
    .await?;
    site(&root, r#"<urn:deps:project> ga:collector <urn:deps:outside> . <urn:deps:outside> a ga:CollectorUse;ga:collectorType ga:RdfFileCollector;ga:source "../outside.ttl" ."#).await?;
    let error = website::collect(
        &root,
        &root.join("project.ttl"),
        "urn:deps:project",
        "urn:deps:website",
        1,
    )
    .await
    .err()
    .expect("source traversal must fail");
    assert!(format!("{error:#}").contains("traversal"));
    Ok(())
}

#[tokio::test]
async fn configured_dependency_budget_and_required_git_revision_are_enforced()
-> Result<()> {
    let root = tempfile::tempdir()?;
    tokio::fs::write(
        root.path().join("terms.ttl"),
        "<urn:s> <urn:p> <urn:o> .",
    )
    .await?;
    // bounded: one over the declared 32-source configuration budget.
    let mut definitions = String::new();
    for index in 0..33 {
        definitions.push_str(&format!(r#"<urn:deps:project> ga:collector <urn:deps:source:{index}> . <urn:deps:source:{index}> a ga:CollectorUse;ga:collectorType ga:RdfFileCollector;ga:source "terms.ttl" .
"#));
    }
    site(root.path(), &definitions).await?;
    let error = website::collect(
        root.path(),
        &root.path().join("project.ttl"),
        "urn:deps:project",
        "urn:deps:website",
        2,
    )
    .await
    .err()
    .expect("excess source definitions must fail");
    assert!(format!("{error:#}").contains("32-source"));
    site(root.path(), r#"<urn:deps:project> ga:collector <urn:deps:git> . <urn:deps:git> a ga:CollectorUse;ga:collectorType ga:GitRdfFileCollector;ga:source "https://example.org/repository.git";ga:sourcePath "terms.ttl" ."#).await?;
    let error = website::collect(
        root.path(),
        &root.path().join("project.ttl"),
        "urn:deps:project",
        "urn:deps:website",
        1,
    )
    .await
    .err()
    .expect("Git source must explicitly select its revision");
    assert!(format!("{error:#}").contains("gitRef"));
    Ok(())
}

#[tokio::test]
async fn canonical_collector_definition_keeps_two_source_uses_distinct()
-> Result<()> {
    let root = tempfile::tempdir()?;
    tokio::fs::write(
        root.path().join("one.ttl"),
        "<urn:one> <urn:p> <urn:o> .",
    )
    .await?;
    tokio::fs::write(
        root.path().join("two.ttl"),
        "<urn:two> <urn:p> <urn:o> .",
    )
    .await?;
    site(root.path(), r#"<urn:deps:project> ga:collector <urn:use:one>, <urn:use:two>.
<urn:use:one> a ga:CollectorUse;ga:collectorType ga:RdfFileCollector;ga:source "one.ttl".
<urn:use:two> a ga:CollectorUse;ga:collectorType ga:RdfFileCollector;ga:source "two.ttl"."#).await?;
    let built = website::collect(
        root.path(),
        &root.path().join("project.ttl"),
        "urn:deps:project",
        "urn:deps:website",
        2,
    )
    .await?;
    let archive = root.path().join("graph.nq");
    built.dataset.export(archive.clone()).await?;
    let text = tokio::fs::read_to_string(archive).await?;
    for value in [
        "urn:use:one",
        "urn:use:two",
        "urn:one",
        "urn:two",
        "#RdfFileCollector",
    ] {
        assert!(text.contains(value), "missing {value}");
    }
    Ok(())
}

#[tokio::test]
async fn collector_definition_rejects_literal_and_unknown_iri() -> Result<()> {
    for value in [r#""rdf-file""#, "<urn:unknown:collector>"] {
        let root = tempfile::tempdir()?;
        site(
            root.path(),
            &format!(
                "<urn:deps:project> ga:collector <urn:use>. <urn:use> a \
                 ga:CollectorUse;ga:collectorType {value};ga:source \
                 \"missing.ttl\"."
            ),
        )
        .await?;
        let error = website::collect(
            root.path(),
            &root.path().join("project.ttl"),
            "urn:deps:project",
            "urn:deps:website",
            1,
        )
        .await
        .err()
        .expect("invalid identity accepted");
        assert!(
            error.to_string().contains("collectorType"),
            "{error:#}"
        );
    }
    Ok(())
}
