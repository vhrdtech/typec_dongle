#import "../lib.typ": *

#show: lab.with(
  title: [Portwright debugs Portwright],
  time: "20 min",
  needs: ([two Portwright],),
  features: ("FW-15", "FW-17", "FW-20", "CLI-36"),
)

== Goal

Plug one Portwright into another and let the first flash and debug the second: Portwright's own SWD pins are on its
receptacle's SBU lines, so the plug of one fits the debug port of the other.

== Setup

Portwright A in the computer, Portwright B's receptacle on A's plug. B enumerates through A's hub, and A's SWD reaches B's
MCU.

#photo[Two Portwright plugged into each other like a short train]

== Steps

#steps(cli: [
  ```sh
  pw device list --tree
  #  A  bench-a   (port 3-1)
  #  └ B  bench-b (port 3-1.2, through A)
  pw -d bench-a power on                 # B runs from A's switched VBUS
  probe-rs info --probe c0de:cafe:<serial of A> --protocol swd    # B's STM32G0B1
  cd fw/portwright && probe-rs run --probe c0de:cafe:<serial of A> --chip STM32G0B1CETx \
      target/thumbv6m-none-eabi/release/portwright
  pw -d bench-a power cycle              # reset B the hard way, from A
  ```
])

== What you should see

probe-rs finds B's MCU through A, flashes it, and shows B's defmt log; B also stays on USB through A's hub. A's meter
shows what B draws, so you see B's firmware's power as you change it.

#tip[This is how Portwright firmware is developed without a J-Link: a board on the bench is flashed, logged and power
  cycled by another one.]

== What next

- Lab "Daisy chain": more than two.
