#import "../lib.typ": *

#show: lab.with(
  title: [Pull the plug at the worst moment],
  time: "30 min + the run",
  needs: ([Donguru], [a device that writes to flash: a data logger, a board with a settings store]),
  features: ("CLI-11", "FW-32", "CLI-35", "CORE-4"),
)

== Goal

Cut the power at random moments while the device writes its flash, thousands of times, and check it boots and its
data is intact every time. This is how a settings store or a file system proves it is power-cut safe.

== Steps

#steps(cli: [
  ```sh
  # cut at a random point 0-200 ms after the device prints "writing", then check its boot message
  dg uart open --baud 115200 --log run.log &
  for i in $(seq 1000); do
    dg uart expect "writing" --timeout 10s
    dg power cycle --after random:0..200ms --off 50ms
    dg uart expect "boot ok" --timeout 5s || { echo "round $i failed"; break; }
  done
  dg power pulse --off 300us            # a dropout shorter than a cycle: brown-out, not a cut
  ```
], python: [
  ```python
  import donguru, random
  dg = donguru.open()
  with dg.uart.open(baud=115200) as con:
      for i in range(1000):
          con.expect(b"writing", timeout=10)
          dg.power.cycle(after=random.uniform(0, 0.2), off=0.05)
          con.expect(b"boot ok", timeout=5)      # raises donguru.Timeout on a bad boot
  ```
])

== What you should see

Each round prints the cut time and the boot result. A store that isn't power-cut safe fails within a few hundred
rounds, often at the same spot of its write sequence: the cut time in the log tells where.

#tip[`power pulse` makes a dropout of microseconds to milliseconds, timed by the firmware: the device's brown-out
  detector and its bulk capacitors decide whether it notices. Sweep the length to find where it starts to reset.]

== What next

- Lab "Fault injection campaign": power cuts, USB cuts and noise together, for days.
