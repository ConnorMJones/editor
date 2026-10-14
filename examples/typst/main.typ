#import "lib.typ": *

#set document(title: "Typst example")
#set page(paper: "a5", margin: 2cm, numbering: "1")
#set text(font: "Libertinus Serif", size: 11pt)
#set heading(numbering: "1.1")
#show raw: set text(font: "DejaVu Sans Mono")

#align(center, text(size: 20pt, weight: "bold")[Typst example])

= Introduction

This document shows #accent[typst] in the editor. Open it, press `space v`
for a live preview beside the text, and run `:export-pdf` to write
`main.pdf` next to it.

#note[Tip][Use `space V` to swap the tab between source and preview.]

== Math

Inline math such as $a^2 + b^2 = c^2$ sits in the text. Block math is
centred:

$ sum_(k=1)^n k = (n(n+1)) / 2 $

$ integral_0^oo e^(-x^2) dif x = sqrt(pi) / 2 $

$ A = mat(1, 2; 3, 4), quad det A = -2 $

#pagebreak()

= Tables and figures

#figure(
  table(
    columns: (auto, 1fr, auto),
    align: (left, left, right),
    table.header[*Format*][*Shows*][*Pages*],
    [Markdown], [Notes, links, callouts], [1],
    [Typst], [Documents, math], [3],
    [Mermaid], [Diagrams], [1],
  ),
  caption: [What each example covers.],
)

#figure(
  image("gradient.png", width: 60%),
  caption: [A small generated image.],
)

#pagebreak()

#include "chapter.typ"
