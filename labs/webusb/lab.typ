#import "../lib.typ": *

#show: lab.with(
  title: [Drive Portwright from a web page],
  time: "5 min",
  needs: ([Portwright], [Chrome, Edge or another Chromium browser]),
  features: ("API-22", "GUI-1"),
)

== Goal

Control Portwright from a browser tab: no install, no driver, works on a locked-down office laptop and on ChromeOS.

== Steps

1. Plug Portwright in. Chrome shows a notification: "Go to portwright.vhrd.tech to connect". Click it.
2. Click "Connect" and pick Portwright in the browser's device list.
3. The app opens: power switch, meter plot, UART terminal.

#tip[The page holds Portwright's USB control interface while it is open. `pw` keeps working next to it: through the
  local WireWeaver server (which shares the USB interface between programs) or over the network interface (NCM).
  See the lab "Many clients at once".]

== What you should see

The same app as `pw ui`, in the tab. Close the tab and `pw` works again.

== What next

- Lab "Portwright on the network": the way in for Firefox and Safari.
