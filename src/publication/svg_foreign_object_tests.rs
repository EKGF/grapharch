//! Attack vectors and draw.io label content for `<foreignObject>`.
use crate::publication::image_assets::validate;

fn svg(foreign: &str) -> String {
    format!(
        "<svg xmlns='http://www.w3.org/2000/svg'><switch>{foreign}\
         </switch></svg>"
    )
}

fn label(body: &str) -> String {
    svg(&format!(
        "<foreignObject width='100%' height='100%' pointer-events='none' \
         style='overflow: visible; text-align: left;'>\
         <div xmlns='http://www.w3.org/1999/xhtml'>{body}</div>\
         </foreignObject>"
    ))
}

#[test]
fn drawio_html_labels_are_accepted() {
    let source = svg(
        "<foreignObject pointer-events='none' width='100%' height='100%' \
         requiredFeatures='http://www.w3.org/TR/SVG11/feature#Extensibility' \
         style='overflow: visible; text-align: left;'>\
         <div xmlns='http://www.w3.org/1999/xhtml' style='display: flex; \
         align-items: unsafe center; width: 118px; height: 1px; \
         padding-top: 30px; margin-left: 1px;'><div \
         style='box-sizing: border-box; font-size: 0; text-align: center;'>\
         <div style='display: inline-block; font-size: 12px; \
         font-family: Helvetica; color: rgb(0, 0, 0); \
         white-space: normal; overflow-wrap: normal;'>Order <b>Service</b>\
         <br/><font color='#ff0000'>v2</font></div></div></div>\
         </foreignObject>",
    );
    validate(source.as_bytes(), "image/svg+xml").unwrap();
}

#[test]
fn formatted_xhtml_labels_are_accepted() {
    let source = label(
        "<p style='margin: 0'><b>Bold</b> <i>italic</i><br/><font \
         color='#333' face='Helvetica' \
         size='2'>font</font></p><ul><li>item</li></ul><table><tr><td \
         colspan='2'>cell</td></tr></table>",
    );
    validate(source.as_bytes(), "image/svg+xml").unwrap();
}

#[test]
fn foreign_object_attack_vectors_are_rejected() {
    for body in [
        "<script>alert(1)</script>",
        "<span onclick='a()'>x</span>",
        "<span onmouseover='a()'>x</span>",
        "<img src='https://example.org/a.png'/>",
        "<img src='x' onerror='a()'/>",
        "<iframe src='https://example.org/'></iframe>",
        "<object data='a.svg'></object>",
        "<embed src='a.swf'/>",
        "<a href='javascript:alert(1)'>x</a>",
        "<a href='https://example.org/'>x</a>",
        "<form action='https://example.org/'><input/></form>",
        "<link rel='stylesheet' href='https://example.org/a.css'/>",
        "<style>div{background:url(https://example.org/a)}</style>",
        "<meta http-equiv='refresh' content='0;url=https://example.org/'/>",
        "<base href='https://example.org/'/>",
        "<video src='a.mp4'></video>",
        "<svg xmlns='http://www.w3.org/2000/svg'><script>a()</script></svg>",
        "<span style='background:url(https://example.org/a.png)'>x</span>",
        "<span style='background: URL( //example.org/a.png )'>x</span>",
        "<span style='background:url(#local)'>x</span>",
        "<span style='background-image:image-set(\"a.png\" 1x)'>x</span>",
        r"<span style='background:u\72l(//example.org/a)'>x</span>",
        "<span style='width:expression(alert(1))'>x</span>",
        "<span style='-moz-binding:url(a.xml#x)'>x</span>",
        "<span style='behavior:url(a.htc)'>x</span>",
        "<span style='x:1;@import \"a.css\"'>x</span>",
        "<span class='a' formaction='https://example.org/'>x</span>",
        "<span xmlns:x='http://www.w3.org/1999/xlink' \
         x:href='https://example.org/'>x</span>",
        "<font face='javascript:a()'>x</font>",
        "<math xmlns='http://www.w3.org/1998/Math/MathML'><mi>x</mi></math>",
        "<custom-element>x</custom-element>",
    ] {
        let source = label(body);
        assert!(
            validate(source.as_bytes(), "image/svg+xml").is_err(),
            "{body}"
        );
    }
}

#[test]
fn foreign_object_attributes_are_allowlisted() {
    for foreign in [
        "<foreignObject onload='a()'/>",
        "<foreignObject requiredExtensions='https://example.org/x'/>",
        "<foreignObject requiredFeatures='https://example.org/feature'/>",
        "<foreignObject style='background:url(https://example.org/a)'/>",
        "<foreignObject xmlns:x='http://www.w3.org/1999/xlink' \
         x:href='https://example.org/'/>",
        "<foreignObject><p>XHTML namespace is required</p></foreignObject>",
    ] {
        let source = svg(foreign);
        assert!(
            validate(source.as_bytes(), "image/svg+xml").is_err(),
            "{foreign}"
        );
    }
}
