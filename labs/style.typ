// Stylesheet of the HTML labs, with the colours taken from the vhrd palette so they never drift from the PDF.
#import "@local/vhrd:0.9.2": brand, status

#let _h(c) = c.to-hex()

#let css = ```
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
```.text.replace("BLACK", _h(brand.black)).replace("GREY", _h(brand.grey)).replace("RED", _h(brand.red))
  .replace("PAPER", _h(brand.paper)).replace("LIGHT", _h(brand.light))
  .replace("WARN-SOFT", _h(status.warning.soft)).replace("WARN-LINE", _h(status.warning.line))
  .replace("INFO-SOFT", _h(status.info.soft)).replace("INFO-LINE", _h(status.info.line))
  .replace("NEUTRAL-SOFT", _h(status.neutral.soft)).replace("NEUTRAL-LINE", _h(status.neutral.line))
