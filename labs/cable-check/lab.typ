#import "../lib.typ": *

#show: lab.with(
  title: [Is it the cable?],
  time: "20 min",
  needs: ([Donguru], [two or three USB-C cables], [a device that draws 1 A or more]),
  features: ("API-12", "API-27", "CLI-31", "CLI-30"),
)

== Goal

Find out which of your cables is good: what its e-marker says it is, and how much voltage it loses under load.

== Setup

The cable under test goes between the computer (or charger) and Donguru's receptacle; the load on Donguru's plug.

#photo[Three cables on the desk, one in use between the laptop and Donguru]

== Steps

#steps(cli: [
  ```sh
  dg cable info                 # the e-marker of the cable on the receptacle, if it has one
  #  e-marked · 5 A · 20 V · USB 3.2 Gen 2 · 1 m · vendor 0x2B1D
  dg power off; dg power meter --average 2s      # voltage without load
  dg power on;  dg power meter --average 2s      # with the load
  #  5.12 V -> 4.83 V at 1.20 A: 0.29 V drop, 240 mΩ (port + cable + connectors)
  ```
])

== What you should see

A good 1 m cable with a 1 A load drops under 0.1 V. A thin charging-only cable can drop 0.3-0.5 V and leave a
device with too little. A cable without an e-marker is limited to 3 A by the spec.

#tip[Donguru measures the voltage only on its receptacle side, so the drop includes the computer's port. Compare
  cables on the same port, one after another.]

== What next

- Lab "Charger inspector": what the charger at the other end of the cable offers.
