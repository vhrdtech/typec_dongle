// Donguru lab book template. A lab is `<slug>/lab.typ`:
//
//   #import "../lib.typ": *
//   #show: lab.with(title: [...], level: "first-hour", time: "10 min", needs: (...), features: (...))
//   == Goal
//   ...
//
// The same source renders as a PDF page (book.typ, preview.typ) and as an HTML page (site.typ), so every helper
// here checks `target()`: paged output gets the vhrd look, HTML gets plain elements with classes for labs.css.
#import "@local/vhrd:0.9.2": brand, grey, status, categorical, stamp, back-cover

// Levels from the catalog, easiest first: name, colour, short description.
#let levels = (
  first-hour: (name: "First hour", color: categorical.lime, about: [No wiring, a few commands.]),
  bench: (name: "Bench user", color: categorical.blue, about: [Measuring and switching on the bench.]),
  automation: (name: "Test automation", color: categorical.violet, about: [Scripts, CI and test benches.]),
  developer: (name: "Embedded developer", color: categorical.teal, about: [Flashing, debugging, own firmware.]),
  hacker: (name: "Hardware hacker", color: status.warning.solid, about: [Own firmware, wires and board mods.]),
  insane: (name: "Insane", color: status.error.solid, about: [Because we can.]),
)

#let lab-counter = counter("lab")
// The slug of the lab being rendered, set by `lab-at`; lab() records it with the title for the site index.
#let lab-slug = state("lab-slug", none)

#let _html = () => target() == "html"

// An HTML element with a class, or the paged fallback.
#let _div(class, body, paged) = context if _html() { html.elem("div", attrs: (class: class), body) } else { paged }

#let _chip(body, color) = box(inset: (x: 4pt, y: 2pt), radius: 2pt, fill: color.lighten(85%),
  text(7.5pt, weight: "bold", fill: color, body))

// A boxed note: kind is a key of the vhrd `status` palette (warning, info, ok, error, neutral).
#let _note(kind, title, body) = _div("note " + kind, [*#title* #body],
  block(width: 100%, inset: 8pt, radius: 3pt, fill: status.at(kind).soft, stroke: (left: 2pt + status.at(kind).line),
    [#text(fill: status.at(kind).solid, weight: "bold", title) #h(0.3em) #body]))

/// ⚠️ Anything that can hurt the DUT, the computer or the person: power, PD voltages, shorts.
#let safety(body) = _note("warning", [⚠ Safety], body)
/// A hint that makes the lab easier or explains why.
#let tip(body) = _note("info", [Tip], body)
/// Something not written or not built yet, in plain sight so it isn't forgotten.
#let todo(body) = _note("neutral", [To do], body)

/// A photo or screenshot of the setup. Without a path it is a placeholder box with the caption.
#let photo(path: none, width: 100%, caption) = if path == none {
  _div("placeholder", [Photo to come: #caption],
    block(width: width, height: 4cm, radius: 3pt, stroke: (dash: "dashed", paint: grey.at("400")),
      align(center + horizon, text(fill: grey.at("600"))[Photo to come: #caption])))
} else {
  figure(image(path, width: width), caption: caption)
}

/// The same task done from each interface. Leave out the ones that don't exist yet.
#let steps(cli: none, python: none, gui: none) = {
  for (name, body) in (("CLI", cli), ("Python", python), ("GUI", gui)) {
    if body != none {
      _div("steps", [#strong(name) #body], block(breakable: true, [#text(fill: brand.grey, weight: "bold", name) #body]))
    }
  }
}

// The lab's header: number, title, level, time, what you need and which features it relies on.
#let _header(title, level, time, needs, features) = {
  let lv = levels.at(level)
  let meta = (
    ([Level], context if _html() [#lv.name: #lv.about] else [#_chip(lv.name, lv.color) #lv.about]),
    ([Time], [#time]),
    ([You need], needs.map(n => [#n]).join[, ]),
    ([Features], features.map(raw).join[, ]),
  )
  context {
    let n = lab-counter.get().first()
    if _html() {
      heading(level: 1)[Lab #n: #title]
      html.elem("dl", attrs: (class: "lab-meta"), meta.map(((k, v)) => html.elem("dt", k) + html.elem("dd", v)).join())
    } else {
      heading(level: 1, supplement: [Lab])[#title]
      grid(columns: (auto, 1fr), column-gutter: 8pt, row-gutter: 5pt,
        ..meta.map(((k, v)) => (text(fill: brand.grey, k), v)).flatten())
      v(0.4em)
    }
  }
}

/// One lab. `level`: a key of `levels`; `needs`: things on the bench; `features`: FEATURES.md IDs it relies on.
#let lab(title: none, level: "first-hour", time: none, needs: (), features: (), body) = {
  lab-counter.step()
  context [#metadata((slug: lab-slug.get(), title: title)) <lab>]
  _header(title, level, time, needs, features)
  body
}

// Paged look for the book and the one-lab preview: vhrd colours, A4, header and footer with the build stamp.
#let book-style(docrev: none, body) = {
  set document(title: "Donguru lab book", author: "VHRD Tech")
  set text(font: ("Noto Sans", "Liberation Sans", "DejaVu Sans"), size: 10pt, fill: brand.black)
  set par(justify: true, leading: 0.65em)
  set page(paper: "a4", margin: (x: 2cm, top: 2.2cm, bottom: 2cm),
    header: context if counter(page).get().first() > 1 [
      #set text(8pt, brand.grey)
      Donguru lab book #h(1fr) #{
        let labs = query(heading.where(level: 1)).filter(h => h.location().page() <= here().page())
        if labs.len() > 0 [Lab #lab-counter.at(labs.last().location()).first(): #labs.last().body]
      }
      #v(-0.6em) #line(length: 100%, stroke: 0.4pt + brand.light)
    ],
    footer: context [
      #set text(8pt, brand.grey)
      VHRD Tech · vhrd.tech #h(1fr) #stamp(docrev) #h(1fr) #counter(page).display("1 / 1", both: true)
    ])
  show heading.where(level: 1): it => {
    pagebreak(weak: true)
    block(below: 0.9em)[
      #text(9pt, fill: brand.red, weight: "bold")[LAB #lab-counter.get().first()]
      #v(-0.5em)
      #text(18pt, weight: "bold", it.body)
      #v(-0.6em) #line(length: 100%, stroke: 1pt + brand.red)
    ]
  }
  show heading.where(level: 2): set text(12pt, fill: brand.red)
  show heading.where(level: 2): set block(above: 1.3em, below: 0.7em)
  show raw.where(block: true): it => block(width: 100%, fill: brand.paper, inset: (x: 8pt, y: 7pt), radius: 3pt,
    stroke: (left: 1.5pt + brand.light), text(8.5pt, font: ("IBM Plex Mono", "DejaVu Sans Mono"), it))
  set table(stroke: 0.4pt + brand.light, inset: 5pt)
  body
}

/// Lab `slug` with its number from its place in `labs` (labs.typ): what book, preview and site include.
#let lab-at(labs, slug) = {
  lab-counter.update(labs.position(s => s == slug))
  lab-slug.update(slug)
  include slug + "/lab.typ"
}
