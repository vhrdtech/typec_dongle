#import "../lib.typ": *

#show: lab.with(
  title: [Read Donguru's own crash reports],
  time: "10 min",
  needs: ([Donguru],),
  features: ("CLI-19", "FW-13", "API-9", "API-10"),
)

== Goal

See how a device can report its own crashes, log and counters to the host without a debug probe, using Donguru
itself as the example, and take the pattern home for your own firmware.

== Steps

#steps(cli: [
  ```sh
  dg device reset-cause         # last reset: PowerOn · boots 41   (or: Fault, with the crash record)
  dg device log                 # the dongle's own defmt log, decoded with its ELF found by build id
  dg device log --follow        # live
  dg device counters            # cnt counters: hub recoveries, USB resets, guard trips ...
  dg device reset-cause --clear
  ```
])

== What you should see

After a normal plug-in the cause is `PowerOn`. If the dongle ever crashed, `reset-cause` shows the fault, the program
counter and the line it maps to, and `device log` has the lines before the crash: the firmware keeps them in RAM that
survives the reset.

#tip[The same comes for free in any firmware built on the embedded_bedrock template (WireWeaver device contract):
  `ww info`, `ww report`, `ww log` work on it with no device-specific code on the host.]

== What next

- Lab "Write your own WireWeaver method": add something of your own to the firmware.
