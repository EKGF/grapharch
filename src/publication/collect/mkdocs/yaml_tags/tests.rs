use super::*;

#[test]
fn known_tags_become_data_and_unknown_tags_are_diagnostics() {
    let body = "a: !!python/name:material.extensions.emoji.to_svg\nb: \
                !!python/object/apply:pymdownx.slugs.slugify\n  kwds:\n    \
                case: lower\nc: !!python/name:os.system\nd: \
                !!python/object/apply:subprocess.call [ls]\ne: \
                '!!python/name:quoted' # !!python/name:comment\n";
    let untagged = Untagged::read(body);
    let value: Value = serde_yaml::from_str(&untagged.body).unwrap();
    let map = value.as_mapping().unwrap();
    assert!(
        PythonReference::EmojiToSvg.is(&map["a"]),
        "{:?}",
        map["a"]
    );
    assert_eq!(
        map["b"]["kwds"]["case"],
        Value::String("lower".into())
    );
    assert_eq!(map["c"], Value::Null);
    assert_eq!(map["d"][0], Value::String("ls".into()));
    assert_eq!(
        map["e"],
        Value::String("!!python/name:quoted".into())
    );
    assert_eq!(untagged.warnings.len(), 2);
    assert!(
        untagged
            .warnings
            .iter()
            .all(|w| w.code == WarningCode::UnknownYamlTag)
    );
}
