#import "../lib.typ": *

#show: lab.with(
  title: [Hot-plug torture],
  time: "30 min + the run",
  needs: ([Donguru], [the USB device under test], [its VID:PID (`lsusb`)]),
  features: ("CLI-26", "CORE-4"),
)

== Goal

Plug and unplug a USB device a thousand times without touching it, and count the times it didn't come back or came
back slowly. Firmware USB stacks and host drivers fail here in ways a person never reproduces by hand.

== Steps

#steps(cli: [
  ```sh
  dg usb cycle --repeat 1000 --off 500ms --wait c0de:cafe --timeout 5s -f jsonl > hotplug.jsonl
  #  [=====>     ] 412/1000 · ok 411 · slow 1 (> 2 s) · failed 0 · median 0.71 s
  dg usb cycle --repeat 1000 --with-power --wait c0de:cafe   # the same, with VBUS too
  ```
], python: [
  ```python
  import donguru, statistics
  dg = donguru.open()
  times, failed = [], 0
  for i in range(1000):
      try:
          times.append(dg.usb.cycle(off=0.5, wait_for="c0de:cafe", timeout=5.0))
      except donguru.Timeout:
          failed += 1
          dg.power.cycle(off=2.0)            # bring it back for the next round
  print(f"failed {failed}, median {statistics.median(times):.2f} s, max {max(times):.2f} s")
  ```
])

== What you should see

A healthy device comes back every time in a steady 0.3-1.5 s. A slow tail or a few failures point at the device's
USB stack (it misses a reset) or the host driver. The jsonl file has every attempt for a histogram.

== What next

- Lab "Power-cut test": the same idea for power instead of data.
