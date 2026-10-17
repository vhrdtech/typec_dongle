#import "../lib.typ": *

#show: lab.with(
  title: [Flash and debug over the USB-C cable],
  time: "20 min",
  needs: ([Donguru], [a board with SWD on its USB-C SBU pins (our B129A, or the SBU breakout and 3 wires)],
    [probe-rs]),
  features: ("FW-15", "FW-17", "FW-20", "API-4", "CLI-17"),
)

== Goal

Flash firmware into a board and debug it through the same USB-C connector that powers it: Donguru is a CMSIS-DAP
probe on the plug's SBU lines, and the cable may go in either way round.

== Setup

#photo[B129A on Donguru's plug, probe-rs running in a terminal]

== Steps

#steps(cli: [
  ```sh
  probe-rs list                          # Donguru shows up as a CMSIS-DAP v2 probe (c0de:cafe)
  dg power on
  probe-rs info --probe c0de:cafe --protocol swd
  probe-rs run --probe c0de:cafe --chip STM32G0B1CETx --speed 2000 target/thumbv6m-none-eabi/release/app
  dg swd status                          # Connected · 2 MHz · orientation normal
  dg sbu orientation swapped             # if the cable is in the other way and Auto can't tell yet
  ```
])

== What you should see

`probe-rs run` flashes, starts the firmware and streams its defmt log, as with any probe. Flashing 64 KB takes a
few seconds at 2 MHz. `dg swd status` shows the session and the clock in use.

#tip[Donguru has no reset line to the target: "connect under reset" isn't possible. For a board whose firmware
  disables SWD early, cycle its power with `dg power cycle` and connect right after.]

== What next

- Lab "SWD link memtest": check the link is clean before you trust it.
- Lab "Logs next to current": the DUT's log and its power draw on one timeline.
