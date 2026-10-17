// Stylesheet of the HTML labs, with the colours taken from the vhrd palette so they never drift from the PDF.
#import "@local/vhrd:0.9.2": brand, status

#let _h(c) = c.to-hex()

#let _template = ```
body { max-width: 46rem; margin: 2rem auto; padding: 0 1rem; font-family: "Noto Sans", system-ui, sans-serif;
  line-height: 1.55; color: BLACK; }
nav { font-size: 0.9rem; margin-bottom: 1.5rem; color: GREY; }
a { color: RED; }
h2 { border-bottom: 2px solid RED; padding-bottom: 0.2rem; }
h3 { color: RED; }
pre { background: PAPER; border-left: 3px solid LIGHT; padding: 0.6rem 0.8rem; overflow-x: auto; }
dl.lab-meta { display: grid; grid-template-columns: auto 1fr; gap: 0.2rem 1rem; }
dl.lab-meta dt { color: GREY; }
dl.lab-meta dd { margin: 0; }
.note { padding: 0.6rem 0.8rem; border-radius: 3px; margin: 1rem 0; }
.note.warning { background: WARN-SOFT; border-left: 3px solid WARN-LINE; }
.note.info { background: INFO-SOFT; border-left: 3px solid INFO-LINE; }
.note.neutral { background: NEUTRAL-SOFT; border-left: 3px solid NEUTRAL-LINE; }
.placeholder { border: 2px dashed LIGHT; color: GREY; padding: 3rem 1rem; text-align: center; margin: 1rem 0; }
.steps > strong { color: GREY; }
img { max-width: 100%; }
p.pdf { margin-top: 2rem; font-size: 0.9rem; }
.note.error { background: ERR-SOFT; border-left: 3px solid ERR-LINE; }
.feature { font-size: 0.8rem; padding: 0.05rem 0.3rem; border-radius: 3px; background: NEUTRAL-SOFT; margin-right: 0.2rem; }
.feature.built { background: OK-SOFT; }
.feature.untested, .feature.in-progress { background: INFO-SOFT; }
.feature.bug, .feature.unknown { background: ERR-SOFT; }
table { border-collapse: collapse; }
td, th { border: 1px solid LIGHT; padding: 0.2rem 0.5rem; text-align: left; }
```.text

// The placeholders in the template above and their palette colours.
#let _colors = (
  "WARN-SOFT": status.warning.soft, "WARN-LINE": status.warning.line,
  "INFO-SOFT": status.info.soft, "INFO-LINE": status.info.line,
  "NEUTRAL-SOFT": status.neutral.soft, "NEUTRAL-LINE": status.neutral.line,
  "ERR-SOFT": status.error.soft, "ERR-LINE": status.error.line, "OK-SOFT": status.ok.soft,
  "BLACK": brand.black, "GREY": brand.grey, "RED": brand.red, "PAPER": brand.paper, "LIGHT": brand.light,
)
#let css = _colors.pairs().fold(_template, (acc, (k, c)) => acc.replace(k, _h(c)))
