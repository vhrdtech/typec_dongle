#import "../lib.typ": *

#show: lab.with(
  title: [The relay and the header's power],
  time: "15 min",
  needs: ([Donguru], [jumper wires], [a device with a push button to press, or a 5 V fan]),
  features: ("CLI-16", "API-11", "CORE-4"),
)

== Goal

Use the header's isolated relay to "press" a button on another device, and the header's switched VBUS to power a
second small load.

== Setup

Header pins 11 and 12 (the relay contact) go across the other device's button, like a finger would. Pin 10 (switched
VBUS) and a GND pin power the fan.

#photo[Wires from Donguru's header to a dev board's RESET button and to a small fan]

== Steps

#steps(cli: [
  ```sh
  dg header relay on; sleep 0.2; dg header relay off    # a 200 ms press
  dg header vbus on                     # fan spins; shows the voltage it passes on
  dg header vbus off
  ```
], python: [
  ```python
  import donguru, time
  dg = donguru.open()
  def press(seconds=0.2):
      dg.header.relay = True; time.sleep(seconds); dg.header.relay = False
  press()        # reset the other board
  press(3.0)     # a long press: many boards enter their bootloader
  ```
])

== What you should see

The other device reacts as if its button was pressed. The relay is a photo-MOS: it isolates, so the other device
can be at any ground, but it is for signals, not motors.

#safety[Header VBUS is the computer's VBUS as it is, up to 20 V with a PD charger. The command shows it and needs
  `--allow-voltage` above 5.5 V. The relay is rated for low-voltage signals: check the datasheet values before
  switching anything else.]

== What next

- Lab "Hung board": press a stuck board's reset from a script.
