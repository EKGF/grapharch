//! One shared term/value table, so HTML and PDF present identical evidence.
use super::{
    identifiers::{CommitSha, RepositoryUrl},
    model::{BuildOperator, Commit, Provenance, Repository},
};

const HOMEPAGE: &str = "https://ekgf.github.io/grapharch/";
const GENERATOR_REPOSITORY: &str = "https://github.com/EKGF/grapharch";

/// Presentation-neutral value of one provenance field.
pub(super) enum Value {
    Text(String),
    Link {
        text: String,
        url:  String,
    },
    /// A commit, abbreviated and linked when its forge commit page is known.
    Commit {
        sha: CommitSha,
        url: Option<String>,
    },
    Sequence(Vec<Value>),
}

/// One term/value row of the provenance table.
pub(super) struct Field {
    pub(super) label: &'static str,
    pub(super) value: Value,
    /// Rows of a source repository are grouped per repository.
    pub(super) group: Option<usize>,
}

impl Field {
    fn new(label: &'static str, value: Value) -> Self {
        Self { label, value, group: None }
    }
}

impl Value {
    fn commit(sha: CommitSha, repository: &RepositoryUrl) -> Self {
        let url = repository.commit_url(&sha);
        Self::Commit { sha, url }
    }
}

impl Provenance {
    /// Every field once: generator, build run, then each source repository.
    pub(super) fn fields(&self) -> Vec<Field> {
        // bounded: four generator rows plus the capped operators and
        // repositories.
        let mut fields = vec![Field::new("Generator", generator())];
        if let Some(commit) = generator_commit() {
            fields.push(Field::new("Generator commit", commit));
        }
        fields.extend(
            self.operators
                .iter()
                .map(|operator| Field::new("Build operator", operator.value())),
        );
        for (index, repository) in self.repositories.iter().enumerate() {
            fields.extend(repository.fields().map(|mut field| {
                field.group = Some(index);
                field
            }));
        }
        fields
    }
}

impl Repository {
    fn fields(&self) -> impl Iterator<Item = Field> + '_ {
        [
            Field::new("Repository", Value::Link {
                text: self.url.as_str().into(),
                url:  self.url.as_str().into(),
            }),
            Field::new(
                "Source commit",
                Value::commit(self.commit.clone(), &self.url),
            ),
            Field::new("Source state", Value::Text(self.state().into())),
        ]
        .into_iter()
        .chain(
            (!self.tags.is_empty()).then(|| {
                Field::new("Git tags", Value::Text(self.tags.join(", ")))
            }),
        )
        .chain(self.releases.iter().map(|(text, url)| {
            Field::new("Release", Value::Link {
                text: text.clone(),
                url:  url.clone(),
            })
        }))
    }

    /// The source commit itself is named once, in the table.
    pub(super) fn commit_value(&self, commit: &Commit) -> Value {
        if commit.id == self.commit {
            match self.commit_url(&commit.id) {
                Some(url) => Value::Link { text: "source commit".into(), url },
                None => Value::Text("source commit".into()),
            }
        } else {
            Value::commit(commit.id.clone(), &self.url)
        }
    }
}

impl BuildOperator {
    fn value(&self) -> Value {
        let run = match &self.run {
            Some(url) => {
                Value::Link { text: self.label.clone(), url: url.clone() }
            },
            None => Value::Text(self.label.clone()),
        };
        if self.agents.is_empty() {
            run
        } else {
            Value::Sequence(vec![
                run,
                Value::Text(format!(" ({})", self.agents.join(", "))),
            ])
        }
    }
}

/// Name, version and build kind, without the commit the next row names.
fn generator() -> Value {
    let product = Value::Link {
        text: format!("GraphArch {}", env!("CARGO_PKG_VERSION")),
        url:  HOMEPAGE.into(),
    };
    let build = if !env!("GRAPHARCH_BUILD_RUN_URL").is_empty() {
        Value::Link {
            text: format!("build {}", env!("GRAPHARCH_BUILD_NUMBER")),
            url:  env!("GRAPHARCH_BUILD_RUN_URL").into(),
        }
    } else {
        Value::Text(
            match (
                env!("GRAPHARCH_BUILD_CHANNEL"),
                env!("GRAPHARCH_GENERATOR_MODIFIED") == "true",
            ) {
                ("release", _) => "release",
                _ if env!("GRAPHARCH_GENERATOR_REVISION").is_empty() => {
                    "source archive"
                },
                ("local", true) => "local build, modified source",
                ("local", false) => "local build",
                (_, true) => "source build, modified source",
                (_, false) => "source build",
            }
            .into(),
        )
    };
    Value::Sequence(vec![
        product,
        Value::Text(" (".into()),
        build,
        Value::Text(")".into()),
    ])
}

fn generator_commit() -> Option<Value> {
    let revision = env!("GRAPHARCH_GENERATOR_REVISION");
    if revision.is_empty() {
        return None;
    }
    let sha = CommitSha::try_new(revision)
        .expect("the build script validates the generator revision");
    let repository = RepositoryUrl::try_new(GENERATOR_REPOSITORY)
        .expect("generator repository constant is a valid URL");
    Some(Value::commit(sha, &repository))
}
