#import "../lib.typ": *

#show: lab.with(
  title: [Many clients at once],
  time: "20 min",
  needs: ([Portwright], [a computer], [optionally a second computer on the same network]),
  features: ("CORE-12", "API-33", "CLI-46", "API-21", "GUI-1", "CORE-4"),
)

== Goal

Use one Portwright from several places at the same time: the app in a browser, `pw` in two terminals, a Python notebook
and a CI script, and see how they share it without fighting.

== Setup

Portwright has two ways in that work at the same time: its USB control interface and its network interface (NCM). On the
computer, a local WireWeaver server holds the USB interface and serves it to any number of local programs over IPC,
so `pw`, Python and the app don't need the interface for themselves.

#photo[A browser with the Portwright app, two terminals and a notebook, all showing the same live current]

== Steps

#steps(cli: [
  ```sh
  ww serve &                     # the local WireWeaver server takes the USB interface, serves local clients
  pw power watch                 # terminal 1: live meter, through the server
  pw power cycle --off 1s        # terminal 2: terminal 1 sees the cycle as it happens
  xdg-open http://10.83.17.1     # the browser app over NCM, at the same time
  pw clients
  #  usb   ww serve: pw power watch (pid 4121), notebook (pid 4380)
  #  ncm   browser 10.83.17.2 · observing meter, plug
  #  holds uart[0]: notebook (pid 4380) since 12:04:11
  ```
], python: [
  ```python
  import portwright
  pw = portwright.open()                    # finds the local server, or opens USB directly when there is none
  with pw.uart.open(baud=115200) as con: # takes the UART for this client; others can still listen
      con.write(b"reboot\r\n")
  ```
])

== What you should see

Every client sees every change: power switched from the terminal shows up in the browser at once, because state is
pushed to all observers, not polled. Functions that only one client can drive (the UART's TX, an SWD session) are
taken by one client at a time, and the others get "held by notebook (pid 4380)" instead of a silent fight.

#tip[Commands that change state carry who sent them, so `pw device log` and the app's history say "plug VBUS off by
  pw power cycle (pid 4122)", not just "off".]

== What next

- Lab "Test farm": many clients, many dongles, from other computers too.
