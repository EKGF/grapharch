use {
    super::Containers,
    crate::publication::content_blocks::{Boundary, TableColumn},
};

fn json(boundary: &Boundary) -> String {
    serde_json::to_string(boundary).unwrap()
}

fn table(columns: &[(&str, u8)]) -> Boundary {
    Boundary::Table {
        columns: columns
            .iter()
            .map(|(title, percent)| {
                TableColumn { title: (*title).into(), percent: *percent }
            })
            .collect(),
    }
}

#[test]
fn table_cells_form_rows_in_html_and_typst() {
    let figure = Boundary::TableFigure {
        caption: "Example Details".into(),
        label:   "ga-table-1".into(),
    };
    let sequence = [
        json(&figure),
        json(&table(&[("Name", 40), ("Value", 60)])),
        json(&Boundary::Cell),
        json(&Boundary::End),
        json(&Boundary::Cell),
        json(&Boundary::End),
        json(&Boundary::End),
        json(&Boundary::End),
    ];
    let mut html = Containers::default();
    let rendered: String = sequence
        .iter()
        .map(|source| html.html(source).unwrap())
        .collect();
    html.finish().unwrap();
    assert!(rendered.contains(
        "<figure class=\"ga-table-figure\" \
         id=\"ga-table-1\"><figcaption>Example Details</figcaption>"
    ));
    assert!(
        rendered.contains("<col style=\"width:40%\"><col style=\"width:60%\">")
    );
    assert!(
        rendered.contains(
            "<tr><td>\n</td>\n<td>\n</td></tr>\n</tbody></table></div>"
        )
    );
    let mut typst = Containers::default();
    let rendered: String = sequence
        .iter()
        .map(|source| typst.typst(source).unwrap())
        .collect();
    typst.finish().unwrap();
    assert!(rendered.contains("numbering: ga-table-numbering"));
    assert!(rendered.contains("#ga-table(columns: (40%, 60%,)"));
    assert!(rendered.ends_with("[],\n)\n]#label(\"ga-table-1\")]\n"));
}

#[test]
fn tables_hold_only_complete_rows_of_cells() {
    let mut containers = Containers::default();
    containers
        .html(&json(&table(&[("Name", 50), ("Value", 50)])))
        .unwrap();
    assert!(
        containers.html(&json(&Boundary::Caption)).is_err(),
        "a table holds only cells"
    );
    let mut containers = Containers::default();
    containers
        .html(&json(&table(&[("Name", 50), ("Value", 50)])))
        .unwrap();
    containers.html(&json(&Boundary::Cell)).unwrap();
    containers.html(&json(&Boundary::End)).unwrap();
    assert!(
        containers.html(&json(&Boundary::End)).is_err(),
        "half a row"
    );
    assert!(Containers::default().html(&json(&Boundary::Cell)).is_err());
    assert!(
        Boundary::parse(&json(&table(&[("Name", 50), ("Value", 40)]))).is_err(),
        "widths must total 100 percent"
    );
}
