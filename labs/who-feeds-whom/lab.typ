#import "../lib.typ": *

#show: lab.with(
  title: [Who feeds whom?],
  time: "10 min",
  needs: ([Portwright], [a phone that can charge other devices, or a USB-C power bank]),
  features: ("API-12", "CLI-11", "API-24", "CLI-25"),
)

== Goal

See power flow both ways through Portwright: the computer charging a power bank, then the power bank feeding the
computer. Learn what the sign of the current means and who decides the direction on USB-C.

== Setup

Portwright in the computer, a power bank (or a phone with reverse charging) on its plug.

#photo[Laptop, Portwright, power bank, with an arrow drawn for each direction]

== Steps

#steps(cli: [
  ```sh
  pw cc status                  # who is source and who is sink on each side
  pw power on
  pw power watch                # current > 0: computer -> DUT (the power bank charges)
  pw cc plug --role source      # Portwright presents Rp on the plug: "I am a charger"
  pw cc plug --role sink        # Rd on the plug: "charge me", the bank starts to feed back
  pw power watch                # current < 0: DUT -> computer
  ```
], python: [
  ```python
  import portwright
  pw = portwright.open()
  pw.plug.cc_role = "sink"            # ask the power bank to be the source
  for m in pw.meter.watch(interval=0.5, count=10):
      print(f"{m.current:+.3f} A", "->DUT" if m.current > 0 else "<-DUT")
  ```
])

== What you should see

`pw power watch` prints a direction column next to the current: `→ DUT` while the computer charges the bank,
`← DUT` once the plug presents a sink and the bank feeds back. The laptop's battery icon may show "charging".

#safety[Two sources on one VBUS fight. Portwright only lets the DUT feed back after the plug is a sink and the computer
  side has stopped sourcing (`pw cc status` says so); a host that keeps sourcing gets the switch opened.]

== What next

- Lab "Reverse power": a whole laptop charged from a power bank through Portwright, metered.
