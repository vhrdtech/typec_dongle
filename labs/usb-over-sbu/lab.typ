#import "../lib.typ": *

#show: lab.with(
  title: [Two USB links in one cable],
  time: "30 min",
  needs: ([Donguru Plus], [a board with a second USB interface on its SBU pins (a debug or bootloader USB)]),
  features: ("API-14", "FW-22", "CLI-45"),
)

== Goal

Some boards have two USB devices: the product's own USB on D+/D- and a debug USB (a bridge chip, a second MCU) on
SBU. Donguru Plus routes the SBU pair to its hub's fourth port, so both show up on the computer through one cable.

== Steps

#steps(cli: [
  ```sh
  dg device info                 # variant: Plus
  dg sbu usb on                  # SBU1/2 -> hub port 4
  lsusb                          # the board's own device and its debug device, both behind Donguru
  dg sbu usb off                 # SBU back to Hi-Z
  ```
])

== What you should see

Both USB devices enumerate. While the SBU pair carries USB, UART, SWD and I2C on SBU answer "in use": one owner at a
time.

== What next

- Lab "A rack console": boards with a debug USB make good rack citizens.
