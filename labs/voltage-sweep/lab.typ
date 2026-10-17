#import "../lib.typ": *

#show: lab.with(
  title: [Down to the last volt],
  time: "30 min",
  needs: ([Donguru], [a USB-C charger with PPS (programmable supply)], [a device that runs from VBUS]),
  features: ("API-23", "FW-28", "FW-33", "FW-29", "CLI-37"),
)

== Goal

Find out at what supply voltage a device stops working, and whether it comes back cleanly when the voltage returns:
the brown-out behaviour you'd otherwise need a lab power supply and a script for.

== Setup

A PPS charger can be asked for any voltage from 3.3 V to 5.9 V (and higher) in 20 mV steps. Donguru asks for a sweep
while the device runs, logs the meter and the device's current, and you read the result afterwards. Because the
charger takes the receptacle, Donguru runs the sweep on its own.

#photo[A PPS charger, Donguru and a dev board on the desk; no computer]

== Steps

#steps(cli: [
  ```sh
  # write the sweep into the dongle while it is on the computer
  dg seq load sweep.toml          # steps: PPS 5.0 V, then -50 mV every 2 s down to 3.3 V, then back up
  dg settings set standalone run-seq
  # move Donguru to the PPS charger with the device on its plug, wait for the green LED, then back:
  dg seq result -f csv > sweep.csv
  dg power plot sweep.csv --x voltage --y current
  ```
  ```toml
  # sweep.toml
  [[step]]
  pd = { pps = "5.0V", current = "1A" }
  [[step]]
  power = { mode = "on", max_voltage = "5.9V" }
  [[step]]
  sweep = { from = "5.0V", to = "3.3V", step = "-50mV", hold = "2s" }
  [[step]]
  sweep = { from = "3.3V", to = "5.0V", step = "50mV", hold = "2s" }
  ```
])

== What you should see

The current stays flat until the device's regulator drops out, then jumps (the device resets and boots again) or
falls to nearly zero. On the way up, the voltage where it starts again shows its brown-out hysteresis.

#safety[The plug follows the charger's voltage; the sequence's `max_voltage` caps it. Check what the device takes
  before you sweep above 5 V.]

== What next

- Lab "Charger inspector": which of your chargers have PPS.
