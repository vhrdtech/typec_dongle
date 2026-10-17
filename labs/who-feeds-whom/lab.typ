#import "../lib.typ": *

#show: lab.with(
  title: [Who feeds whom?],
  time: "10 min",
  needs: ([Donguru], [a phone that can charge other devices, or a USB-C power bank]),
  features: ("API-12", "CLI-11", "API-24", "CLI-25"),
)

== Goal

See power flow both ways through Donguru: the computer charging a power bank, then the power bank feeding the
computer. Learn what the sign of the current means and who decides the direction on USB-C.

== Setup

Donguru in the computer, a power bank (or a phone with reverse charging) on its plug.

#photo[Laptop, Donguru, power bank, with an arrow drawn for each direction]

== Steps

#steps(cli: [
  ```sh
  dg cc status                  # who is source and who is sink on each side
  dg power on
  dg power watch                # current > 0: computer -> DUT (the power bank charges)
  dg cc plug --role source      # Donguru presents Rp on the plug: "I am a charger"
  dg cc plug --role sink        # Rd on the plug: "charge me", the bank starts to feed back
  dg power watch                # current < 0: DUT -> computer
  ```
], python: [
  ```python
  import donguru
  dg = donguru.open()
  dg.plug.cc_role = "sink"            # ask the power bank to be the source
  for m in dg.meter.watch(interval=0.5, count=10):
      print(f"{m.current:+.3f} A", "->DUT" if m.current > 0 else "<-DUT")
  ```
])

== What you should see

`dg power watch` prints a direction column next to the current: `→ DUT` while the computer charges the bank,
`← DUT` once the plug presents a sink and the bank feeds back. The laptop's battery icon may show "charging".

#safety[Two sources on one VBUS fight. Donguru only lets the DUT feed back after the plug is a sink and the computer
  side has stopped sourcing (`dg cc status` says so); a host that keeps sourcing gets the switch opened.]

== What next

- Lab "Reverse power": a whole laptop charged from a power bank through Donguru, metered.
