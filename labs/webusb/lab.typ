#import "../lib.typ": *

#show: lab.with(
  title: [Drive Donguru from a web page],
  time: "5 min",
  needs: ([Donguru], [Chrome, Edge or another Chromium browser]),
  features: ("API-22", "GUI-1"),
)

== Goal

Control Donguru from a browser tab: no install, no driver, works on a locked-down office laptop and on ChromeOS.

== Steps

1. Plug Donguru in. Chrome shows a notification: "Go to donguru.vhrd.tech to connect". Click it.
2. Click "Connect" and pick Donguru in the browser's device list.
3. The app opens: power switch, meter plot, UART terminal.

#tip[Only one program can hold Donguru's control interface: while the page has it, `dg` can't, and the page says so.
  Its network and SWD interfaces stay usable.]

== What you should see

The same app as `dg ui`, in the tab. Close the tab and `dg` works again.

== What next

- Lab "Donguru on the network": the way in for Firefox and Safari.
