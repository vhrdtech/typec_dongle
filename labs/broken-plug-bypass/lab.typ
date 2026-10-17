#import "../lib.typ": *

#show: lab.with(
  title: [If the plug breaks],
  time: "1 h",
  needs: ([Donguru rev B], [a pigtail, a USB-C cable to cut, or wires], [a soldering iron]),
  features: ("FW-15", "API-11", "PKG-9"),
)

== Goal

A USB-C plug wears out after enough insertions. Rev B has a solder pad for every plug pin in use, so a worn or broken
plug can be replaced by a cable soldered on, and Donguru can also be wired straight to a board that has no USB-C port.

#mod[Solder a pigtail, a cut USB-C cable or single wires to the labelled pads next to the plug: VBUS and GND with
  thick wire, D+/D- as a twisted pair kept short, CC and SBU1/2 as needed. Desolder the broken plug first if it
  shorts.]

== What a cut cable can and can't do

A regular USB-C cable carries only one CC wire from end to end. Power, USB 2.0 data, the meter and the SBU functions
work through it, but the debug accessory mode needs both CC lines, so whatever a board enables only in that mode
(SWD on SBU, following Donguru's SBU convention) isn't available through a cut cable. Single wires straight to a
board, or a pigtail made for Donguru with both CC lines, keep everything.

== Steps

1. Check each wire with a multimeter against the pad labels before powering.
2. `dg power on --trip 200mA` with nothing connected: no current should flow.
3. Connect the board: it enumerates and the meter reads its current. With only one CC line, set
   `dg sbu orientation normal` by hand.

== What you should see

Everything that doesn't need the debug accessory mode works as through the plug. With a full pigtail or direct wires,
SWD works too.

#tip[Pigtails (a short cable with both CC lines, made for Donguru) may come as an accessory, and Donguru may come
  with one soldered on from the start for benches where a cable is handier than a plug.]

== What next

- Lab "Make your board Donguru-ready", for the next board.
