#import "../lib.typ": *

#show: lab.with(
  title: [A hardware CI runner],
  time: "1 h",
  needs: ([Portwright], [the board under test, on Portwright's plug], [a PC as a GitHub Actions self-hosted runner]),
  features: ("FW-15", "CLI-11", "CLI-35", "CLI-34", "CORE-8", "PKG-7"),
)

== Goal

Every push builds the firmware, flashes it into a real board, runs it, and passes or fails on what the board prints
and how much it draws: through one USB-C cable, with no person at the bench.

== Setup

The runner PC has Portwright in a USB port and the board on Portwright's plug. SWD and the console go over the plug's SBU
lines, power through the switch.

#photo[A mini PC, Portwright and a board in a box labelled "CI"]

== Steps

#steps(cli: [
  ```yaml
  # .github/workflows/hil.yml
  jobs:
    hil:
      runs-on: [self-hosted, portwright]
      steps:
        - uses: actions/checkout@v5
        - run: cargo build --release
        - uses: vhrdtech/portwright-action@v1          # installs pw, locks the dongle for this job
          with: { device: ci-board-1 }
        - run: pw power cycle --off 1s
        - run: probe-rs download --probe c0de:cafe --chip STM32G0B1CETx target/thumbv6m-none-eabi/release/app
        - run: pw power cycle --off 500ms
        - run: pw uart expect "ALL TESTS PASSED" --timeout 60s --fail-on "PANIC" --log console.log
        - run: pw power measure --for 10s -f json > power.json
  ```
])

== What you should see

The job's log has the console, the test result and the power numbers; a failure leaves the board's output as an
artifact. Two jobs never fight over the board: the action takes a lock on the dongle for the job.

== What next

- Lab "pytest fixture": the same steps from Python tests.
- Lab "Test farm": many boards, many runners.
