#import "../lib.typ": *

#show: lab.with(
  title: [Is the SWD link clean?],
  time: "10 min",
  needs: ([Donguru], [a board with SWD on SBU]),
  features: ("CLI-18", "CORE-5"),
)

== Goal

Before blaming the firmware for a weird crash, check the debug link: write and read back the target's RAM with
test patterns at several SWD speeds and find the fastest clean one.

== Steps

#steps(cli: [
  ```sh
  dg swd memtest --chip STM32G0B1CETx --speed 400,1000,2000,3000 --passes 3
  #  400 kHz   ok   144 KiB × 3 · 10 patterns
  #  1000 kHz  ok
  #  2000 kHz  ok
  #  3000 kHz  3 errors (bit 7 at 0x20004a10, ...)
  dg swd memtest --chip STM32G0B1CETx --speed 2000 -f json | jq .errors
  ```
])

== What you should see

A short cable and a board with a clean SBU path pass up to 2-3 MHz. Errors that appear only at high speed point at
the cable, the level shifter or long wires: use the highest clean speed with some margin.

== What next

- Lab "Flash and debug over the USB-C cable" with the speed you found.
