#import "../lib.typ": *

#show: lab.with(
  title: [Board mod: see the plug-side voltage],
  time: "1 h",
  needs: ([Donguru rev A], [two 0402 resistors (100 k and 15 k)], [a fine soldering iron, thin wire]),
  features: ("FW-35", "API-12", "FW-3"),
)

== Goal

Donguru measures VBUS only on the computer's side. Add a divider on the plug side and a few lines of firmware, and
the meter shows the voltage the device actually gets, and the drop across the switch.

#mod[Two resistors from the plug side of the current shunt (`+V2`) to GND, their middle to a free ADC pin (see
  the docs page for the pads). Undo by removing them. The warranty doesn't mind; the board might, if the iron slips.]

== Steps

#steps(cli: [
  ```sh
  cd fw/donguru && cargo run --release --features plug-vsense    # enables the extra ADC channel
  dg power meter
  #  vbus_in 5.12 V · vbus_out 5.01 V (drop 0.11 V) · 1.21 A · 6.06 W
  ```
])

== What you should see

`vbus_out` follows `vbus_in` minus the switch and shunt drop (about 0.1 V per amp). With the plug off it reads the
DUT's own voltage: a device that back-feeds VBUS shows up at once.

== What next

- Lab "Is it the cable?" now tells the cable's drop from the port's.
