//! The declared calendar date of a publication target.
use {
    super::model::Plan,
    anyhow::{Result, anyhow, bail, ensure},
    chrono::{Datelike, NaiveDate},
    oxrdf::Term,
};

/// A validated `ga:publicationDate`, an `xsd:date` in `YYYY-MM-DD` form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PublicationDate(NaiveDate);

impl PublicationDate {
    /// Read the target's declared publication date, if any.
    pub(super) async fn read(plan: &Plan) -> Result<Option<Self>> {
        match plan
            .source
            .optional(&plan.target, "publicationDate")
            .await?
        {
            None => Ok(None),
            Some(Term::Literal(date))
                if date.datatype().as_str() ==
                    "http://www.w3.org/2001/XMLSchema#date" =>
            {
                Self::parse(date.value()).map(Some)
            },
            Some(_) => bail!("publicationDate must be an xsd:date"),
        }
    }

    fn parse(value: &str) -> Result<Self> {
        let date =
            NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| {
                anyhow!("publicationDate must be a valid YYYY-MM-DD date")
            })?;
        ensure!(
            date.format("%Y-%m-%d").to_string() == value,
            "publicationDate must use YYYY-MM-DD"
        );
        Ok(Self(date))
    }

    /// The canonical `YYYY-MM-DD` lexical form.
    pub(super) fn iso(self) -> String { self.0.format("%Y-%m-%d").to_string() }

    /// The English long form, for example `24 October 2026`.
    pub(super) fn long(self) -> String {
        self.0.format("%-d %B %Y").to_string()
    }

    /// A Typst `datetime` value for document metadata.
    pub(super) fn typst(self) -> String {
        format!(
            "datetime(year: {}, month: {}, day: {})",
            self.0.year(),
            self.0.month(),
            self.0.day()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::PublicationDate;

    #[test]
    fn dates_keep_their_canonical_calendar_form() -> anyhow::Result<()> {
        let date = PublicationDate::parse("2026-10-24")?;
        assert_eq!(date.iso(), "2026-10-24");
        assert_eq!(date.long(), "24 October 2026");
        assert_eq!(
            date.typst(),
            "datetime(year: 2026, month: 10, day: 24)"
        );
        assert!(PublicationDate::parse("2026-02-30").is_err());
        assert!(PublicationDate::parse("2026-1-04").is_err());
        Ok(())
    }
}
