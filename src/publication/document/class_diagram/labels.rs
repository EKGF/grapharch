//! Source-owned display labels with bounded language selection and local
//! identity disambiguation.
use {
    crate::publication::view::RevisionView,
    anyhow::{Result, ensure},
    oxrdf::{NamedNode, Term},
    std::collections::{BTreeMap, BTreeSet},
};

pub(super) struct Label {
    pub iri:   String,
    pub qname: String,
    pub text:  String,
}

impl Label {
    pub async fn read(
        view: &RevisionView,
        id: &NamedNode,
        qname: String,
        language: &str,
    ) -> Result<Self> {
        let mut selection = Selection::default();
        let mut rows = view
            .objects_by(
                id,
                &NamedNode::new_unchecked(
                    "http://www.w3.org/2000/01/rdf-schema#label",
                ),
            )
            .await?;
        while let Some(row) = rows.next().await? {
            selection.accept(&row["value"], language)?;
        }
        let text = selection
            .best
            .map(|(_, text)| text)
            .unwrap_or_else(|| qname.clone());
        Ok(Self { iri: id.as_str().into(), qname, text })
    }
}

#[derive(Default)]
struct Selection {
    count: usize,
    best:  Option<(u8, String)>,
}
impl Selection {
    fn accept(&mut self, value: &Term, language: &str) -> Result<()> {
        self.count += 1;
        ensure!(
            self.count <= 64,
            "class diagram resource exceeds 64 label budget"
        );
        ensure!(
            !language.is_empty() && language.len() <= 64,
            "invalid diagram label language"
        );
        let Term::Literal(literal) = value else {
            anyhow::bail!("rdfs:label must be a literal");
        };
        ensure!(
            literal.language().is_some() ||
                literal.datatype().as_str() ==
                    "http://www.w3.org/2001/XMLSchema#string",
            "rdfs:label must be text"
        );
        ensure!(
            !literal.value().trim().is_empty() && literal.value().len() <= 512,
            "diagram label must be nonempty and at most 512 bytes"
        );
        let priority = match literal.language() {
            Some(tag) if tag.eq_ignore_ascii_case(language) => 0,
            Some(tag)
                if tag.to_ascii_lowercase().starts_with(&format!(
                    "{}-",
                    language.to_ascii_lowercase()
                )) =>
            {
                1
            },
            None => 2,
            Some(_) => return Ok(()),
        };
        // bounded: at most 512 source bytes, retaining one preferred label.
        let text = literal
            .value()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let candidate = (priority, text);
        if self.best.as_ref().is_none_or(|best| candidate < *best) {
            self.best = Some(candidate);
        }
        Ok(())
    }
}

pub(super) fn disambiguate(labels: &mut [Label]) -> Result<()> {
    ensure!(
        labels.len() <= 357,
        "diagram display label budget exceeded"
    );
    // bounded: one entry per admitted display label, at most 357 (focus + 36
    // endpoints + 64 shape identities + 256 field endpoints).
    let mut identities = BTreeMap::new();
    let mut groups: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for label in labels.iter() {
        ensure!(
            !label.text.is_empty() &&
                label.text.len() <= 512 &&
                !label.qname.is_empty() &&
                label.qname.len() <= 512 &&
                label.iri.len() <= 2048,
            "invalid diagram display label"
        );
        if let Some(previous) =
            identities.insert(&label.iri, (&label.qname, &label.text))
        {
            ensure!(
                previous == (&label.qname, &label.text),
                "inconsistent labels for the same diagram resource"
            );
        }
        groups
            .entry(label.text.clone())
            .or_default()
            .insert(label.iri.clone());
    }
    for label in labels.iter_mut() {
        if groups[&label.text].len() > 1 {
            label.text = format!("{} ({})", label.text, label.qname);
        }
    }
    groups.clear();
    for label in labels.iter() {
        groups
            .entry(label.text.clone())
            .or_default()
            .insert(label.iri.clone());
    }
    for label in labels.iter_mut() {
        if groups[&label.text].len() > 1 {
            label.text = format!("{} [{}]", label.text, label.iri);
        }
        ensure!(
            label.text.len() <= 512,
            "disambiguated diagram label exceeds 512 bytes"
        );
    }
    groups.clear();
    for label in labels.iter() {
        let ids = groups.entry(label.text.clone()).or_default();
        ids.insert(label.iri.clone());
        ensure!(
            ids.len() == 1,
            "diagram labels remain ambiguous after identity disambiguation"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use {super::*, oxrdf::Literal};

    fn text(value: &str, language: Option<&str>) -> Term {
        match language {
            Some(language) => {
                Literal::new_language_tagged_literal(value, language)
                    .unwrap()
                    .into()
            },
            None => Literal::new_simple_literal(value).into(),
        }
    }
    #[test]
    fn exact_requested_language_precedes_regional_and_neutral_labels()
    -> Result<()> {
        let mut selection = Selection::default();
        for value in [
            text("Neutral", None),
            text("Regional", Some("en-GB")),
            text("English", Some("en")),
            text("Français", Some("fr")),
        ] {
            selection.accept(&value, "en")?;
        }
        assert_eq!(selection.best.unwrap().1, "English");
        let mut french = Selection::default();
        french.accept(&text("English", Some("en")), "fr")?;
        french.accept(&text("Français", Some("fr")), "fr")?;
        assert_eq!(french.best.unwrap().1, "Français");
        Ok(())
    }
    #[test]
    fn neutral_selection_and_equal_priority_are_stable() -> Result<()> {
        let mut selection = Selection::default();
        for value in [
            text("Zulu", None),
            text("Alpha", None),
            text("Étiquette", Some("fr")),
        ] {
            selection.accept(&value, "en")?;
        }
        assert_eq!(selection.best.unwrap().1, "Alpha");
        let mut foreign = Selection::default();
        foreign.accept(&text("Étiquette", Some("fr")), "en")?;
        assert!(foreign.best.is_none());
        Ok(())
    }
    #[test]
    fn malformed_labels_and_source_budgets_fail() -> Result<()> {
        for value in [
            Term::NamedNode(NamedNode::new_unchecked("urn:not-a-literal")),
            text("  ", None),
            text(&"x".repeat(513), None),
            Literal::from(42).into(),
        ] {
            assert!(Selection::default().accept(&value, "en").is_err());
        }
        let mut selection = Selection::default();
        for _ in 0..64 {
            selection.accept(&text("Allowed", None), "en")?;
        }
        assert!(selection.accept(&text("Excess", None), "en").is_err());
        Ok(())
    }
    fn label(iri: &str, qname: &str, text: &str) -> Label {
        Label {
            iri:   iri.into(),
            qname: qname.into(),
            text:  text.into(),
        }
    }
    #[test]
    fn only_different_resources_with_colliding_labels_get_identifiers()
    -> Result<()> {
        let mut labels = [
            label("urn:a", "ex:a", "Party"),
            label("urn:b", "ex:b", "Party"),
            label("urn:c", "ex:c", "Contract"),
            label("urn:a", "ex:a", "Party"),
        ];
        disambiguate(&mut labels)?;
        assert_eq!(labels[0].text, "Party (ex:a)");
        assert_eq!(labels[1].text, "Party (ex:b)");
        assert_eq!(labels[2].text, "Contract");
        assert_eq!(labels[3].text, labels[0].text);
        Ok(())
    }
    #[test]
    fn collision_with_an_authored_qualified_name_remains_unambiguous()
    -> Result<()> {
        let mut labels = [
            label("urn:a", "ex:a", "Party"),
            label("urn:b", "ex:b", "Party"),
            label("urn:c", "ex:c", "Party (ex:a)"),
        ];
        disambiguate(&mut labels)?;
        assert_eq!(labels[0].text, "Party (ex:a) [urn:a]");
        assert_eq!(labels[2].text, "Party (ex:a) [urn:c]");
        Ok(())
    }
}
