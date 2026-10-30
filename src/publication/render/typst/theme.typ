// Presentation components shared by all generated content. No source text is evaluated.
#let ga-specification = ga-style == "specification"
#let ga-ink = rgb("#182b35")
#let ga-muted = rgb("#526670")
#let ga-line = rgb("#dce5e5")
#let ga-soft = ga-accent.lighten(94%)
#let ga-heading = if ga-specification { ga-accent } else { ga-ink }
#let ga-running(text, limit: 55) = if text.clusters().len() > limit { text.clusters().slice(0, limit - 3).join() + "..." } else { text }
#set text(font: "Liberation Sans", size: if ga-specification { 10pt } else { 10.5pt }, fill: ga-ink, lang: "en", hyphenate: false)
#set par(justify: false, leading: 0.7em, spacing: 0.8em)
#set list(indent: 1.2em, body-indent: 0.5em, spacing: 0.65em)
#set enum(indent: 1.5em, body-indent: 0.5em, spacing: 0.65em)
#set heading(numbering: "1.1")
#show heading: set block(sticky: true)
// Space belongs to the fragment container: heading above-spacing collapses
// when the heading is the first element of a breakable fragment block.
#let ga-section(level, body) = block(above: if level == 1 { 28pt } else if level == 2 { 21pt } else { 16pt }, breakable: true, body)
#show heading.where(level: 1): set text(size: if ga-specification { 19pt } else { 26pt }, weight: if ga-specification { "regular" } else { "bold" }, fill: ga-heading)
#show heading.where(level: 1): set block(above: 28pt, below: 16pt)
#show heading.where(level: 2): set text(size: if ga-specification { 13pt } else { 16pt }, weight: "bold", fill: ga-heading)
#show heading.where(level: 2): set block(above: 21pt, below: 11pt)
#show heading.where(level: 3): set text(size: 11.5pt, weight: "bold", fill: ga-heading)
#show heading.where(level: 3): set block(above: 14pt, below: 7pt)
#show heading.where(level: 4): set text(size: 10.5pt, weight: "bold", fill: ga-heading)
#show link: set text(fill: ga-accent)
#show raw: set text(font: "DejaVu Sans Mono", size: 8.2pt, hyphenate: false)
// Wrap code to the available measure; the quoted source remains unchanged.
#let ga-code-text(value) = layout(size => {
  let capacity = calc.max(1, calc.floor(size.width / measure(text("M")).width))
  for (index, line) in value.split("\n").enumerate() {
    if index > 0 { linebreak() }
    let clusters = line.clusters()
    for start in range(0, clusters.len(), step: capacity) {
      if start > 0 { linebreak() }
      text(clusters.slice(start, calc.min(start + capacity, clusters.len())).join())
    }
  }
})
#show raw.where(block: true): it => block(width: 100%, breakable: true, inset: (x: 10pt, y: 9pt), fill: ga-soft, stroke: (left: 2pt + ga-accent), radius: 3pt)[#set text(font: "DejaVu Sans Mono", size: 8.2pt)
#ga-code-text(it.text)]
#show quote.where(block: true): it => block(width: 100%, breakable: true, inset: (x: 11pt, y: 8pt), fill: ga-soft, stroke: (left: 2pt + ga-accent))[#it.body]
#let ga-table(columns: (), align: auto, has-header: true, ..cells) = block(above: 11pt, below: 12pt, breakable: true)[
  #set text(size: 8.5pt)
  #set par(leading: 0.55em, spacing: 0.4em)
  #table(columns: columns, align: align, inset: (x: 7pt, y: 6pt),
    stroke: if ga-specification { 0.4pt + ga-line } else { (x: none, y: 0.4pt + ga-line) },
    fill: (x, y) => if has-header and y == 0 { ga-soft }, ..cells)
]
#let ga-callout(label, body, kind: "note") = {
  let colors = (
    note: (rgb("#175a98"), rgb("#f1f6fc")),
    tip: (rgb("#176644"), rgb("#eff8f2")),
    important: (rgb("#68449c"), rgb("#f6f2fb")),
    warning: (rgb("#825600"), rgb("#fff8e7")),
    caution: (rgb("#aa3035"), rgb("#fff2f2")),
  )
  let (tint, background) = colors.at(kind)
  block(width: 100%, breakable: true, above: 10pt, below: 10pt, inset: (x: 11pt, y: 9pt), fill: background, stroke: (left: 2pt + tint))[
    #block(sticky: true, below: 6pt)[#text(size: 9pt, weight: "bold", fill: tint)[#label]]
    #body
  ]
}
#let ga-cover(publisher, title, subtitle, front) = {
  v(12mm)
  block(width: 100%, inset: (bottom: 6mm), stroke: (bottom: 0.6pt + ga-accent))[
    #text(size: 9pt, weight: "bold", fill: ga-accent, tracking: 0.9pt)[#text(publisher)]
  ]
  v(13mm)
  block(width: 100%)[#set par(leading: 0.4em)
    #text(size: if ga-specification { 34pt } else { 40pt }, weight: "bold", fill: ga-heading)[#text(title)]
  ]
  subtitle
  v(8mm)
  rect(width: 16mm, height: 1.5mm, fill: ga-accent, stroke: none)
  if front != [] {
    v(9mm)
    block(width: 100%, inset: (x: 6mm, y: 5mm), fill: ga-soft, radius: (top-right: 4mm, bottom-left: 4mm))[
      #set text(size: 9pt)
      #set par(leading: 0.6em, spacing: 0.7em)
      #front
    ]
  }
}
#set page(paper: "a4", margin: (left: 23mm, right: 23mm, top: 25mm, bottom: 23mm),
  background: context {
    if counter(page).get().first() == 1 {
      place(top + right, rect(width: 15mm, height: 100%, fill: ga-soft, stroke: none))
      place(bottom + left, rect(width: 100%, height: 3mm, fill: ga-accent, stroke: none))
    }
    if ga-ribbon != none {
      let ribbon-measure = measure(text(size: 9pt, weight: "bold", ga-ribbon)).width
      let ribbon-height = calc.min(160mm, calc.max(82mm, ribbon-measure + 12mm))
      place(top + left, block(width: 8mm, height: ribbon-height, fill: ga-accent)[
        #place(center + horizon, rotate(-90deg, reflow: true)[
          #text(size: calc.min(9pt, (ribbon-height - 12mm) / ribbon-measure * 9pt), fill: white, weight: "bold")[#text(ga-ribbon)]
        ])
      ])
    }
  },
  header: context {
    if counter(page).get().first() > 1 {
      set text(size: 8pt, fill: ga-muted)
      let chapters = query(heading.where(level: 1))
      // Generated chapter headings are in flow/page order. Find the first
      // chapter on this page with logarithmic lookups, not a full per-page
      // interpreted filter over the document's chapter list.
      let current-page = here().page()
      let begin = 0
      let end = chapters.len()
      while begin < end {
        let middle = calc.floor((begin + end) / 2)
        if chapters.at(middle).location().page() < current-page {
          begin = middle + 1
        } else {
          end = middle
        }
      }
      let running = if begin < chapters.len() and chapters.at(begin).location().page() == current-page {
        chapters.at(begin).body.text
      } else if begin > 0 {
        chapters.at(begin - 1).body.text
      } else { ga-title }
      grid(columns: (1fr, 1fr), column-gutter: 15pt,
        [#ga-running(ga-publisher)], align(right)[#ga-running(running)])
      v(4pt)
      line(length: 100%, stroke: 0.5pt + ga-accent)
    }
  },
  footer: context {
    if counter(page).get().first() > 1 {
      set text(size: 8pt, fill: ga-muted)
      line(length: 100%, stroke: 0.5pt + ga-accent)
      v(5pt)
      align(right)[#text(fill: ga-accent, weight: "bold")[#counter(page).display("1 / 1", both: true)]]
    }
  })
