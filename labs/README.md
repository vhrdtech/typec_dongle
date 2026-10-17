# Donguru lab book

Short labs that show what Donguru can do, from measuring a phone's charging power to hacking the firmware. Each lab
is one Typst file; the same source becomes a page of the PDF book and a page on the website.

## Layout

- `<slug>/lab.typ`: one lab, with its photos and plots next to it in the same folder.
- `labs.typ`: the labs in book order. A lab's number is its place in this list.
- `lib.typ`: the template: `lab()`, `steps()`, `photo()`, `safety[]`, `tip[]`, `todo[]` and the PDF look.
- `book.typ`: the whole book as one PDF (cover, list of labs, every lab, back cover).
- `site.typ`: the website: an index page and one HTML page per lab, the stylesheet (`style.typ`, colours from the
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
4. Commands are real: run them on a board and paste what they print. A lab that relies on something not built yet
   says so with `todo[]` and lists the FEATURES.md IDs in `features:`.
5. `photo[caption]` without a `path:` is a placeholder until the photo is taken.
