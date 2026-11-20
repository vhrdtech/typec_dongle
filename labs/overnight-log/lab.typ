#import "../lib.typ": *

#show: lab.with(
  title: [A night of power],
  time: "10 min + a night",
  needs: ([Portwright], [a device to charge or run overnight: phone, earbuds case, a board on a test]),
  features: ("API-12", "FW-29", "CLI-29", "CORE-4"),
)

== Goal

Record a whole charge cycle, or a night of a device running, and plot it in the morning. Learn where a phone's
charger slows down (constant current, then constant voltage) and how much energy a full charge takes.

== Setup

#photo[Phone charging through Portwright on the nightstand, laptop closed next to it]

== Steps

#steps(cli: [
  ```sh
  pw power on
  pw power log night.csv --interval 1s --for 10h     # appends; survives the laptop sleeping (see below)
  # morning:
  pw power log --summary night.csv     # duration, energy, charge, peak, time above 1 A
  pw power plot night.csv -o night.png # V / I / P over time
  ```
], python: [
  ```python
  import portwright, pandas as pd
  pw = portwright.open()
  df = pw.meter.history()             # the dongle's own record: one point a second for the last 24 h
  df.set_index("time")["power"].plot()
  print(f"{df.energy.iloc[-1]:.2f} Wh")
  ```
])

#tip[A laptop that goes to sleep stops reading the dongle. Portwright keeps its own history (one averaged point a
  second, 24 hours) in RAM, and `pw power log` fills the gap when the computer wakes up, so the night has no holes.
  The dongle needs power for that: a laptop that cuts USB power in sleep loses it too, so keep it on a charger.]

== What you should see

A phone charges at full current for most of the way (the flat part of the plot), then the current falls off over the
last 20 % while the voltage stays put. Energy in is 10 to 30 % more than the battery's rated Wh: that is the
charging loss.

== What next

- Lab "Standby current": what the phone draws once it's full.
- Lab "Energy per operation": the same totals around a single action in a test.
