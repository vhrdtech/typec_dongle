#import "../lib.typ": *

#show: lab.with(
  title: [What does this accessory do?],
  time: "30 min",
  needs: ([Donguru], [a USB-C accessory or board of your own with an unknown port: an old prototype, a dock]),
  features: ("API-15", "API-18", "CLI-40", "CLI-20"),
)

== Goal

Find out what an undocumented USB-C device of your own uses its extra pins for: pull-ups on CC (what role it
expects), levels on SBU and A2/B2, a serial console hiding somewhere, and at what speed.

== Steps

#steps(cli: [
  ```sh
  dg probe pins
  #  CC1   Rd 5.1 k   (a sink: it wants power)
  #  CC2   open
  #  SBU1  3.28 V idle, toggling       -> looks like a UART TX
  #  SBU2  pulled up 10 k
  #  A2    0 V,  B2  0 V
  dg uart detect --line sbu1        # 115200 8N1, 4 lines seen: "U-Boot 2023.04 ..."
  dg uart monitor
  ```
])

== What you should see

`probe pins` measures each pin floating, with a weak pull-up and with a weak pull-down, and names what it sees.
`uart detect` tries the usual baud rates on an idle-high toggling line and stops at the one that gives readable
text.

#safety[Probing only drives pins weakly (pull resistors, 3.3 V), but a pin you don't know could be anything: start
  with the device unpowered and `dg power on --trip 100mA`.]

== What next

- Lab "Serial console over the USB-C cable" once you know the pins.
