// Donguru lab book template. A lab is `<slug>/lab.typ`:
//
//   #import "../lib.typ": *
//   #show: lab.with(title: [...], time: "10 min", needs: (...), features: ("API-12", "CLI-11"))
//   == Goal
//   ...
//
// The level comes from where the lab sits in labs.typ. The same source renders as a PDF page (book.typ,
// preview.typ) and as an HTML page (site.typ), so every helper checks `target()`: paged output gets the vhrd look,
// HTML gets plain elements with classes for style.typ.
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

// Feature status straight from FEATURES.md, so a lab never claims more than the code does: ID -> status emoji.
#let feature-status = {
  let found = (:)
  for m in read("/FEATURES.md").matches(regex("(?m)^- (\S+) \*\*([A-Z]+-\d+)\b")) {
    found.insert(m.captures.at(1), m.captures.at(0))
  }
  found
}
#let status-words = (
  "✅": ("built", status.ok), "🧪": ("untested", status.info), "🚧": ("in progress", status.info),
  "📋": ("planned", status.neutral), "💡": ("idea", status.neutral), "⬜": ("stub", status.neutral),
  "🐛": ("bug", status.error), "⛔": ("blocked", status.error), "🔍": ("to check", status.neutral),
)
#let feature-word(id) = status-words.at(feature-status.at(id, default: "?"), default: ("unknown", status.error))
#let built(id) = feature-status.at(id, default: none) == "✅"

#let lab-counter = counter("lab")
// The slug and level of the lab being rendered, set by `lab-at`; lab() records them for the index and feature map.
#let lab-slug = state("lab-slug", none)
#let lab-level = state("lab-level", none)

#let _html = () => target() == "html"

// An HTML element with a class, or the paged fallback.
#let _div(class, body, paged) = context if _html() { html.elem("div", attrs: (class: class), body) } else { paged }
#let _span(class, body, paged) = context if _html() { html.elem("span", attrs: (class: class), body) } else { paged }

#let _chip(body, color) = box(inset: (x: 4pt, y: 2pt), radius: 2pt, fill: color.lighten(85%),
  text(7.5pt, weight: "bold", fill: color, body))

/// A feature ID with its status from FEATURES.md, e.g. `API-12 built`.
#let feature(id) = {
  let (word, st) = feature-word(id)
  _span("feature " + word.replace(" ", "-"), [#raw(id) #word],
    box(inset: (x: 3pt, y: 1.5pt), radius: 2pt, fill: st.soft, text(7.5pt, fill: st.solid)[#raw(id) #word]))
}

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
/// Hardware changes: a board mod, an extra part, wires. Says what it costs and how to undo it.
#let mod(body) = _note("error", [Board mod], body)

/// A photo or screenshot of the setup. Without a path it is a placeholder box with the caption.
#let photo(path: none, width: 100%, caption) = if path == none {
  _div("placeholder", [Photo to come: #caption],
    block(width: width, height: 3.2cm, radius: 3pt, stroke: (dash: "dashed", paint: grey.at("400")),
      align(center + horizon, text(fill: grey.at("600"))[Photo to come: #caption])))
} else {
  figure(image(path, width: width), caption: caption)
}

/// The same task done from each interface. Leave out the ones that don't exist yet.
#let steps(cli: none, python: none, gui: none) = {
  for (name, body) in (("CLI", cli), ("Python", python), ("GUI", gui)) {
    if body != none {
      _div("steps", [#strong(name) #body],
        block(breakable: true, [#block(below: 0.5em, text(fill: brand.grey, weight: "bold", name)) #body]))
    }
  }
}

// The lab's header: number, title, level, time, what you need, the features it relies on and their status.
#let _header(title, time, needs, features) = context {
  let lv = levels.at(lab-level.get())
  let n = lab-counter.get().first()
  let meta = (
    ([Level], if _html() [#lv.name: #lv.about] else [#_chip(lv.name, lv.color) #lv.about]),
    ([Time], [#time]),
    ([You need], needs.map(n => [#n]).join[, ]),
    ([Features], features.map(feature).join[ ]),
  )
  if _html() {
    heading(level: 1)[Lab #n: #title]
    html.elem("dl", attrs: (class: "lab-meta"), meta.map(((k, v)) => html.elem("dt", k) + html.elem("dd", v)).join())
  } else {
    heading(level: 1, supplement: [Lab])[#title]
    grid(columns: (auto, 1fr), column-gutter: 8pt, row-gutter: 5pt,
      ..meta.map(((k, v)) => (text(fill: brand.grey, k), v)).flatten())
    v(0.2em)
  }
  let missing = features.filter(id => not built(id))
  if missing.len() > 0 {
    _note("info", [Preview], [This lab describes Donguru as it will be: it uses #missing.map(raw).join[, ], not
      built yet (status from FEATURES.md). Commands and output may still change.])
  }
}

/// One lab. `needs`: things on the bench; `features`: FEATURES.md IDs it relies on, built or not.
#let lab(title: none, time: none, needs: (), features: (), body) = {
  lab-counter.step()
  context [#metadata((slug: lab-slug.get(), level: lab-level.get(), title: title, features: features)) <lab>]
  _header(title, time, needs, features)
  body
}

/// Lab `slug` with its number from its place in labs.typ: what book, preview and site include.
#let lab-at(parts, slug) = {
  let all = parts.map(p => p.labs).flatten()
  lab-counter.update(all.position(s => s == slug))
  lab-slug.update(slug)
  lab-level.update(parts.find(p => slug in p.labs).level)
  include slug + "/lab.typ"
}

/// Every feature the labs rely on, with its status and the labs that use it: what is missing, in one table.
#let feature-map() = context {
  let seen = (:)
  for m in query(<lab>) {
    if seen.at(m.value.slug, default: none) == none { seen.insert(m.value.slug, m.value) }
  }
  let uses = (:)
  for (slug, l) in seen {
    for id in l.features { uses.insert(id, uses.at(id, default: ()) + (slug,)) }
  }
  let order = ("?", "⛔", "🐛", "⬜", "💡", "📋", "🔍", "🚧", "🧪", "✅")
  let rank(id) = order.position(s => s == feature-status.at(id, default: "?"))
  let ids = uses.keys().sorted(key: id => (rank(id), id))
  table(columns: (auto, auto, 1fr), table.header([Feature], [Status], [Labs]),
    ..ids.map(id => (raw(id), feature-word(id).at(0), uses.at(id).join(", "))).flatten())
}

// Paged look for the book and the one-lab preview: vhrd colours, A4, header and footer with the build stamp.
#let book-style(docrev: none, body) = {
  set document(title: "Donguru lab book", author: "VHRD Tech")
  set text(font: ("Noto Sans", "Liberation Sans", "DejaVu Sans", "Noto Color Emoji"), size: 10pt, fill: brand.black)
  set par(justify: true, leading: 0.65em)
  set page(paper: "a4", margin: (x: 2cm, top: 2.2cm, bottom: 2cm),
    header: context if counter(page).get().first() > 1 [
      #set text(8pt, brand.grey)
      Donguru lab book #h(1fr) #{
        let labs = query(heading.where(level: 1)).filter(h => h.location().page() <= here().page())
        if labs.len() > 0 and labs.last().supplement == [Lab] [
          Lab #lab-counter.at(labs.last().location()).first(): #labs.last().body
        ]
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
      #if it.supplement == [Lab] {
        text(9pt, fill: brand.red, weight: "bold")[LAB #lab-counter.get().first()]
        v(-0.5em)
      }
      #text(18pt, weight: "bold", it.body)
      #v(-0.6em) #line(length: 100%, stroke: 1pt + brand.red)
    ]
  }
  show heading.where(level: 2): set text(12pt, fill: brand.red)
  show heading.where(level: 2): set block(above: 1.3em, below: 0.7em)
  show raw.where(block: true): it => block(width: 100%, fill: brand.paper, inset: (x: 8pt, y: 7pt), radius: 3pt,
    stroke: (left: 1.5pt + brand.light), text(8.5pt, font: ("IBM Plex Mono", "DejaVu Sans Mono"), it))
  set table(stroke: 0.4pt + brand.light, inset: 5pt)
  show table.cell.where(y: 0): set text(weight: "bold")
  body
}
