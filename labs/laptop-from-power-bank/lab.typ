#import "../lib.typ": *

#show: lab.with(
  title: [Charge a laptop from a power bank, through Donguru],
  time: "30 min",
  needs: ([Donguru], [a USB-C PD power bank], [a laptop that charges over USB-C]),
  features: ("API-24", "API-32", "API-23", "CLI-25", "CLI-28"),
)

== Goal

Turn the usual direction around: the power bank on Donguru's plug is the source, the laptop on the receptacle is the
sink, and the laptop stays the USB host so `dg` keeps working while it is being charged through the dongle it talks
to.

== Steps

#steps(cli: [
  ```sh
  dg cc plug --role sink                    # Donguru asks the power bank for power
  dg cc host --role source --pd 15V         # and offers the laptop up to 15 V (PD source on the receptacle)
  dg cc status
  #  plug: power bank sources 15 V 3 A · host: laptop sinks, 15 V 2.9 A contract · data: laptop is host (DR_Swap)
  dg power watch                            # ← DUT 2.71 A, 40.7 W: the bank charges the laptop
  ```
])

== What you should see

The laptop shows "charging" and `dg` keeps running on it. In USB-C, the side that sources power is the USB host by
default; Donguru swaps the data role with PD (`DR_Swap`) so the laptop stays host while it sinks power.

#safety[Tens of watts through a dongle: the switch is rated 5 A, so up to 100 W at 20 V is the design target, to be
  confirmed on rev B. Keep Donguru in the open, watch `dg power status`'s temperature, and set the guard to the voltage
  you agreed. Laptops without PD data-role swap lose the USB link: the lab then ends there.]

== What next

- Lab "Who feeds whom?" for the basics of source and sink.
