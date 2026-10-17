// One lab as a PDF while writing it: `just lab SLUG` (typst watch with --input lab=SLUG).
#import "lib.typ": *
#import "labs.typ": labs

#let slug = sys.inputs.at("lab", default: labs.first())
#show: book-style.with(docrev: "Preview")

#lab-at(labs, slug)
