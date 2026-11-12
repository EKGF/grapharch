// Presentation components shared by all generated content. Values come from
// the validated `ga-theme` token dictionary. No source text is evaluated.
#let T = ga-theme
#let ga-ink = T.ink_color
#let ga-muted = T.muted_color
#let ga-line = T.rule_color
#let ga-soft = T.panel_color
#let ga-heading = T.heading_color
#let ga-running(text, limit: 55) = if text.clusters().len() > limit { text.clusters().slice(0, limit - 3).join() + "..." } else { text }
// Line-break opportunities inside long identifiers. An empty inline box is a
// Unicode contingent break; unlike U+200B it adds no character to the text
// that readers copy, search and hear.
#let ga-wrap(..pieces) = pieces.pos().join(box())
#let ga-wrap-raw(..pieces) = pieces.pos().map(raw).join(box())
#set text(font: T.body_font, size: T.body_size, fill: ga-ink, lang: "en", hyphenate: T.hyphenate)
#set par(justify: T.justify, leading: T.leading, spacing: T.paragraph_spacing)
#set list(indent: 1.2em, body-indent: 0.5em, spacing: 1em)
#set enum(indent: 1.5em, body-indent: 0.5em, spacing: 1em)
#set heading(numbering: if T.number_headings { "1.1" } else { none })
#show heading: set block(sticky: true)
#show heading: set par(justify: false)
#show heading: set text(font: T.heading_font, fill: ga-heading)
#show heading: it => if T.heading_number_gap == none or it.numbering == none { it } else {
  grid(columns: (T.heading_number_gap, 1fr), counter(heading).display(it.numbering), it.body)
}
// Space belongs to the fragment container: heading above-spacing collapses
// when the heading is the first element of a breakable fragment block.
#let ga-section(level, body) = block(inset: (top: if level == 1 { 22pt } else if level == 2 { 16pt } else { 12pt }), above: 0pt, breakable: true, body)
#show heading.where(level: 1): set text(size: T.heading_1_size, weight: T.heading_1_weight)
#show heading.where(level: 1): set block(above: 28pt, below: 16pt)
#show heading.where(level: 2): set text(size: T.heading_2_size, weight: T.heading_2_weight)
#show heading.where(level: 2): set block(above: 21pt, below: 11pt)
#show heading.where(level: 3): set text(size: T.heading_3_size, weight: "bold")
#show heading.where(level: 3): set block(above: 14pt, below: 7pt)
#show heading.where(level: 4): set text(size: T.heading_4_size, weight: "bold")
#show link: set text(fill: T.link_color)
#show link: it => if T.link_underline { underline(it) } else { it }
#show raw: set text(font: T.code_font, size: T.code_size, hyphenate: false)
#show figure.caption: set text(font: T.caption_font, size: T.caption_size, weight: if T.caption_bold { "bold" } else { "regular" })
#show figure.where(kind: table): set figure.caption(position: if T.table_caption_above { top } else { bottom })
#show outline.entry: set text(font: T.toc_font)
#show outline.entry.where(level: 1): set text(size: T.toc_entry_size)
// Already highlighted runs share HTML's palette. Wrap grapheme clusters without
// re-tokenising sliced strings, preserving multiline lexical context.
#let ga-code(runs) = block(width: 100%, breakable: true, inset: (x: T.code_inset, y: 9pt), fill: T.code_fill, stroke: if T.code_rule_color == none { none } else { (left: 2pt + T.code_rule_color) }, radius: if T.code_fill == none { 0pt } else { 3pt })[
  #set text(font: T.code_font, size: T.code_size, ligatures: false, hyphenate: false)
  #set par(justify: false)
  #layout(size => {
    let capacity = calc.max(1, calc.floor(size.width / measure(text("M")).width))
    let column = 0
    for (value, color) in runs {
      let fill = if T.code_highlight { color } else { ga-ink }
      let piece = ""
      for cluster in value.clusters() {
        if cluster in ("\n", "\r\n") {
          text(fill: fill, piece); piece = ""
          linebreak(); column = 0
        } else {
          if column >= capacity {
            text(fill: fill, piece); piece = ""
            linebreak(); column = 0
          }
          if cluster == "\t" {
            let count = 4 - calc.rem(column, 4)
            piece += " " * count; column += count
          } else { piece += cluster; column += 1 }
        }
      }
      text(fill: fill, piece)
    }
  })
]
#show quote.where(block: true): it => block(width: 100%, breakable: true, above: 12pt, below: 12pt, inset: (x: T.quote_inset_x, y: T.quote_inset_y), fill: ga-soft, stroke: (left: 2pt + T.quote_rule_color))[
  #set text(size: T.quote_text_size)
  #it.body
]
#let ga-table(columns: (), align: auto, has-header: true, header-row: 0, ..cells) = block(above: 11pt, below: 12pt, breakable: true)[
  #set text(size: T.table_text_size)
  #set par(justify: false, leading: 0.55em, spacing: 0.4em)
  #show table.cell: it => if has-header and it.y == header-row { text(fill: T.table_header_text, it) } else { it }
  #table(columns: columns, align: align, inset: (x: 7pt, y: 6pt),
    stroke: if T.table_grid { 0.4pt + T.table_rule_color } else { (x: none, y: 0.4pt + T.table_rule_color) },
    fill: (x, y) => if has-header and y == header-row { T.table_header_fill }, ..cells)
]
#let ga-callout(label, body, kind: "note") = if T.tinted_callouts {
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
} else {
  block(width: 100%, breakable: true, above: 10pt, below: 10pt, inset: 9pt, stroke: 0.4pt + ga-line)[
    #block(sticky: true, below: 5pt)[#strong[#label]]
    #body
  ]
}
#let ga-example(body) = {
  counter("ga-example").step()
  if T.tinted_callouts {
    block(width: 100%, breakable: true, above: 12pt, below: 12pt, inset: (x: 11pt, y: 9pt), fill: rgb("#fcfaee"), stroke: (left: 2pt + rgb("#b69a28")))[
      #block(sticky: true, below: 6pt)[#text(size: 9pt, weight: "bold", fill: rgb("#685415"))[Example #context counter("ga-example").display()]]
      #body
    ]
  } else {
    block(width: 100%, breakable: true, above: 10pt, below: 10pt, inset: 9pt, stroke: 0.4pt + ga-line)[
      #block(sticky: true, below: 5pt)[Example #context counter("ga-example").display()]
      #body
    ]
  }
}
// Notice front matter: unnumbered and absent from the contents.
#let ga-notice-heading(size, body) = block(width: 100%, above: 1.5em, below: 0.7em, sticky: true)[
  #align(T.notice_heading_align)[#text(font: T.notice_heading_font, size: size, weight: "bold")[#if T.notice_heading_uppercase { upper(body) } else { body }]]
]
#let ga-notices(body) = {
  set text(font: T.notice_font, size: T.notice_size, fill: ga-ink)
  set par(justify: T.notice_justify, leading: 0.5em, spacing: 0.75em)
  set heading(numbering: none, outlined: false)
  show heading: it => ga-notice-heading(T.notice_heading_size, it.body)
  body
}
#let ga-notice-title(title) = {
  if T.notice_title_rule {
    block(width: 100%, inset: (bottom: 10pt), stroke: (bottom: 1.5pt + ga-accent))[
      #text(font: T.notice_title_font, size: T.notice_title_size, weight: "bold")[#title]
    ]
    v(10pt)
  } else {
    ga-notice-heading(T.notice_title_size, title)
  }
}
#let ga-blank-page(notice) = context {
  if calc.odd(counter(page).get().first()) {
    pagebreak()
    align(center + horizon)[#notice]
    pagebreak()
  } else { pagebreak() }
}
#set page(paper: T.paper, margin: (left: T.margin_left, right: T.margin_right, top: T.margin_top, bottom: T.margin_bottom), footer-descent: T.footer_descent,
  background: context {
    if T.cover_bands and counter(page).get().first() == 1 and here().page() == 1 {
      place(top + right, rect(width: 15mm, height: 100%, fill: ga-soft, stroke: none))
      place(bottom + left, rect(width: 100%, height: 3mm, fill: ga-accent, stroke: none))
    }
    if T.status_ribbon and ga-ribbon != none {
      // Like the website's .authority-banner, the label reads bottom to top
      // and ends a fixed padding below the ribbon's top end (website: 1.5rem
      // at a 0.875rem label, about 1.7em; 6mm at the 9pt label).
      let ribbon-padding = 6mm
      let ribbon-measure = measure(text(size: 9pt, weight: "bold", ga-ribbon)).width
      let ribbon-height = calc.min(160mm, calc.max(82mm, ribbon-measure + 2 * ribbon-padding))
      place(top + left, block(width: 8mm, height: ribbon-height, inset: (y: ribbon-padding), fill: ga-accent,
        align(top + center, rotate(-90deg, reflow: true)[
          #text(size: calc.min(9pt, (ribbon-height - 2 * ribbon-padding) / ribbon-measure * 9pt), fill: white, weight: "bold")[#text(ga-ribbon)]
        ])))
    }
  },
  header: context {
    if T.running_header and here().page() > 1 {
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
    let numbered = T.front_numbering == none or page.numbering != none
    if here().page() > 1 and numbered {
      set text(font: T.footer_font, size: T.footer_size, fill: T.footer_color)
      if T.footer_rule {
        line(length: 100%, stroke: 0.5pt + ga-accent)
        v(5pt)
      }
      let number = text(fill: T.page_number_color, weight: if T.page_number_bold { "bold" } else { "regular" })[#if T.page_of_total { counter(page).display("1 / 1", both: true) } else { counter(page).display(if page.numbering == none { "1" } else { page.numbering }) }]
      let label = if T.footer_shows_text { ga-running(ga-footer-text, limit: 85) } else { [] }
      if T.outer_page_number and calc.even(counter(page).get().first()) {
        grid(columns: (auto, 1fr), column-gutter: 12pt, number, align(right)[#label])
      } else {
        grid(columns: (1fr, auto), column-gutter: 12pt, label, align(right)[#number])
      }
    }
  })
