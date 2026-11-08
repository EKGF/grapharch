//! HTML and print projections consume the retained SHACL view, never source
//! RDF.
use {
    crate::publication::document::{
        class_diagram::svg,
        shacl_class_view::diagram::Projection,
    },
    anyhow::{Result, ensure},
    sha2::{Digest, Sha256},
};

pub(super) fn html(body: &str) -> Result<String> {
    let projection = Projection::parse(body)?;
    let (drawing, width, _) = drawing(&projection)?;
    Ok(format!(
        "<figure class=\"shacl-validation-diagram\"><div tabindex=\"0\" \
         role=\"region\" aria-label=\"SHACL validation view; full constraints \
         follow in the table\" style=\"overflow-x:auto\"><div \
         style=\"width:{}px;max-width:100%;min-width:{}px\">{drawing}</div></\
         div><figcaption>SHACL validation view: {}. {}</figcaption></figure>",
        width,
        width.min(400.0),
        svg::xml(&projection.label),
        super::diagram_notes::reference_html()
    ))
}

pub(super) fn typst(body: &str) -> Result<String> {
    let projection = Projection::parse(body)?;
    let (drawing, width, height) = drawing(&projection)?;
    Ok(format!(
        "#block(breakable: false)[#layout(size => image(bytes({}), format: \
         \"svg\", width: calc.min(size.width, {}pt), alt: {}))]\n{}",
        super::markup::quote(&drawing),
        360.0 * width / height,
        super::markup::quote(&format!(
            "SHACL validation view {}",
            projection.label
        )),
        super::diagram_notes::reference_typst()
    ))
}

fn drawing(projection: &Projection) -> Result<(String, f64, f64)> {
    let document = roxmltree::Document::parse(&projection.svg)?;
    let root = document.root_element();
    let dimensions: Vec<f64> = root
        .attribute("viewBox")
        .ok_or_else(|| anyhow::anyhow!("SHACL SVG has no viewBox"))?
        .split_whitespace()
        .take(5)
        .map(str::parse)
        .collect::<std::result::Result<_, _>>()?; // bounded: SVG viewBox must contain exactly four numbers.
    ensure!(
        dimensions.len() == 4 &&
            dimensions.iter().all(|number| number.is_finite()) &&
            dimensions[2] > 0.0 &&
            dimensions[3] > 0.0,
        "invalid SHACL SVG dimensions"
    );
    let prefix = format!(
        "shacl-{}-",
        hex::encode(Sha256::digest(projection.shape.as_bytes()))
    );
    let mut drawing = projection.svg.clone();
    for id in document
        .descendants()
        .filter_map(|node| node.attribute("id"))
    {
        drawing = drawing
            .replace(
                &format!("id=\"{id}\""),
                &format!("id=\"{prefix}{id}\""),
            )
            .replace(
                &format!("url(#{id})"),
                &format!("url(#{prefix}{id})"),
            );
    }
    if let Some(style) = root.attribute("style") {
        drawing = drawing.replacen(
            &format!("style=\"{style}\""),
            "style=\"display:block;max-width:100%;height:auto\"",
            1,
        );
    }
    Ok((drawing, dimensions[2], dimensions[3]))
}
