// Restrained submission presentation. This style does not confer approval,
// supply a document number, or insert an authority's legal terms or logo.
#set text(font: "Liberation Serif", size: 10pt, fill: black)
#set par(justify: true, leading: 0.55em, spacing: 0.8em)
#show heading: set text(font: "Liberation Sans", fill: black, weight: "bold")
#show heading.where(level: 1): set text(size: 18pt)
#show heading.where(level: 2): set text(size: 14pt)
#show heading.where(level: 3): set text(size: 12pt)
#show heading.where(level: 4): set text(size: 11pt)
#show heading.where(level: 4): set block(below: 10pt)
#let ga-cover(publisher, title, subtitle, front) = {
  set text(font: "Liberation Sans", fill: black)
  set par(justify: false)
  v(12mm)
  text(size: 12pt, publisher)
  v(30mm)
  text(size: 26pt, weight: "bold", title)
  v(8mm)
  subtitle
  v(10mm)
  line(length: 100%, stroke: 0.6pt + black)
  v(7mm)
  set text(size: 10pt)
  front
}
#set page(paper: "us-letter", margin: 1in, numbering: none,
  background: none, header: none,
  footer: context {
    if here().page() > 1 and page.numbering != none {
      set text(font: "Liberation Sans", size: 9pt, fill: black)
      grid(columns: (1fr, auto), column-gutter: 12pt,
        [#ga-running(ga-title, limit: 85)],
        counter(page).display(page.numbering))
    }
  })
#let ga-callout(label, body, kind: "note") = block(width: 100%, breakable: true,
  above: 10pt, below: 10pt, inset: 9pt, stroke: 0.4pt + ga-line)[
  #block(sticky: true, below: 5pt)[#strong[#label]]
  #body
]
#let ga-example(body) = {
  counter("ga-example").step()
  block(width: 100%, breakable: true, above: 10pt, below: 10pt,
    inset: 9pt, stroke: 0.4pt + ga-line)[
    #block(sticky: true, below: 5pt)[Example #context counter("ga-example").display()]
    #body
  ]
}
