#import "../lib.typ": *

#show: lab.with(
  title: [A rack console from dongles],
  time: "a weekend",
  needs: ([a small Linux computer as the controller], [one Portwright per board in the rack], [powered USB hubs]),
  features: ("CORE-10", "CLI-33", "FW-31", "CLI-36", "API-20", "GUI-3", "CORE-8"),
)

== Goal

Build a small baseboard management controller (BMC) for a rack of single-board computers: from anywhere on your
network, see each board's console, power-cycle it, watch its power draw, and let hung boards reset themselves.

== Setup

The controller with powered hubs; each SBC gets a Portwright between its USB-C power input and the hub. Each dongle is labelled
with its slot.

#photo[A shelf of SBCs, the controller at the end, a bundle of dongles]

== Steps

#steps(cli: [
  ```sh
  # on the controller:
  pw each --all -- watchdog device --uart "login:" --timeout 10m --grace 2m   # each board resets itself if stuck
  pw serve --listen 0.0.0.0:7700 --auth ~/.config/portwright/rack-keys     # share the rack, with keys
  # from your laptop:
  pw --remote rack-host device list
  pw --remote rack-host -d slot4 uart open     # slot 4's console
  pw --remote rack-host each 'slot*' -f jsonl -- power meter
  ```
], gui: [
  `pw ui --remote rack-host` shows the rack as a grid: one tile per slot with its power, the last console line, and
  buttons for console and power cycle.
])

== What you should see

Every board's console and power from one place; boards that hang come back on their own and the dongle counts how
often. The controller itself stays simple: no daemon of its own beyond `pw serve`.

#safety[`pw serve` gives power and console control of the rack to whoever holds a key: keep it on a private network,
  one key per person, and never without `--auth`.]

== What next

- Back to lab 1, with a rack of dongles to label.
