// The lab book for the website: one HTML page per lab, an index, the stylesheet, the whole book and every lab as a
// PDF, in one Typst bundle (`just build-labs`, needs Typst 0.15 with --features html,bundle).
#import "lib.typ": *
#import "labs.typ": labs
#import "style.typ": css

#asset("labs.css", css)

#let page-head(title) = {
  html.elem("link", attrs: (rel: "stylesheet", href: "labs.css"))
  html.elem("nav", [#link("index.html")[Donguru lab book] · #link("donguru-lab-book.pdf")[PDF]])
}

#document("index.html", title: "Donguru lab book")[
  #page-head[]
  = Donguru lab book
  From a phone's charging power to hacking the firmware: one cable, a script and a lab at a time.
  Also as one #link("donguru-lab-book.pdf")[PDF].

  // Titles from the labs themselves (the metadata lab() records).
  #context {
    let title(slug) = query(<lab>).find(m => m.value.slug == slug).value.title
    enum(..labs.map(slug => link(slug + ".html", title(slug))))
  }
]

#for (i, slug) in labs.enumerate() {
  document(slug + ".html", title: "Donguru lab " + str(i + 1))[
    #page-head[]
    #lab-at(labs, slug)
    #html.elem("p", attrs: (class: "pdf"), link(slug + ".pdf")[This lab as PDF])
  ]
  document(slug + ".pdf")[
    #show: book-style.with(docrev: "Draft")
    #lab-at(labs, slug)
  ]
}

#document("donguru-lab-book.pdf")[#include "book.typ"]
