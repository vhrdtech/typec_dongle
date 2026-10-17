#import "../lib.typ": *

#show: lab.with(
  title: [Logs next to current],
  time: "30 min",
  needs: ([Donguru], [a board with SWD on SBU running defmt or RTT logging], [its ELF file]),
  features: ("FW-15", "FW-34", "CLI-38", "API-26", "GUI-2"),
)

== Goal

See which line of your firmware's log goes with which current spike: Donguru reads the board's RTT log itself over
SWD and stamps it with the same clock as its power meter.

== Steps

#steps(cli: [
  ```sh
  dg trace --elf target/thumbv6m-none-eabi/release/app --for 30s -o trace.jsonl
  #  0.000 s   12 mA   INFO  boot
  #  0.412 s   88 mA   INFO  radio on
  #  0.415 s  141 mA   ▲ peak
  #  0.903 s   14 mA   INFO  sleep 5 s
  dg trace view trace.jsonl              # the timeline in the terminal; -o trace.html for a browser
  ```
], gui: [
  The Trace tab plots current on top and log lines below on the same time axis; hovering a spike highlights the
  line logged just before it.
])

== What you should see

Each log line sits on the current plot where it happened. A radio transmission, a flash write or a display refresh
show up as spikes next to the line that started them, so "where does the energy go" becomes a list of lines.

#tip[RTT read by the dongle needs no probe-rs and no second probe; while `dg trace` runs, probe-rs can't use the SWD
  link, and the other way round.]

== What next

- Lab "Energy per operation": put numbers on the spikes you found.
