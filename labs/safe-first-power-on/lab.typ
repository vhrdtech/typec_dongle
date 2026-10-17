#import "../lib.typ": *

#show: lab.with(
  title: [Safe first power-on],
  time: "10 min",
  needs: ([Donguru], [a freshly assembled board with a USB-C port]),
  features: ("API-11", "CLI-11", "CLI-32", "FW-30"),
)

== Goal

Power a board for the first time without risking it: if there is a short or a part in backwards, Donguru cuts the
power within milliseconds instead of letting it burn.

== Steps

#steps(cli: [
  ```sh
  dg power on --trip 100mA --report      # cut above 100 mA; report peak and settle
  #  on · peak 63 mA (inrush) · settled 18 mA after 9 ms · ForceOn ≤ 5.5 V, trip 100 mA
  # or, with a short on the board:
  #  tripped: OverCurrent 412 mA after 2 ms · VBUS off           (exit code 4)
  dg power on --trip 300mA               # once it looks sane, a looser limit for normal work
  ```
], python: [
  ```python
  import donguru
  dg = donguru.open()
  r = dg.power.on(trip=0.1, report=True)
  if r.tripped:
      raise SystemExit(f"short? {r.trip_reason} at {r.peak*1e3:.0f} mA")
  ```
])

== What you should see

A healthy board settles at its idle current well under the trip. A board with a short trips at once and the
command exits with code 4, so a production script stops there.

#safety[The current trip runs in the ADC hardware (its analog watchdog) and opens the switch within microseconds of
  a sample over the limit. Voltage and power limits are checked every ~15 ms. It protects a board from burning; it
  is not a fuse rated for a dead short at 20 V.]

== What next

- Lab "Inrush": measure the startup peak to pick a tight trip.
