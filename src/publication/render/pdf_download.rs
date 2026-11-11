//! Consistent, explicitly labelled PDF downloads across publication surfaces.
use {super::markup::html_escape, crate::publication::href::Href};

pub(super) fn link(href: &str, label: &str, class: &str) -> String {
    format!(
        "<a class=\"{}\" href=\"{}\" type=\"application/pdf\" \
         aria-label=\"Download {}\"><svg aria-hidden=\"true\" \
         focusable=\"false\" width=\"26\" height=\"30\" viewBox=\"0 0 28 \
         32\"><path d=\"M5 1h13l8 8v22H5zM18 1v8h8\" fill=\"white\" \
         stroke=\"currentColor\" stroke-width=\"1.5\"/><rect x=\"0\" y=\"17\" \
         width=\"25\" height=\"12\" rx=\"2\" fill=\"#c62828\"/><text \
         x=\"12.5\" y=\"26\" text-anchor=\"middle\" fill=\"white\" \
         font-family=\"Arial,sans-serif\" font-weight=\"bold\" \
         font-size=\"9\">PDF</text></svg><span>{}</span></a>",
        html_escape(class),
        html_escape(href),
        html_escape(label),
        html_escape(label)
    )
}

pub(super) fn metadata_links(html: &str) -> String {
    let document = scraper::Html::parse_fragment(html);
    let selector =
        scraper::Selector::parse("a[href]").expect("static selector");
    // bounded: metadata contains at most 64 rows, each capped at 64 KiB.
    let mut seen = std::collections::HashSet::new();
    let mut downloads = String::new();
    for anchor in document.select(&selector) {
        let href = anchor.value().attr("href").expect("selected href");
        if Href::parse(href).is_ok_and(|href| href.is_pdf()) &&
            seen.insert(href)
        {
            let label = anchor.text().collect::<String>();
            downloads.push_str(&link(href, &label, "pdf-download"));
        }
    }
    downloads
}

#[cfg(test)]
mod tests {
    #[test]
    fn relative_pdf_links_get_download_controls_with_the_same_href() {
        let downloads = super::metadata_links(
            "<dd><a href=\"../pdf/ekgf/document.pdf\">EKGF</a><br><a \
             href=\"https://example.org/omg.pdf\">OMG</a><a \
             href=\"../index.html\">Home</a></dd>",
        );
        assert_eq!(
            downloads.matches("class=\"pdf-download\"").count(),
            2
        );
        assert!(downloads.contains("href=\"../pdf/ekgf/document.pdf\""));
        assert!(downloads.contains("href=\"https://example.org/omg.pdf\""));
    }
}
