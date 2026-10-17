#import "../lib.typ": *

#show: lab.with(
  title: [Donguru on the network],
  time: "15 min",
  needs: ([Donguru], [a computer, nothing installed]),
  features: ("API-21", "API-28", "GUI-1"),
)

== Goal

Reach Donguru without installing anything: it shows up as a small USB network adapter, serves a start page, and
passes its UART on as a TCP port.

== Steps

#steps(cli: [
  ```sh
  ip addr                        # a new interface (usb0 / enx...) with 10.83.17.2/30
  xdg-open http://10.83.17.1     # the start page: loads the Donguru app in the browser
  nc 10.83.17.1 2323             # the SBU UART as a raw TCP stream (like ser2net)
  dg net status                  # address, sockets, who is connected
  ```
], gui: [
  The start page loads the same app as `dg ui`, compiled for the browser: power, meter, UART terminal.
])

== What you should see

The interface comes up within a few seconds on Linux, macOS and Windows 11 with no driver. The start page needs
internet once to load the app; without it, it says so and links the docs.

#tip[Two dongles on one computer get two different subnets, derived from their serial numbers, so they never clash.]

== What next

- Lab "WebUSB": the same app from our website, straight over USB, in Chrome.
