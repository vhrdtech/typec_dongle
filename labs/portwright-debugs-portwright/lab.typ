#import "../lib.typ": *

#show: lab.with(
  title: [Donguru debugs Donguru],
  time: "20 min",
  needs: ([two Donguru],),
  features: ("FW-15", "FW-17", "FW-20", "CLI-36"),
)

== Goal

Plug one Donguru into another and let the first flash and debug the second: Donguru's own SWD pins are on its
receptacle's SBU lines, so the plug of one fits the debug port of the other.

== Setup

Donguru A in the computer, Donguru B's receptacle on A's plug. B enumerates through A's hub, and A's SWD reaches B's
MCU.

#photo[Two Donguru plugged into each other like a short train]

== Steps

#steps(cli: [
  ```sh
  dg device list --tree
  #  A  bench-a   (port 3-1)
  #  └ B  bench-b (port 3-1.2, through A)
  dg -d bench-a power on                 # B runs from A's switched VBUS
  probe-rs info --probe c0de:cafe:<serial of A> --protocol swd    # B's STM32G0B1
  cd fw/donguru && probe-rs run --probe c0de:cafe:<serial of A> --chip STM32G0B1CETx \
      target/thumbv6m-none-eabi/release/donguru
  dg -d bench-a power cycle              # reset B the hard way, from A
  ```
])

== What you should see

probe-rs finds B's MCU through A, flashes it, and shows B's defmt log; B also stays on USB through A's hub. A's meter
shows what B draws, so you see B's firmware's power as you change it.

#tip[This is how Donguru firmware is developed without a J-Link: a board on the bench is flashed, logged and power
  cycled by another one.]

== What next

- Lab "Daisy chain": more than two.
