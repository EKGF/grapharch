//! Wrap authored phrases on word boundaries; preserve compact identifiers.
use super::escape::Escaped;

pub(super) fn render(value: &str) -> String {
    if let Some((prefix, local)) = value.split_once(':') &&
        !prefix.chars().any(char::is_whitespace)
    {
        return format!("{}:\\n{}", Escaped::inline(prefix), render(local));
    }
    if !value.chars().any(char::is_whitespace) {
        return identifier(value);
    }
    let mut lines = Vec::new(); // bounded: source labels have at most 512 bytes.
    let mut line = String::new();
    for word in value.split_whitespace() {
        let size = word.chars().count();
        if !line.is_empty() && line.chars().count() + 1 + size > 24 {
            lines.push(Escaped::inline(&line).to_string());
            line.clear();
        }
        if size > 24 {
            lines.push(identifier(word));
        } else {
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
    }
    if !line.is_empty() {
        lines.push(Escaped::inline(&line).to_string());
    }
    lines.join("\\n")
}

fn identifier(value: &str) -> String {
    let mut lines = Vec::new(); // bounded: source labels have at most 512 bytes.
    let mut line = String::new();
    let mut length = 0;
    for character in value.chars() {
        if length == 24 || (length >= 10 && character.is_uppercase()) {
            lines.push(Escaped::inline(&line).to_string());
            line.clear();
            length = 0;
        }
        line.push(character);
        length += 1;
    }
    lines.push(Escaped::inline(&line).to_string());
    lines.join("\\n")
}
