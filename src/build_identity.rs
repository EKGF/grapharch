//! Compile-time identity separates package versions from released executables.
#[derive(Debug)]
pub(crate) struct Identity {
    pub label:    String,
    pub channel:  &'static str,
    pub revision: String,
}
impl Identity {
    pub fn new(
        version: &str,
        revision: Option<&str>,
        dirty: bool,
        release: Option<&str>,
    ) -> Result<Self, String> {
        if let Some(revision) = revision {
            if revision.bytes().all(|b| b == b'0') ||
                revision.len() != 40 ||
                !revision.bytes().all(|b| {
                    b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
                })
            {
                return Err("generator revision must be a full lowercase Git \
                            commit"
                    .into());
            }
        }
        let (label, channel) = match (release, revision) {
            (Some(release), Some(_)) if release == version && !dirty => {
                (version.to_owned(), "release")
            },
            (Some(_), _) => {
                return Err("release identity requires the matching package \
                            version and clean exact source revision"
                    .into())
            },
            (None, Some(revision)) => {
                (
                    format!(
                        "{version}-dev+{}{}",
                        &revision[..12],
                        if dirty { ".dirty" } else { "" }
                    ),
                    "development",
                )
            },
            (None, None) => (format!("{version}-source"), "source"),
        };
        Ok(Self {
            label,
            channel,
            revision: revision.unwrap_or("").into(),
        })
    }
}
