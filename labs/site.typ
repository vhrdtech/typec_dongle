// The lab book for the website: one HTML page per lab, an index, the feature map, the stylesheet, the whole book and
// every lab as a PDF, in one Typst bundle (`just build-labs`, needs Typst 0.15 with --features html,bundle).
#import "lib.typ": *
#import "labs.typ": parts
#import "style.typ": css

#asset("labs.css", css)

#let page-head() = {
  html.elem("link", attrs: (rel: "stylesheet", href: "labs.css"))
  html.elem("nav", [#link("index.html")[Donguru lab book] · #link("features.html")[Features the labs use] ·
    #link("donguru-lab-book.pdf")[PDF]])
}

#document("index.html", title: "Donguru lab book")[
  #page-head()
  = Donguru lab book
  From a phone's charging power to hacking the firmware: one cable, a script and a lab at a time.
  Also as one #link("donguru-lab-book.pdf")[PDF].

  // Titles from the labs themselves (the metadata lab() records).
  #context {
    let title(slug) = query(<lab>).find(m => m.value.slug == slug).value.title
    let n = 0
    for part in parts {
      let lv = levels.at(part.level)
      heading(level: 2)[#lv.name]
      [#lv.about]
      enum(start: n + 1, ..part.labs.map(slug => link(slug + ".html", title(slug))))
      n += part.labs.len()
    }
  }
]

#document("features.html", title: "Donguru lab book: features")[
  #page-head()
  = Features the labs use
  Every feature the labs rely on, with its status in FEATURES.md: what is missing comes first.
  #feature-map()
]

#for slug in parts.map(p => p.labs).flatten() {
  document(slug + ".html", title: "Donguru lab: " + slug)[
    #page-head()
    #lab-at(parts, slug)
    #html.elem("p", attrs: (class: "pdf"), link(slug + ".pdf")[This lab as PDF])
  ]
  document(slug + ".pdf")[
    #show: book-style.with(docrev: "Draft")
    #lab-at(parts, slug)
  ]
}

#document("donguru-lab-book.pdf")[#include "book.typ"]
