use super::*;
#[cfg(unix)]
use crate::publication::{
    assembly::Assembler,
    collect::{Collector, RdfFile, plantuml::PlantUmlFile},
    plan::TargetPlanner,
    render::{PublishOutcome, Publisher},
};

#[tokio::test]
async fn missing_local_renderer_is_an_actionable_error_without_download_fallback()
 {
    let missing =
        std::env::temp_dir().join("grapharch-no-such-plantuml-executable");
    let error = runner::version(missing.as_os_str())
        .await
        .expect_err("renderer must be installed explicitly");
    assert!(format!("{error:#}").contains("never downloads a renderer"));
}

#[cfg(unix)]
#[tokio::test]
async fn retained_sources_document_after_deletion_and_svg_renders_without_the_tool()
-> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let source = tempfile::tempdir()?;
    let tool = tempfile::tempdir()?;
    let executable = tool.path().join("recorded-plantuml");
    // Recorded offline protocol fixture; no claim of a real PlantUML
    // invocation.
    tokio::fs::write(&executable, r##"#!/bin/sh
if [ "$PLANTUML_SECURITY_PROFILE" != SANDBOX ]; then exit 2; fi
if [ "$1" = -version ]; then printf 'PlantUML version recorded-fixture\n'; exit 0; fi
while IFS= read -r line; do
  if [ "$line" = @enduml ]; then
    printf '<svg xmlns="http://www.w3.org/2000/svg" width="120" height="40"><rect width="110" height="30" fill="#116659"/></svg>GRAPHARCH-PLANTUML-END\n' || exit $?
  fi
done
"##).await?;
    let mut permissions = tokio::fs::metadata(&executable).await?.permissions();
    permissions.set_mode(0o755);
    tokio::fs::set_permissions(&executable, permissions).await?;
    tokio::fs::write(
        source.path().join("diagram.puml"),
        "@startuml\n!include theme.puml\nclass Example\n@enduml\n",
    )
    .await?;
    tokio::fs::write(
        source.path().join("theme.puml"),
        "skinparam backgroundColor transparent\n",
    )
    .await?;
    let asset = identity("local-image-asset", &[
        "urn:specification:project",
        "diagrams/out/diagram.svg",
    ]);
    let definitions = source.path().join("project.ttl");
    let authored =
        include_str!("../../../../examples/specification/project.ttl").replace(
            "A publication is a view",
            "![Source diagram](diagrams/out/diagram.svg)\n\nA publication is \
             a view",
        );
    tokio::fs::write(&definitions, format!(r#"{authored}
ex:project ga:collector ex:uml .
ex:uml a ga:Collector; ga:kind "plantuml-file"; ga:source "diagram.puml"; ga:destination "diagrams/out/diagram.svg"; ga:rendererExecutable "plantuml" .
ex:scope ga:include {asset} .
ex:intro-body ga:asset {asset}; ga:reference ex:uml-reference .
ex:uml-reference a ga:ExternalReference; dct:identifier "diagrams/out/diagram.svg"; ga:url "{asset_url}"; ga:asset {asset} .
"#, asset_url=asset.as_str())).await?;
    let dataset = Dataset::memory()?;
    let (writer, worker) = GraphWriter::spawn(dataset.clone(), 4, 1024 * 1024)?;
    RdfFile::new(
        "urn:specification:project",
        "urn:uml:definitions",
        definitions,
    )?
    .collect(&writer)
    .await?;
    let view = RevisionView::new(
        dataset.clone(),
        dataset.snapshot_for("urn:specification:project").await?,
    );
    PlantUmlFile::configured(
        "urn:specification:project",
        &NamedNode::new("urn:specification:uml")?,
        "diagram.puml",
        source.path(),
        &view,
    )
    .await?
    .collect(&writer)
    .await?;
    tokio::fs::remove_dir_all(source.path()).await?;
    document_with(
        &dataset,
        "urn:specification:project",
        &writer,
        executable.as_os_str(),
    )
    .await?;
    let revision = Assembler::new(dataset.clone())
        .assemble("urn:specification:project")
        .await?;
    let plan = TargetPlanner::new(dataset.clone(), revision)
        .resolve(
            "urn:specification:project",
            &NamedNode::new("urn:specification:target")?,
            &writer,
        )
        .await?;
    drop(writer);
    worker.await??;
    let output = tempfile::tempdir()?;
    let archive = output.path().join("documentation.nq");
    dataset.export(archive.clone()).await?;
    tokio::fs::remove_dir_all(tool.path()).await?;
    let restored = Dataset::memory()?;
    restored.import(archive).await?;
    let PublishOutcome::Published(published) = Publisher::new(restored)
        .render(&plan, output.path())
        .await?
    else {
        panic!("publication required")
    };
    assert!(
        tokio::fs::read_to_string(published.directory.join("main.typ"))
            .await?
            .contains("#image(")
    );
    assert!(
        tokio::fs::read(published.directory.join("document.pdf"))
            .await?
            .starts_with(b"%PDF-")
    );
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn renderer_output_and_identity_are_bounded_and_validated() {
    let program = OsStr::new("/bin/sh");
    let error = runner::run(
        program,
        &["-c", "printf abcdefghijklmnopqrstuvwxyz"],
        b"",
        8,
    )
    .await
    .expect_err("oversized output must fail");
    assert!(format!("{error:#}").contains("output exceeds byte budget"));
    assert!(runner::version(OsStr::new("/bin/echo")).await.is_err());
}

#[tokio::test]
#[ignore = "requires an explicitly installed local PlantUML and Graphviz"]
async fn installed_plantuml_produces_repeatable_valid_svg_without_remote_includes()
-> Result<()> {
    let program = OsStr::new("plantuml");
    let source =
        "@startuml\nclass Example [[/concept/example/#definition]]\n@enduml\n";
    let renderer = LocalRenderer::new(program);
    let first = renderer.svg(source).await?;
    let second = renderer.svg(source).await?;
    assert_eq!(first, second);
    crate::publication::image_assets::validate(&first, "image/svg+xml")?;
    assert!(
        renderer
            .svg(
                "@startuml\n!includeurl \
                 https://example.org/never-fetch\nclass A\n@enduml\n"
            )
            .await
            .is_err()
    );
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn local_renderer_reuses_one_bounded_pipe_for_multiple_diagrams()
-> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir()?;
    let executable = root.path().join("recorded-pipe");
    tokio::fs::write(&executable, r##"#!/bin/sh
if [ "$PLANTUML_SECURITY_PROFILE" != SANDBOX ]; then exit 2; fi
if [ "$1" = -version ]; then printf 'PlantUML version recorded-pipe\n'; exit 0; fi
count=0
while IFS= read -r line; do
  if [ "$line" = @enduml ]; then
    count=$((count + 1))
    printf '<svg xmlns="http://www.w3.org/2000/svg" width="100" height="40" data-render="%s"><rect width="90" height="30"/></svg>GRAPHARCH-PLANTUML-END\n' "$count" || exit $?
  fi
done
"##).await?;
    let mut permissions = tokio::fs::metadata(&executable).await?.permissions();
    permissions.set_mode(0o755);
    tokio::fs::set_permissions(&executable, permissions).await?;
    let renderer = LocalRenderer::new(executable.as_os_str());
    assert_eq!(
        renderer.version().await?,
        "PlantUML version recorded-pipe"
    );
    let first = renderer.svg("@startuml\nclass First\n@enduml\n").await?;
    let second = renderer.svg("@startuml\nclass Second\n@enduml\n").await?;
    assert!(std::str::from_utf8(&first)?.contains("data-render=\"1\""));
    assert!(std::str::from_utf8(&second)?.contains("data-render=\"2\""));
    assert!(!std::str::from_utf8(&second)?.contains("GRAPHARCH-PLANTUML-END"));
    let named = renderer
        .svg(
            "' authored preamble\n@startuml named-diagram\nclass \
             Named\n@enduml\n",
        )
        .await?;
    assert!(std::str::from_utf8(&named)?.contains("data-render=\"3\""));
    Ok(())
}
