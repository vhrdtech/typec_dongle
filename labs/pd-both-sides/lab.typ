#import "../lib.typ": *

#show: lab.with(
  title: [USB-PD on both sides],
  time: "30 min",
  needs: ([Donguru], [a PD charger], [a phone or laptop that charges over PD]),
  features: ("API-23", "API-30", "CLI-25", "FW-28", "GUI-1"),
)

== Goal

Understand a USB Power Delivery negotiation by taking part in it: Donguru is the sink towards the charger and the
source towards your phone, logs both conversations and passes power through. You see what your own device asks for
and why.

== Setup

Donguru can't sit invisibly between a charger and a phone: it ends the CC line on both sides. Instead it negotiates
with each side itself and offers the phone what it got from the charger (a "PD bridge"). The charger takes the
receptacle, so this runs standalone and the record is read afterwards.

#photo[Charger, Donguru, phone in a row; Donguru's LEDs show the contract]

== Steps

#steps(cli: [
  ```sh
  dg settings set standalone pd-bridge --max-voltage 9V    # offer the phone at most 9 V
  # charger -> Donguru -> phone, let it charge a minute, then Donguru back to the computer:
  dg pd last --both
  #  charger: 5 V 3 A · 9 V 3 A · 15 V 3 A · 20 V 3.25 A        contract 9 V 2.22 A
  #  phone:   offered 5 V 3 A · 9 V 2.22 A   requested 9 V 2.0 A (object 2), accepted 38 ms later
  dg pd last --both --messages      # every message both ways, decoded, with timestamps
  ```
])

== What you should see

The phone asks for the highest offer it supports, usually within 100-300 ms of plugging in. Messages like
`Source_Capabilities`, `Request`, `Accept`, `PS_RDY` appear on each side in order.

#safety[Donguru never offers the phone more than `--max-voltage`, and the guard opens the plug if the charger side
  goes above it.]

== What next

- Lab "PD edge cases": check how your own device copes with unusual but valid offers.
