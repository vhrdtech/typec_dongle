#import "../lib.typ": *

#show: lab.with(
  title: [The first 50 milliseconds],
  time: "15 min",
  needs: ([Donguru], [a device with big input capacitors: a dev board, a USB hub, a display]),
  features: ("API-26", "CLI-29", "API-11", "CORE-4"),
)

== Goal

Capture the current spike when a device is switched on, how high it goes and how long the device takes to settle.
That decides whether it trips a laptop port and how to set Donguru's trip limit.

== Steps

#steps(cli: [
  ```sh
  dg power off
  dg power record --rate 50k --for 100ms --trigger power-on -o inrush.csv &
  dg power on
  wait
  dg power plot inrush.csv -o inrush.png
  #  peak 1.84 A at 0.6 ms · above 500 mA for 3.1 ms · settled at 42 mA after 18 ms
  ```
], python: [
  ```python
  import donguru
  dg = donguru.open()
  with dg.meter.capture(rate=50_000, duration=0.1, trigger="power-on") as cap:
      dg.power.on()
  cap.data.plot(x="time", y="current")   # pandas, 5000 rows
  print(cap.summary())
  ```
], gui: [
  The Meter tab has a "Capture" button: pick the rate, the trigger (power on, a current level) and the length.
])

== What you should see

A board with 100 µF on its input draws 1-3 A for a millisecond or two, then drops to its running current. The
capture shows the spike that `dg power watch` (a few readings a second) never sees.

#safety[USB 2.0 allows 10 µF of input capacitance on a device; a big inrush can make a laptop port cut out. Set
  `dg power on --trip` above the inrush peak, or it trips at every start.]

== What next

- Lab "Safe first power-on": use what you measured as the trip limit.
