#import "../lib.typ": *

#show: lab.with(
  title: [Is it really asleep?],
  time: "15 min",
  needs: ([Portwright], [a battery-less device that sleeps: a dev board, a sensor node]),
  features: ("API-12", "API-25", "CLI-30"),
)

== Goal

Measure what a device draws when it should be idle, and compare two firmware versions. Learn the limits of
Portwright's meter at low currents.

== Steps

#steps(cli: [
  ```sh
  pw power off
  pw power zero                         # with the plug off, the meter learns its own offset
  pw power on
  pw power meter --average 30s          # mean, min, max over 30 s
  #  current 4.1 mA (± 0.8 mA)  min 3.2 mA  max 61 mA   power 21 mW
  ```
], python: [
  ```python
  import portwright
  pw = portwright.open()
  pw.meter.zero()
  a = pw.meter.average(seconds=30)
  print(f"{a.current*1e3:.1f} ± {a.current_error*1e3:.1f} mA")
  ```
])

== What you should see

A board that sleeps properly shows a few mA or less; one that never sleeps shows tens of mA with spikes every
tick. Portwright shows the uncertainty next to the value: averaging brings it down to about a milliamp.

#tip[Portwright's meter is built for amps: a 5 mΩ shunt, one ADC step is about 3 mA. Averaging and the offset learnt by
  `pw power zero` get it to about 1 mA; for microamp sleep currents use a dedicated meter (Joulescope, Nordic PPK2)
  and let Portwright switch the power.]

== What next

- Lab "Inrush": what happens in the first milliseconds after power-on.
