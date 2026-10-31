//! Literal prose and explicit source links; source strings never become markup.
use super::{
    super::markup::{html_escape, quote},
    evidence::Evidence,
};

pub(super) fn html(entries: &[Evidence]) -> String {
    let mut output = String::new();
    for entry in entries {
        output.push_str(&format!(
            "<div class=\"quality-evidence\"><p><strong>{}</strong> <span>— \
             {}</span></p><p>{}</p><p>Source: ",
            html_escape(entry.field()),
            html_escape(&entry.language()),
            html_escape(entry.value.value()).replace('\n', "<br>")
        ));
        let source = entry.source_text();
        if let Some(target) = target(source) {
            output.push_str(&format!(
                "<a href=\"{}\">{}</a>",
                html_escape(&target),
                html_escape(source)
            ));
        } else {
            output.push_str(&html_escape(source));
        }
        output.push_str("</p></div>");
    }
    output
}

pub(super) fn markdown(entries: &[Evidence]) -> String {
    let mut output = String::new();
    for entry in entries {
        output.push_str(&format!(
            "> **{} — {}**\n>\n",
            escape(entry.field()),
            escape(&entry.language())
        ));
        for line in entry.value.value().lines() {
            output.push_str(&format!("> {}\n", escape(line)));
        }
        let source = entry.source_text();
        if let Some(target) = target(source) {
            let target = target
                .replace('>', "%3E")
                .replace('<', "%3C")
                .replace('\n', "%0A");
            output.push_str(&format!(
                ">\n> Source: [{}](<{target}>)\n>\n",
                escape(source)
            ));
        } else {
            output.push_str(&format!(">\n> Source: {}\n>\n", escape(source)));
        }
    }
    output
}

pub(super) fn typst(entries: &[Evidence]) -> String {
    let mut output = String::new();
    for entry in entries {
        output.push_str(&format!(
            "#parbreak()#v(0.6em)#strong[#text({})] — \
             #text({})#parbreak()#text({})#parbreak()Source: ",
            quote(entry.field()),
            quote(&entry.language()),
            quote(entry.value.value())
        ));
        let source = entry.source_text();
        if let Some(target) = target(source) {
            output.push_str(&format!(
                "#link({})[#text({})]",
                quote(&target),
                quote(&wrappable(source))
            ));
        } else {
            output.push_str(&format!("#text({})", quote(&wrappable(source))));
        }
        output.push_str("#parbreak()\n");
    }
    output
}

fn target(source: &str) -> Option<String> {
    if ["https://", "http://"]
        .iter()
        .any(|scheme| source.starts_with(scheme))
    {
        return Some(source.into());
    }

    None
}

fn escape(value: &str) -> String {
    let mut output = String::new();
    for ch in value.chars() {
        if "\\`*_{}[]()#+-.!|>".contains(ch) {
            output.push('\\');
        }
        match ch {
            '<' => output.push_str("&lt;"),
            '&' => output.push_str("&amp;"),
            _ => output.push(ch),
        }
    }
    output
}

pub(super) fn wrappable(value: &str) -> String {
    // Invisible legal line-break opportunities preserve the full identifier.
    let mut output = String::new();
    for (index, ch) in value.chars().enumerate() {
        output.push(ch);
        if "/#_-:.".contains(ch) || index % 24 == 23 {
            output.push('\u{200b}');
        }
    }
    output
}
