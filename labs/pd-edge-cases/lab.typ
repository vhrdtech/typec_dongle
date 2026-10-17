#import "../lib.typ": *

#show: lab.with(
  title: [PD edge cases for your own device],
  time: "1 h",
  needs: ([Donguru], [a device of your own with a USB-PD sink]),
  features: ("API-30", "CLI-25", "CLI-44"),
)

== Goal

Before a certification lab sees your device, check how its PD sink handles the unusual offers real chargers make:
Donguru acts as a PD source and plays a list of valid-but-awkward scenarios against it, recording how it answers.

== Steps

#steps(cli: [
  ```sh
  dg pd scenarios list
  #  many-pdos       seven fixed supplies, 5 to 20 V
  #  pps-only        5 V plus a PPS range, no fixed 9 V
  #  low-current     every supply limited to 0.5 A
  #  new-caps        offer changes while charging (a charger whose other port got busy)
  #  hard-reset      the source resets the contract mid-charge
  dg pd scenarios run --all -o pd-report.html
  #  many-pdos ok (asked 9 V 3 A) · pps-only ok · low-current ok · new-caps FAIL: no Request within 30 ms
  ```
])

== What you should see

Each scenario says what the device asked for and how long it took to answer. A sink that ignores new capabilities,
or takes too long, shows up before a customer's charger finds it.

#tip[These are spec-conformant scenarios for testing your own design, not a compliance test: certification still
  needs a USB-IF test lab and their tools.]

== What next

- Lab "USB-PD on both sides" to see a real charger's offers.
