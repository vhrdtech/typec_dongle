// The whole lab book as one PDF: `just labs-book`.
#import "lib.typ": *
#import "labs.typ": labs

#show: book-style.with(docrev: "Draft")

#page(header: none, footer: none, margin: 2.5cm)[
  #v(1fr)
  #text(36pt, weight: "bold", fill: brand.red)[Donguru]
  #v(-0.6em)
  #text(22pt, weight: "bold")[Lab book]
  #v(0.4em)
  #text(12pt, fill: brand.grey)[From a phone's charging power to hacking the firmware: one cable, a script and a lab at
    a time.]
  #v(2fr)
  #text(9pt, fill: brand.grey)[VHRD Tech · vhrd.tech · #stamp("Draft")]
]

#text(18pt, weight: "bold")[Labs]
#outline(title: none, target: heading.where(level: 1))

#for slug in labs { lab-at(labs, slug) }

#back-cover()
