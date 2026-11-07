//! MkDocs' native leading key/value metadata is source syntax, not body prose.
use {
    anyhow::{Result, ensure},
    serde_yaml::{Mapping, Value},
};

pub(crate) fn normalize(source: &str) -> Result<String> {
    if source.starts_with("---\n") {
        return Ok(source.to_owned());
    }
    let Some((header, body)) = source.split_once("\n\n") else {
        return Ok(source.to_owned());
    };
    let Some((key, _)) =
        header.lines().next().unwrap_or_default().split_once(':')
    else {
        return Ok(source.to_owned());
    };
    if !valid_key(key) {
        return Ok(source.to_owned());
    }
    ensure!(
        header.len() <= 16 * 1024,
        "MkDocs page metadata exceeds16KiB"
    );
    let mut fields = Mapping::new(); // bounded: source header16KiB and maximum64keys.
    let mut last = None;
    for line in header.lines() {
        if line.starts_with([' ', '\t']) {
            let key = last.as_ref().ok_or_else(|| {
                anyhow::anyhow!("metadata continuation requires a key")
            })?;
            let Value::String(value) = fields.get_mut(key).unwrap() else {
                unreachable!()
            };
            value.push(' ');
            value.push_str(line.trim());
            continue;
        }
        let (key, value) = line
            .split_once(':')
            .ok_or_else(|| anyhow::anyhow!("invalid MkDocs page metadata"))?;
        ensure!(
            valid_key(key),
            "invalid MkDocs metadata key {key}"
        );
        let key = Value::String(key.to_lowercase());
        ensure!(
            fields.len() < 64 && !fields.contains_key(&key),
            "duplicate metadata key or more than64keys"
        );
        fields.insert(
            key.clone(),
            Value::String(value.trim().to_owned()),
        );
        last = Some(key);
    }
    Ok(format!(
        "---\n{}---\n{body}",
        serde_yaml::to_string(&fields)?
    ))
}
fn valid_key(key: &str) -> bool {
    !key.is_empty() &&
        key.len() <= 128 &&
        key.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
