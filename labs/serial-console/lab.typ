#import "../lib.typ": *

#show: lab.with(
  title: [Serial console over the USB-C cable],
  time: "20 min",
  needs: ([Donguru], [a board with UART on its USB-C SBU pins, or the SBU breakout and two wires]),
  features: ("FW-7", "API-14", "API-20", "CLI-20", "PKG-6"),
)

== Goal

Get a board's serial console through the same USB-C connector that powers it: no extra USB-serial adapter, and the
cable can go in either way round.

== Setup

Boards that follow Donguru's SBU convention have TX and RX on SBU1 and SBU2. Any other board: the SBU breakout
(a USB-C receptacle with SBU1, SBU2, GND on pins) and two wires to its UART pins.

#photo[A dev board with the SBU breakout, two wires to its UART header]

== Steps

#steps(cli: [
  ```sh
  dg uart open --baud 115200            # a terminal: type, see the console; Ctrl-] to leave
  dg uart monitor                       # listen to both SBU lines at once, tagged A / B
  dg uart pty                           # /dev/pts/7 for tools that want a tty
  picocom -b 115200 /dev/ttyACM0        # or the plain CDC-ACM port, no dg needed
  ```
], python: [
  ```python
  import donguru
  dg = donguru.open()
  with dg.uart.open(baud=115200) as port:      # flip handled by Donguru
      port.write(b"help\r\n")
      print(port.read_until(b"> ", timeout=2.0).decode())
  ```
])

== What you should see

The console appears whichever way the cable is in: Donguru finds TX by the idle-high level and swaps the lines
itself. `dg sbu status` shows `Uart (owner: WireWeaver)` while it is open.

#tip[Only one owner at a time: while picocom has `/dev/ttyACM0`, `dg uart open` says so instead of fighting over
  the port, but `dg uart monitor` still listens along.]

== What next

- Lab "I2C on the plug": the same two SBU wires, another bus.
