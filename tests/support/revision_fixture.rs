//! An exact-revision fixture: the example's content tree in a content
//! repository, and its publication definitions in a separate controlling
//! checkout, as an orchestrator would supply them.
// Several test crates include this fixture; each uses a subset of it.
#![allow(dead_code)]
use {
    super::family_repository,
    anyhow::Result,
    grapharch::publication::{
        collect::git::{self, CommittedDefinitions},
        dataset::Dataset,
        plan::ResolvedPlan,
        revision_build::{self, RevisionInputs, TargetSelection},
    },
    std::path::{Path, PathBuf},
};

/// Project defined by the example's definitions.
pub const PROJECT: &str = "urn:dprod-showcase:project";
/// Definitions path within the controlling checkout.
pub const DEFINITIONS: &str = "publication/definitions.ttl";
/// Generator revision declared by the test orchestrator.
pub const GENERATOR: &str = "1111111111111111111111111111111111111111";

/// Example files and their location in the pinned content tree.
const CONTENT: [(&str, &str); 8] = [
    ("sources/ontology.ttl", "ontology.ttl"),
    ("sources/shapes.ttl", "shapes.ttl"),
    ("sources/dataset-schema.md", "dataset-schema.md"),
    ("sources/LICENSE.txt", "LICENSE.txt"),
    ("abstract.md", "abstract.md"),
    ("status.md", "status.md"),
    ("overview.md", "overview.md"),
    ("website", "website"),
];

pub struct RevisionFixture {
    root:                   tempfile::TempDir,
    pub repository:         String,
    pub commit:             String,
    pub definitions_commit: String,
}

impl RevisionFixture {
    /// Pin `example`'s content and definitions in two separate repositories.
    pub fn new(example: &Path) -> Result<Self> {
        let root = tempfile::tempdir()?;
        let content = root.path().join("content-source");
        for (source, target) in CONTENT {
            let (source, target) = (example.join(source), content.join(target));
            if source.is_dir() {
                family_repository::copy_tree(&source, &target)?;
            } else {
                std::fs::create_dir_all(target.parent().unwrap())?;
                std::fs::copy(source, target)?;
            }
        }
        let (repository, commit) = family_repository::bare_copy(
            &content,
            &root.path().join("content"),
        )?;
        let controlling = root.path().join("controlling");
        std::fs::create_dir_all(controlling.join("publication"))?;
        std::fs::copy(
            example.join("definitions.ttl"),
            controlling.join(DEFINITIONS),
        )?;
        family_repository::git(&controlling, &["init", "-b", "main"])?;
        family_repository::git(&controlling, &["add", "--all"])?;
        family_repository::git(&controlling, &[
            "commit",
            "-m",
            "Controlling definitions",
        ])?;
        let definitions_commit =
            family_repository::git(&controlling, &["rev-parse", "HEAD"])?;
        Ok(Self { root, repository, commit, definitions_commit })
    }

    /// Working checkout of the content repository.
    pub fn content(&self) -> PathBuf {
        self.root.path().join("content/checkout")
    }

    /// Working checkout of the controlling repository.
    pub fn controlling(&self) -> PathBuf {
        self.root.path().join("controlling")
    }

    /// Definitions file in the controlling checkout.
    pub fn definitions(&self) -> PathBuf {
        self.controlling().join(DEFINITIONS)
    }

    /// Bare transport cache shared by library and CLI builds.
    pub fn cache(&self) -> PathBuf { self.root.path().join("cache") }

    /// Edit one content file and pin the edit as the designated commit.
    pub fn edit_content(
        &mut self,
        path: &str,
        edit: impl FnOnce(String) -> Result<String>,
    ) -> Result<()> {
        let file = self.content().join(path);
        let text = if file.exists() {
            std::fs::read_to_string(&file)?
        } else {
            String::new()
        };
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&file, edit(text)?)?;
        let checkout = self.content();
        family_repository::git(&checkout, &["add", "--all"])?;
        family_repository::git(&checkout, &["commit", "-m", "Edit content"])?;
        self.commit =
            family_repository::git(&checkout, &["rev-parse", "HEAD"])?;
        family_repository::git(&checkout, &[
            "push",
            &self.repository,
            "HEAD:refs/heads/main",
        ])?;
        Ok(())
    }

    /// Edit the definitions and pin the edit as the controlling commit.
    pub fn edit_definitions(
        &mut self,
        edit: impl FnOnce(String) -> Result<String>,
    ) -> Result<()> {
        let file = self.definitions();
        std::fs::write(&file, edit(std::fs::read_to_string(&file)?)?)?;
        let checkout = self.controlling();
        family_repository::git(&checkout, &["add", "--all"])?;
        family_repository::git(&checkout, &[
            "commit",
            "-m",
            "Edit definitions",
        ])?;
        self.definitions_commit =
            family_repository::git(&checkout, &["rev-parse", "HEAD"])?;
        Ok(())
    }

    /// Collect and plan through the library, as a gallery runner does.
    pub async fn collect(
        &self,
        targets: TargetSelection,
    ) -> Result<(Dataset, Vec<ResolvedPlan>)> {
        let content = git::remote_documentation(
            &self.repository,
            &self.commit,
            &self.cache(),
            PROJECT,
        )
        .await?;
        let definitions = CommittedDefinitions::checkout(
            &self.definitions(),
            &self.definitions_commit,
            PROJECT,
        )
        .await?;
        revision_build::collect(RevisionInputs {
            content,
            definitions,
            project: oxrdf::NamedNode::new(PROJECT)?,
            targets,
        })
        .await
    }

    /// Run `publication build-revision` with these pins and `selection`
    /// arguments.
    pub async fn build(
        &self,
        output: &Path,
        selection: &[&str],
    ) -> Result<std::process::Output> {
        self.build_with(output, selection, &[]).await
    }

    /// Run `publication build-revision`, overriding pinned arguments.
    pub async fn build_with(
        &self,
        output: &Path,
        selection: &[&str],
        overrides: &[(&str, &str)],
    ) -> Result<std::process::Output> {
        let definitions = self.definitions();
        let cache = self.cache();
        let mut arguments = vec![
            ("--git-repository", self.repository.as_str()),
            ("--git-commit", self.commit.as_str()),
            ("--definitions", definitions.to_str().unwrap()),
            (
                "--definitions-commit",
                self.definitions_commit.as_str(),
            ),
            ("--project", PROJECT),
            ("--generator-revision", GENERATOR),
            ("--git-cache", cache.to_str().unwrap()),
            ("--output-root", output.to_str().unwrap()),
        ];
        for (name, value) in overrides {
            let argument = arguments
                .iter_mut()
                .find(|(existing, _)| existing == name)
                .expect("known argument");
            argument.1 = value;
        }
        let mut command =
            tokio::process::Command::new(env!("CARGO_BIN_EXE_grapharch"));
        command.args(["publication", "build-revision", "--json"]);
        for (name, value) in arguments {
            command.args([name, value]);
        }
        command.args(selection);
        Ok(command.output().await?)
    }
}
