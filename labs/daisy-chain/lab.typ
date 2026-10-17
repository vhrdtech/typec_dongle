#import "../lib.typ": *

#show: lab.with(
  title: [How long can the chain be?],
  time: "30 min",
  needs: ([three to five Donguru], [a device at the end]),
  features: ("CLI-36", "API-12", "FW-35", "CLI-34"),
)

== Goal

Chain dongles plug-to-receptacle and measure what each hop costs: voltage drop, power, and the USB tiers a hub chain
uses up. A playful way to learn why "too many hubs" fails.

== Steps

#steps(cli: [
  ```sh
  dg device list --tree                     # one level deeper per dongle
  dg each --all -- power on
  dg each --all -f jsonl -- power meter | jq -r '[.label,.vbus_in,.current] | @tsv'
  #  d1  5.11 V  0.62 A
  #  d2  5.02 V  0.55 A
  #  d3  4.94 V  0.49 A
  #  d4  4.86 V  0.43 A    <- the device at the end gets less than 4.8 V
  ```
])

== What you should see

Each dongle adds a hub tier and some 80 mV drop at half an amp, plus its own ~60 mA. USB allows five hubs between
the computer and a device: the device behind the fifth dongle doesn't enumerate, the first four do.

== What next

- Lab "Soak test": chains make good long-running fixtures, too.
