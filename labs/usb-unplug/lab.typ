#import "../lib.typ": *

#show: lab.with(
  title: [Unplug without unplugging],
  time: "10 min",
  needs: ([Donguru], [a USB device: a flash drive, a keyboard, a dev board]),
  features: ("CLI-6", "CLI-26"),
)

== Goal

Make a USB device vanish from the computer and come back, with power kept on or cut too, to see how the operating
system and your application cope with a device that goes away.

== Setup

#photo[A USB flash drive on Donguru's plug, the file manager open next to it]

== Steps

#steps(cli: [
  ```sh
  dg power on
  dg usb disconnect             # data lines cut: the OS sees an unplug, the device keeps power
  dg usb connect                # it enumerates again
  dg usb cycle --off 1s --wait 0bda:9210   # cut, reconnect, wait until the OS lists that VID:PID again
  dg usb cycle --off 1s --with-power       # a real unplug: data and VBUS together
  ```
], python: [
  ```python
  import donguru
  dg = donguru.open()
  t = dg.usb.cycle(off=1.0, wait_for="0bda:9210")   # seconds until the OS saw it again
  print(f"re-enumerated after {t:.2f} s")
  ```
])

== What you should see

On `disconnect` the file manager drops the drive; `dmesg` (Linux) shows a USB disconnect. With power kept on, a
device with its own state (a dev board running firmware) carries on and only its USB restarts. `dg usb cycle
--wait` prints how long the OS took to see the device again: usually 0.3 to 1.5 s.

#safety[Cutting data while a drive is being written can corrupt its file system: that is the point of the test, so
  use a drive with nothing on it you need.]

== What next

- Lab "Hot-plug torture": the same cycle a thousand times, counting the ones that failed.
