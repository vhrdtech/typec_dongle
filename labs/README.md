# Donguru lab book

Short labs that show what Donguru can do, from measuring a phone's charging power to hacking the firmware. Each lab
is one Typst file; the same source becomes a page of the PDF book and a page on the website.

## Layout

- `<slug>/lab.typ`: one lab, with its photos and plots next to it in the same folder.
- `labs.typ`: the labs in book order, grouped by level. A lab's number is its place in this list, its level the
  group it is in.
- `lib.typ`: the template: `lab()`, `steps()`, `photo()`, `safety[]`, `tip[]`, `todo[]`, `mod[]` (board mods),
  `feature-map()` and the PDF look.
- `book.typ`: the whole book as one PDF (cover, list of labs, a part per level, every lab, the feature map, back
  cover).
- `site.typ`: the website: an index page, the feature map and one HTML page per lab, the stylesheet (`style.typ`, colours from the
  vhrd palette), the book and every lab as PDF, in one Typst bundle.
- `preview.typ`: one lab as PDF, for writing.

## Build

Needs Typst 0.15 (HTML and bundle export are still experimental there, behind `--features html,bundle`) and the
vhrd package (`@local/vhrd:0.9.2`, `just install` in the `vhrd_brand` repo).

```sh
just lab phone-charging   # one lab as PDF, rebuilt on every save: target/labs/phone-charging.pdf
just labs-book            # the whole book: target/labs/donguru-lab-book.pdf
just build-labs           # the website part: site/labs/index.html, one page and one PDF per lab, the book
```

## Writing a lab

1. Make a folder `<slug>/` (2-4 kebab-case words, it becomes the page name) with a `lab.typ`, and add the slug to
   `labs.typ` where it belongs in the order.
2. Start from an existing lab. Every lab has the same sections: Goal, Setup (photo), Steps, What you should see,
   What next, and a `safety[]` box wherever power or voltages are involved.
3. `steps(cli: [...], python: [...], gui: [...])` shows the same task from each interface; leave out the ones that
   don't exist yet.
4. Labs describe Donguru as it should be, not only as it is: a lab may use commands that don't exist yet. Every
   feature it relies on is a FEATURES.md ID in `features:`; the header shows each one's status, read from
   FEATURES.md at build time, and a lab using anything not built gets a Preview note. A feature that has no ID yet
   gets one in FEATURES.md (💡 or 📋) in the same commit. The feature map at the end of the book lists them all,
   missing ones first. Commands that exist are run on a board and their real output pasted.
5. `photo[caption]` without a `path:` is a placeholder until the photo is taken.
