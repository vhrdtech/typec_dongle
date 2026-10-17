#import "../lib.typ": *

#show: lab.with(
  title: [A four-channel logic analyzer],
  time: "30 min",
  needs: ([Donguru], [jumper wires], [PulseView (sigrok)]),
  features: ("FW-37", "CLI-42", "API-15"),
)

== Goal

Use the header's four IO pins as a small logic analyzer: sample them at up to a few MHz, stream to the computer and
look at the result in PulseView, with protocol decoders.

== Steps

#steps(cli: [
  ```sh
  dg la capture io7,io8,io9,io10 --rate 2M --for 50ms -o capture.sr
  #  4 ch · 2 MS/s · 100 000 samples · trigger io8 falling at 12.4 ms
  dg la capture io7,io8 --rate 1M --trigger io8:falling --pre 10%
  pulseview capture.sr
  ```
])

== What you should see

PulseView opens the capture; its UART or I2C decoder reads the traffic. Sampling runs from a timer and DMA, so the
rate is exact; the limit is USB Full Speed, about 1 MB/s, which allows several MS/s for short buffered captures and
less for continuous streaming.

== What next

- Lab "Donguru as a dev board": build features like this one yourself.
