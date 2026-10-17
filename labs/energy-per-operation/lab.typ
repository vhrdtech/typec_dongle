#import "../lib.typ": *

#show: lab.with(
  title: [Energy per operation],
  time: "20 min",
  needs: ([Donguru], [a device you can trigger from the computer: an OTA update, a photo, a sync]),
  features: ("API-12", "CLI-34", "CORE-4"),
)

== Goal

Measure how much energy one operation costs, repeat it to get a stable number, and turn it into a test that fails
when a new firmware gets hungrier.

== Steps

#steps(cli: [
  ```sh
  dg power measure -- ./trigger-ota.sh   # runs the command, meters exactly that span
  #  23.4 s · 1.02 Wh · 196 mAh · peak 0.81 A · mean 0.16 A   (exit 0)
  dg power measure --repeat 5 -f json -- ./take-photo.sh | jq .energy_wh
  ```
], python: [
  ```python
  import donguru
  dg = donguru.open()
  with dg.meter.measure() as m:
      take_photo()
  print(f"{m.energy_wh*1000:.1f} mWh, peak {m.peak_current:.2f} A")
  assert m.energy_wh < 0.002, "a photo should cost under 2 mWh"
  ```
])

== What you should see

The same operation costs nearly the same each time; the spread across `--repeat` tells you how much margin to give
a test limit. Put the assert in CI and a regression shows up the day it lands.

== What next

- Lab "CI runner": run this on every commit.
