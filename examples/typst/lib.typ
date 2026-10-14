// Helpers shared by main.typ and chapter.typ.

#let note(title, body) = block(
  width: 100%,
  inset: 8pt,
  radius: 4pt,
  stroke: 0.5pt + rgb("#4a6fa5"),
  fill: rgb("#eef3fa"),
)[*#title* #h(0.5em) #body]

#let accent(body) = text(fill: rgb("#4a6fa5"), weight: "bold", body)
