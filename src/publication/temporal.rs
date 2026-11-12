//! Readable presentation of typed `xsd:date` and `xsd:dateTime` literals.
//!
//! Documentors write a temporal value as a semantic Markdown link,
//! `[8 April 2020](urn:grapharch:time:2020-04-08)`, so every renderer keeps
//! the exact lexical value: HTML as `<time datetime>`, PDF as text that
//! renders every lexical component (date, time, fraction, and zone). Only
//! typed literals are interpreted; an untyped string is never guessed to be a
//! date.
//!
//! A date-time at exactly midnight still shows its time. `xsd:dateTime`
//! asserts an instant, and the author chose it over `xsd:date`; presenting it
//! as a date would misstate the source type and, with a zone offset, the day.
use {
    anyhow::{Result, ensure},
    chrono::NaiveDate,
    oxrdf::Literal,
};

/// Semantic Markdown link prefix of a rendered temporal value.
pub(crate) const PREFIX: &str = "urn:grapharch:time:";

const XSD_DATE: &str = "http://www.w3.org/2001/XMLSchema#date";
const XSD_DATE_TIME: &str = "http://www.w3.org/2001/XMLSchema#dateTime";

/// A validated temporal literal with its exact lexical form and English,
/// day-month-year presentation (the `ga:publicationDate` convention).
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct TemporalLiteral {
    lexical: String,
    display: String,
}

impl TemporalLiteral {
    /// Interpret an `xsd:date` or `xsd:dateTime` literal. Other datatypes
    /// return `None`; an invalid or unsupported lexical form is an error.
    pub(crate) fn from_literal(literal: &Literal) -> Result<Option<Self>> {
        let lexical = literal.value();
        let display = match literal.datatype().as_str() {
            XSD_DATE => Self::date(lexical)?,
            XSD_DATE_TIME => Self::date_time(lexical)?,
            _ => return Ok(None),
        };
        Ok(Some(Self {
            lexical: lexical.to_owned(),
            display,
        }))
    }

    /// The semantic Markdown form consumed by the renderers.
    pub(crate) fn markdown(&self) -> String {
        format!("[{}]({PREFIX}{})", self.display, self.lexical)
    }

    fn date(lexical: &str) -> Result<String> {
        let (date, zone) = split_zone(lexical, 10)?;
        let date = calendar_date(date, lexical)?;
        Ok(match zone {
            Some(zone) => format!("{date} ({zone})"),
            None => date,
        })
    }

    fn date_time(lexical: &str) -> Result<String> {
        let (value, zone) = split_zone(lexical, 19)?;
        let (date, time) = value.split_once('T').ok_or_else(|| {
            anyhow::anyhow!("xsd:dateTime {lexical} requires a T separator")
        })?;
        let date = calendar_date(date, lexical)?;
        let (seconds, fraction) = match time.split_once('.') {
            Some((seconds, fraction)) => (seconds, Some(fraction)),
            None => (time, None),
        };
        let parts = seconds.split(':').collect::<Vec<_>>();
        ensure!(
            parts.len() == 3 &&
                parts.iter().all(|p| {
                    p.len() == 2 && p.bytes().all(|b| b.is_ascii_digit())
                }) &&
                fraction.is_none_or(|f| {
                    !f.is_empty() && f.bytes().all(|b| b.is_ascii_digit())
                }),
            "xsd:dateTime {lexical} requires an hh:mm:ss[.s] time"
        );
        let [hour, minute, second] = [parts[0], parts[1], parts[2]]
            .map(|p| p.parse::<u8>().unwrap_or(99));
        let end_of_day = hour == 24 &&
            minute == 0 &&
            second == 0 &&
            fraction.is_none_or(|f| f.bytes().all(|b| b == b'0'));
        ensure!(
            (hour < 24 && minute < 60 && second < 60) || end_of_day,
            "xsd:dateTime {lexical} has an invalid time of day"
        );
        let mut display = format!("{date}, {time}");
        if let Some(zone) = zone {
            display.push(' ');
            display.push_str(&zone);
        }
        Ok(display)
    }
}

/// Destination's exact lexical value, when it is a temporal link.
pub(crate) fn lexical(destination: &str) -> Option<&str> {
    destination.strip_prefix(PREFIX).filter(|value| {
        !value.is_empty() &&
            value.len() <= 64 &&
            value.bytes().all(|byte| {
                byte.is_ascii_digit() ||
                    matches!(byte, b'-' | b':' | b'.' | b'T' | b'Z' | b'+')
            })
    })
}

/// Split a trailing `Z` or `±hh:mm` zone after at least `minimum` bytes.
fn split_zone(lexical: &str, minimum: usize) -> Result<(&str, Option<String>)> {
    ensure!(
        lexical.is_ascii() && lexical.len() >= minimum,
        "temporal literal {lexical} is too short"
    );
    if let Some(value) = lexical.strip_suffix('Z') {
        return Ok((value, Some("UTC".into())))
    }
    let tail = &lexical[minimum..];
    let Some(sign) = tail.rfind(['+', '-']) else {
        return Ok((lexical, None))
    };
    let (value, offset) = lexical.split_at(minimum + sign);
    let digits = &offset[1..];
    ensure!(
        digits.len() == 5 &&
            digits.as_bytes()[2] == b':' &&
            digits
                .bytes()
                .enumerate()
                .all(|(i, b)| i == 2 || b.is_ascii_digit()),
        "temporal literal {lexical} has an invalid zone offset"
    );
    let hours = digits[..2].parse::<u8>()?;
    let minutes = digits[3..].parse::<u8>()?;
    ensure!(
        minutes < 60 && (hours < 14 || (hours == 14 && minutes == 0)),
        "temporal literal {lexical} has an out-of-range zone offset"
    );
    Ok((value, Some(format!("UTC{offset}"))))
}

/// `8 April 2020` for a four-digit-year `YYYY-MM-DD` date.
fn calendar_date(date: &str, lexical: &str) -> Result<String> {
    let parsed = (date.len() == 10)
        .then(|| NaiveDate::parse_from_str(date, "%Y-%m-%d").ok())
        .flatten()
        .filter(|parsed| parsed.format("%Y-%m-%d").to_string() == date)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "temporal literal {lexical} requires a valid YYYY-MM-DD \
                 calendar date with a four-digit year"
            )
        })?;
    Ok(parsed.format("%-d %B %Y").to_string())
}

#[cfg(test)]
mod tests {
    use {
        super::{TemporalLiteral, lexical},
        oxrdf::{Literal, NamedNode},
    };

    fn typed(value: &str, datatype: &str) -> Literal {
        Literal::new_typed_literal(
            value,
            NamedNode::new_unchecked(format!(
                "http://www.w3.org/2001/XMLSchema#{datatype}"
            )),
        )
    }

    fn display(value: &str, datatype: &str) -> anyhow::Result<String> {
        Ok(
            TemporalLiteral::from_literal(&typed(value, datatype))?
                .expect("temporal literal")
                .display,
        )
    }

    #[test]
    fn dates_and_date_times_render_in_english_day_month_year()
    -> anyhow::Result<()> {
        assert_eq!(display("2020-04-08", "date")?, "8 April 2020");
        assert_eq!(
            display("2020-04-08Z", "date")?,
            "8 April 2020 (UTC)"
        );
        assert_eq!(
            display("2020-04-08-05:00", "date")?,
            "8 April 2020 (UTC-05:00)"
        );
        assert_eq!(
            display("2020-04-08T00:00:01Z", "dateTime")?,
            "8 April 2020, 00:00:01 UTC"
        );
        assert_eq!(
            display("2020-06-20T12:36:22.5+02:00", "dateTime")?,
            "20 June 2020, 12:36:22.5 UTC+02:00"
        );
        assert_eq!(
            display("2020-04-08T00:00:00", "dateTime")?,
            "8 April 2020, 00:00:00",
            "midnight keeps its time; a date-time is not a date"
        );
        Ok(())
    }

    #[test]
    fn markdown_keeps_the_exact_lexical_value() -> anyhow::Result<()> {
        let value = TemporalLiteral::from_literal(&typed(
            "2020-04-08T00:00:01Z",
            "dateTime",
        ))?
        .expect("temporal literal");
        let markdown = value.markdown();
        assert_eq!(
            markdown,
            "[8 April 2020, 00:00:01 \
             UTC](urn:grapharch:time:2020-04-08T00:00:01Z)"
        );
        assert_eq!(
            lexical("urn:grapharch:time:2020-04-08T00:00:01Z"),
            Some("2020-04-08T00:00:01Z")
        );
        assert_eq!(lexical("urn:grapharch:time:<script>"), None);
        Ok(())
    }

    #[test]
    fn other_literals_are_not_interpreted() -> anyhow::Result<()> {
        assert!(
            TemporalLiteral::from_literal(&Literal::new_simple_literal(
                "2020-04-08"
            ))?
            .is_none()
        );
        Ok(())
    }

    #[test]
    fn invalid_temporal_lexical_forms_fail() {
        for (value, datatype) in [
            ("2020-02-30", "date"),
            ("2020-4-8", "date"),
            ("20200-04-08", "date"),
            ("2020-04-08", "dateTime"),
            ("2020-04-08T25:00:00Z", "dateTime"),
            ("2020-04-08T10:00:00+15:00", "dateTime"),
            ("2020-04-08T10:00Z", "dateTime"),
        ] {
            assert!(
                TemporalLiteral::from_literal(&typed(value, datatype)).is_err(),
                "{value} must fail as xsd:{datatype}"
            );
        }
    }
}
