#import "../lib.typ": *

#show: lab.with(
  title: [A week-long soak test],
  time: "1 h to set up, a week to run",
  needs: ([Donguru], [your device under test], [a computer that stays on]),
  features: ("CLI-43", "CLI-26", "FW-32", "CLI-35", "FW-29", "FW-31"),
)

== Goal

Find the rare bugs: let your device live through days of random but realistic events (power cuts, unplugs, short
dropouts, reconnects), check after each that it recovers, and log everything for the morning.

== Steps

#steps(cli: [
  ```toml
  # soak.toml
  [check]
  after_each = "dg uart expect 'ready' --timeout 20s"

  [[event]]
  kind = "power-cycle"
  off = "200ms..5s"
  weight = 3

  [[event]]
  kind = "usb-cycle"
  off = "100ms..2s"
  weight = 3

  [[event]]
  kind = "power-pulse"
  off = "200us..20ms"
  weight = 2

  [[event]]
  kind = "idle"
  for = "1m..30m"
  weight = 5
  ```
  ```sh
  dg soak soak.toml --for 7d --seed 42 -o soak.jsonl
  dg soak report soak.jsonl         # events, failures with the console before each, recovery times
  dg soak replay soak.jsonl --from 1834   # rerun the events that led to failure #3, same seed
  ```
])

== What you should see

Most events pass. A failure is logged with the event, its parameters, the console lines before it and the meter's
trace; `replay` repeats the same sequence, so a once-a-week bug reproduces in minutes.

== What next

- Lab "A rack console": a soak test for each board in a rack.
