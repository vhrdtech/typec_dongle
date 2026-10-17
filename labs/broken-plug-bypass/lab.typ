#import "../lib.typ": *

#show: lab.with(
  title: [When the plug breaks],
  time: "1 h",
  needs: ([Donguru rev B], [a USB-C cable to cut, or wires], [a soldering iron]),
  features: ("FW-15", "API-11"),
)

== Goal

A USB-C plug wears out. Rev B has a solder pad for every plug pin in use, so a broken plug can be bypassed with a
cable soldered on, or Donguru can be wired to a board that has no USB-C port at all.

#mod[Solder a cut USB-C cable (or single wires) to the labelled pads next to the plug: VBUS and GND with thick wire,
  D+/D- as a twisted pair kept short, CC and SBU1/2 as needed. Cut or desolder the broken plug first if it shorts.]

== Steps

1. Check each wire with a multimeter against the pad labels before powering.
2. `dg power on --trip 200mA` with nothing connected: no current should flow.
3. Connect the board; everything works as through the plug, orientation fixed to `normal`.

== What you should see

The board enumerates, the meter reads its current, SWD and UART work, as before. Without CC on the other side, set
`dg sbu orientation normal` by hand.

== What next

- Lab "Make your board Donguru-ready", for the next board.
