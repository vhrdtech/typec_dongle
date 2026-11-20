#import "../lib.typ": *

#show: lab.with(
  title: [Hello Portwright],
  time: "5 min",
  needs: ([Portwright], [a computer with a USB-C port and `pw` installed]),
  features: ("CLI-7", "CLI-19", "CLI-21", "CORE-4", "GUI-1"),
)

== Goal

Plug Portwright in, check that the computer sees it, and give it a name, so that later labs (and later you, with three
dongles on the desk) know which one is which.

== Setup

#photo[Portwright in a laptop's USB-C port, nothing on its plug yet]

== Steps

#steps(cli: [
  ```sh
  pw device list          # every Portwright on this computer
  pw info                 # firmware, board revision, serial, label, last reset
  pw device label bench-1 --notes "left USB-C port of the desk PC"
  pw device label         # read it back
  pw led identify 5s      # which one is it? its LEDs blink for 5 s
  ```
], python: [
  ```python
  import portwright                      # uv add portwright
  for d in portwright.list():
      print(d.serial, d.label)
  pw = portwright.open("bench-1")        # by label, serial or index
  print(pw.info())
  pw.leds.identify(5.0)
  ```
], gui: [
  `pw ui` lists every dongle on the left with its label; clicking the blink icon makes that one identify itself.
])

#tip[With more than one dongle, pick one with `--device` (serial number, index or USB path), or
  `pw device select SERIAL` to make it the default for this folder.]

== What you should see

`pw device list` shows one line per dongle with its serial number. `pw info` shows the firmware version, the board
(B153 and its revision) and the reset cause; after the label command the label appears there too.

#todo[Paste real output from a B153B once the labs are tested on rev B.]

== What next

Measure what a phone draws while charging: the next lab.
