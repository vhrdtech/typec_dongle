#import "../lib.typ": *

#show: lab.with(
  title: [Kill switch],
  time: "10 min",
  needs: ([Portwright], [a USB-powered gadget: lamp, fan or a dev board]),
  features: ("CLI-11", "API-11", "CORE-4", "GUI-1"),
)

== Goal

Switch a USB device off and on from the computer, and power-cycle it with an exact off time, without touching the
cable.

== Setup

#photo[A USB lamp on Portwright's plug, lit]

== Steps

#steps(cli: [
  ```sh
  pw power off                  # lamp off
  pw power on                   # lamp on
  pw power cycle --off 2s       # off for exactly 2 s, then back on (timed by the firmware)
  pw power status               # mode, state and the meter in one table
  ```
], python: [
  ```python
  import portwright, time
  pw = portwright.open()
  for _ in range(5):                  # blink the lamp
      pw.power.off(); time.sleep(0.5)
      pw.power.on();  time.sleep(0.5)
  ```
], gui: [
  `pw ui` opens the Portwright app: the Power tab has one big switch per output (plug VBUS, header VBUS, relay) and a
  "Cycle" button with the off time next to it.
])

== What you should see

The lamp follows each command within a few milliseconds. `pw power cycle` returns when power is back, so a script
can go on right away. `pw power status` shows `ForceOn (≤ 5.5 V)` and the lamp's current.

#tip[`on`, `off` and `cycle` exit non-zero when VBUS doesn't end up where asked (a trip, a fault): in a script,
  `pw power on || echo "it tripped"`.]

== What next

- Lab "Safe first power-on": the same switch with a current limit for a board you've never powered.
- Lab "Hung board": power-cycle a device automatically when it stops answering.
