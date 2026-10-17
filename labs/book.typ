// The whole lab book as one PDF: `just labs-book`.
#import "lib.typ": *
#import "labs.typ": parts

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
#outline(title: none, target: heading.where(level: 1).or(heading.where(level: 1, supplement: [Part])))

#for part in parts {
  let lv = levels.at(part.level)
  heading(level: 1, supplement: [Part], outlined: false)[#lv.name]
  text(12pt, fill: brand.grey, lv.about)
  for slug in part.labs { lab-at(parts, slug) }
}

= Features the labs use
Every feature the labs rely on, with its status in FEATURES.md: what is missing comes first.

#feature-map()

#back-cover()
