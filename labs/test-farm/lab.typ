#import "../lib.typ": *

#show: lab.with(
  title: [A test farm],
  time: "1-2 h",
  needs: ([several Portwright], [a powered USB hub], [one board under test per dongle]),
  features: ("CORE-1", "CLI-7", "API-19", "CLI-36", "CORE-10", "CORE-8"),
)

== Goal

Run ten boards from one PC: each dongle named after its board, commands for one, a group or all of them, and the
farm reachable from your laptop over the network.

== Setup

#photo[A shelf with ten boards, each with its Portwright, on two powered hubs]

== Steps

#steps(cli: [
  ```sh
  pw device label rack1-slot3 --notes "nrf52 rev C"   # once per dongle, stored in it
  pw device list --tree                    # which dongle on which hub port, with labels
  pw -d rack1-slot3 power cycle
  pw each 'rack1-*' -- power cycle         # in parallel, one result line per dongle
  pw each --all -f jsonl -- power meter    # the whole farm's power, one line each
  # on the farm PC:
  pw serve --listen 0.0.0.0:7700           # share its dongles on the network
  # on your laptop:
  pw --remote farm-pc -d rack1-slot3 uart open
  ```
], python: [
  ```python
  import portwright
  farm = portwright.connect("farm-pc")        # remote, same API as local
  for d in farm.list(label="rack1-*"):
      d.power.cycle(off=1.0)
  ```
])

== What you should see

`device list --tree` mirrors the shelf. `pw each` runs a command on every match at once and exits non-zero if any
failed, naming which. From the laptop, a remote dongle works like a local one, a bit slower.

#tip[Name dongles after the slot, not the board: when a board is swapped the label stays right.]

== What next

- Lab "A rack console": a small BMC for a rack of single-board computers.
