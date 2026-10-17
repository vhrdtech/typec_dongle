#import "../lib.typ": *

#show: lab.with(
  title: [Make your board Donguru-ready],
  time: "1 h of reading and schematic",
  needs: ([your board's schematic], [KiCad or similar]),
  features: ("PKG-8", "API-24", "FW-15", "FW-7"),
)

== Goal

Design a product's USB-C port so that one cable gives power, USB, a serial console and SWD, with Donguru as the
other end: no debug header on the product, and the debug functions stay off for normal users.

== The convention

- SBU1 / SBU2 carry SWD (SWDIO / SWCLK) or a UART (TX / RX), never both at once; a small analog mux on the board
  picks one, and is off (Hi-Z) by default.
- The mux enables only when Donguru asks: Donguru presents itself as a USB-C debug accessory on CC; a normal
  charger or computer never does that, so the pins stay quiet in normal use.
- 3.3 V or 1.8 V signal levels; Donguru follows the board's I/O voltage.
- Orientation doesn't matter: Donguru reads the CC polarity and swaps the lines.

== Steps

1. Copy the reference schematic (mux, CC detection, ESD) from the docs page.
2. Route SBU1/SBU2 with the D+/D- pair's care: short, away from VBUS.
3. Check with `dg sbu status` on the first prototype: the board answers the debug-accessory handshake and SWD
   connects in both orientations.

#todo[Reference schematic and KiCad snippet, from B129A's SWD_SEL circuit made general.]

== What next

- Lab "Flash and debug over the USB-C cable" on your new board.
