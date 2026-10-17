#import "../lib.typ": *

#show: lab.with(
  title: [Header GPIO and ADC],
  time: "20 min",
  needs: ([Donguru], [an LED with a resistor, a push button, a battery to measure, jumper wires]),
  features: ("FW-10", "API-15", "CLI-5", "CLI-22", "CORE-4"),
)

== Goal

Use the four IO pins on the header as a tiny I/O board: blink an LED, read a button, measure a voltage.

== Setup

IO7 → LED → GND, IO8 → button → GND, IO9 → a 1:2 divider from a Li-ion cell (the ADC reads up to 3.3 V).

#photo[Breadboard with an LED, a button and a cell holder wired to the header]

== Steps

#steps(cli: [
  ```sh
  dg gpio mode io7 output; dg gpio write io7 1    # LED on
  dg gpio mode io8 input-pullup
  dg gpio watch io8                     # prints every edge: falling 12.031 s, rising 12.205 s
  dg adc read io9                       # 1.93 V
  dg adc read io9 --scale 2             # 3.86 V: the cell, through the divider
  ```
], python: [
  ```python
  import donguru, time
  dg = donguru.open()
  led, btn = dg.gpio.header.io7, dg.gpio.header.io8
  led.mode, btn.mode = "output", "input-pullup"
  for edge in btn.events():           # blink while the button is held
      led.value = edge.level == 0
  ```
])

== What you should see

The LED follows `gpio write`, `watch` shows each press with a timestamp, the ADC reads the cell within a few mV.

#safety[Header IO is 3.3 V logic. Nothing above 3.3 V on an IO pin: use a divider for anything higher.]

== What next

- Lab "Hung board": combine a GPIO watch with power cycling.
