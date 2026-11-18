#let project(title, subtitle, authors) = doc => {
  set document(title: title, author: authors)
  set page(numbering: "1", number-align: center)
  set text(font: "New Computer Modern")
  set heading(numbering: "1.")

  align(center)[
    #text(weight: "bold", size: 24pt)[#title]
    #if subtitle != "" [
      #text(style: "italic", size: 14pt)[#subtitle]
    ]
    #text(size: 12pt)[#authors]
  ]

  doc
}