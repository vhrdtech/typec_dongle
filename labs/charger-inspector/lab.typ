#import "../lib.typ": *

#show: lab.with(
  title: [Charger inspector],
  time: "15 min",
  needs: ([Donguru], [a USB-C PD charger with a C-to-C cable], [a computer to read the result]),
  features: ("API-23", "CLI-25", "FW-28", "API-11", "API-12"),
)

== Goal

Find out what a USB Power Delivery charger offers: which voltages and currents, fixed or programmable (PPS), and
whether it keeps its promise under load.

== Setup

A charger has no USB data, and the receptacle is Donguru's only way to a computer. So Donguru inspects on its own:
plug the charger's cable into Donguru's receptacle, Donguru runs from the charger's 5 V, asks for its capabilities,
shows the result on its LEDs and keeps it. Then move Donguru back to the computer and read the record.

#photo[A 65 W charger feeding Donguru on the desk, the LEDs blinking the result]

== Steps

#steps(cli: [
  ```sh
  dg settings set standalone pd-inspect   # what Donguru does when it wakes up without a computer
  # plug the charger in: the status LED blinks once per fixed supply, red if PPS is offered too
  # move Donguru back to the computer, then:
  dg pd last                    # the last charger seen, with time since
  #  5 V 3 A · 9 V 3 A · 15 V 3 A · 20 V 3.25 A · PPS 3.3-21 V 3 A   (Anker 735, 41 s ago)
  dg pd last -f json > charger-anker-735.json
  ```
], gui: [
  The PD tab shows the last charger as cards, one per offer, and keeps a history of every charger seen.
])

== What you should see

A 65 W charger lists four or five fixed supplies and often a PPS range. A cheap 5 V-only adapter shows no PD at all
and `dg pd last` says "no PD source, 5 V Type-C current: 1.5 A" (from its Rp pull-up).

#safety[In this mode Donguru never asks for more than 5 V and keeps the plug off, whatever the settings say.]

== What next

- Lab "PD sniffing": watch a phone and a charger negotiate, with Donguru in the middle.
