// One lab as a PDF while writing it: `just lab SLUG` (typst watch with --input lab=SLUG).
#import "lib.typ": *
#import "labs.typ": parts

#let slug = sys.inputs.at("lab", default: parts.first().labs.first())
#show: book-style.with(docrev: "Preview")

#lab-at(parts, slug)
