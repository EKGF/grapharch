//! The vertical publication status band a theme selects with
//! `ga:statusRibbon`: the publisher's acronym and the target's
//! `ga:publicationStatus`, in the accent colour along the page's start edge,
//! as on a specification.
use {
    crate::publication::{
        governance,
        render::{markup::html_escape as esc, model::Plan},
    },
    anyhow::{Result, bail, ensure},
    oxrdf::Term,
};

pub(in crate::publication::render::website) struct StatusBand(Option<String>);

impl StatusBand {
    /// The band text, or no band when the theme does not select one or the
    /// target declares no publication status.
    pub async fn read(plan: &Plan, selected: bool) -> Result<Self> {
        if !selected {
            return Ok(Self(None));
        }
        let status = match plan
            .source
            .optional(&plan.target, "publicationStatus")
            .await?
        {
            None => return Ok(Self(None)),
            Some(Term::Literal(status)) => status.value().trim().to_owned(),
            Some(_) => bail!("website publicationStatus must be a literal"),
        };
        ensure!(
            !status.is_empty(),
            "website publicationStatus cannot be empty"
        );
        let text = match plan.source.optional(&plan.target, "publisher").await?
        {
            None => status,
            Some(Term::NamedNode(publisher)) => {
                format!(
                    "{} {status}",
                    governance::acronym(&plan.source, &publisher).await?
                )
            },
            Some(_) => bail!("website publisher must be an IRI"),
        };
        Ok(Self(Some(text)))
    }

    /// Whether pages carry the band.
    pub fn shown(&self) -> bool { self.0.is_some() }

    pub fn html(&self) -> String {
        self.0
            .as_ref()
            .map(|text| {
                format!(
                    "<div class=\"status-band\" role=\"note\">{}</div>",
                    esc(text)
                )
            })
            .unwrap_or_default()
    }
}
