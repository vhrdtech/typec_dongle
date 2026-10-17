#import "../lib.typ": *

#show: lab.with(
  title: [Hello Donguru],
  time: "5 min",
  needs: ([Donguru], [a computer with a USB-C port and `dg` installed]),
  features: ("CLI-7", "CLI-19", "CLI-21", "CORE-4", "GUI-1"),
)

== Goal

Plug Donguru in, check that the computer sees it, and give it a name, so that later labs (and later you, with three
dongles on the desk) know which one is which.

== Setup

#photo[Donguru in a laptop's USB-C port, nothing on its plug yet]

== Steps

#steps(cli: [
  ```sh
  dg device list          # every Donguru on this computer
  dg info                 # firmware, board revision, serial, label, last reset
  dg device label bench-1 --notes "left USB-C port of the desk PC"
  dg device label         # read it back
  dg led identify 5s      # which one is it? its LEDs blink for 5 s
  ```
], python: [
  ```python
  import donguru                      # uv add donguru
  for d in donguru.list():
      print(d.serial, d.label)
  dg = donguru.open("bench-1")        # by label, serial or index
  print(dg.info())
  dg.leds.identify(5.0)
  ```
], gui: [
  `dg ui` lists every dongle on the left with its label; clicking the blink icon makes that one identify itself.
])

#tip[With more than one dongle, pick one with `--device` (serial number, index or USB path), or
  `dg device select SERIAL` to make it the default for this folder.]

== What you should see

`dg device list` shows one line per dongle with its serial number. `dg info` shows the firmware version, the board
(B153 and its revision) and the reset cause; after the label command the label appears there too.

#todo[Paste real output from a B153B once the labs are tested on rev B.]

== What next

Measure what a phone draws while charging: the next lab.
