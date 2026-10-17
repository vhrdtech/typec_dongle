#import "../lib.typ": *

#show: lab.with(
  title: [Kill switch],
  time: "10 min",
  needs: ([Donguru], [a USB-powered gadget: lamp, fan or a dev board]),
  features: ("CLI-11", "API-11", "CORE-4", "GUI-1"),
)

== Goal

Switch a USB device off and on from the computer, and power-cycle it with an exact off time, without touching the
cable.

== Setup

#photo[A USB lamp on Donguru's plug, lit]

== Steps

#steps(cli: [
  ```sh
  dg power off                  # lamp off
  dg power on                   # lamp on
  dg power cycle --off 2s       # off for exactly 2 s, then back on (timed by the firmware)
  dg power status               # mode, state and the meter in one table
  ```
], python: [
  ```python
  import donguru, time
  dg = donguru.open()
  for _ in range(5):                  # blink the lamp
      dg.power.off(); time.sleep(0.5)
      dg.power.on();  time.sleep(0.5)
  ```
], gui: [
  `dg ui` opens the Donguru app: the Power tab has one big switch per output (plug VBUS, header VBUS, relay) and a
  "Cycle" button with the off time next to it.
])

== What you should see

The lamp follows each command within a few milliseconds. `dg power cycle` returns when power is back, so a script
can go on right away. `dg power status` shows `ForceOn (≤ 5.5 V)` and the lamp's current.

#tip[`on`, `off` and `cycle` exit non-zero when VBUS doesn't end up where asked (a trip, a fault): in a script,
  `dg power on || echo "it tripped"`.]

== What next

- Lab "Safe first power-on": the same switch with a current limit for a board you've never powered.
- Lab "Hung board": power-cycle a device automatically when it stops answering.
