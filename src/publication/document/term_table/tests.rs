use super::pattern::TextPattern;

#[test]
fn caption_patterns_need_exactly_the_title() {
    assert_eq!(
        TextPattern::caption("{title} Metadata")
            .unwrap()
            .text("Commons X"),
        "Commons X Metadata"
    );
    for invalid in [
        "Metadata",
        "{title} {table}",
        "{title} {other}",
        "{title}\nx",
    ] {
        assert!(
            TextPattern::caption(invalid).is_err(),
            "{invalid}"
        );
    }
}

#[test]
fn lead_ins_escape_text_and_insert_the_reference() {
    let pattern =
        TextPattern::lead_in("Metadata for the {title} is given in {table}.")
            .unwrap();
    assert_eq!(
        pattern.markdown("A_B ontology", "[T](urn:grapharch:table:x)"),
        "Metadata for the A\\_B ontology is given in \
         [T](urn:grapharch:table:x)."
    );
    assert!(TextPattern::lead_in("no reference to {title}").is_err());
    assert!(TextPattern::lead_in("{table} and {table}").is_err());
}
