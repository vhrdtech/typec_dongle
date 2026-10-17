#import "../lib.typ": *

#show: lab.with(
  title: [How much does my phone charge with?],
  level: "first-hour",
  time: "15 min",
  needs: ([Donguru], [a phone with a USB-C port], [a USB-C charger or the computer's port]),
  features: ("API-12", "CLI-11"),
)

== Goal

See the voltage, current and power a phone takes while it charges, and how much energy went in.

== Setup

Donguru's receptacle goes to the computer, its plug into the phone. Donguru switches the computer's VBUS through to
the phone and measures it on the way.

#photo[Laptop, Donguru, phone on the desk]

#safety[Donguru passes the computer's VBUS on as it is. `dg power on` allows up to 5.5 V; with a USB-PD charger
  that negotiates 9 V or more, VBUS is cut unless you allow the higher voltage with `--allow-voltage`.]

== Steps

#steps(cli: [
  ```sh
  dg power on                          # VBUS to the phone, cut above 5.5 V
  dg power watch --interval 500ms      # V, A, W, live; Ctrl-C to stop
  dg power totals --reset              # start counting charge and energy from zero
  # ... let it charge for 10 minutes ...
  dg power totals                      # mAh and Wh since the reset
  dg power watch -f jsonl >> phone.jsonl   # or log it for a plot later
  ```
])

== What you should see

At 5 V most phones take 0.5 to 2 A: 2.5 to 10 W. The current falls as the battery fills; a full phone takes only
tens of mA. A positive current means the computer feeds the phone; a phone that can charge other devices may show a
negative one.

#todo[A plot of a real charge curve from `phone.jsonl`, and the GIF of `dg power watch`.]

== What next

- Log a whole night and plot the charge curve.
- Compare cables: the same phone through a good and a cheap cable.
