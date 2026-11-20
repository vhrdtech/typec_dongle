#import "../lib.typ": *

#show: lab.with(
  title: [Update Portwright itself],
  time: "5 min",
  needs: ([Portwright], [internet for the download]),
  features: ("FW-12", "CLI-10"),
)

== Goal

Keep Portwright's firmware current, and know how to bring it back if an update goes wrong.

== Steps

#steps(cli: [
  ```sh
  pw fw version                 # running 0.21.0, latest 0.23.1 (released 2 Dec 2026)
  pw fw update                  # download, check, flash through the ROM bootloader, reconnect
  pw fw update ./portwright-0.24.0-rc1.elf   # a local build
  pw fw update --all            # every Portwright on this computer, one after another
  ```
])

== What you should see

The update takes about 15 seconds with a progress bar. Portwright drops off USB, comes back as an STM32 bootloader,
then as Portwright with the new version. The label and settings stay.

#tip[If an update is cut off, hold the button while plugging Portwright in: it starts the ROM bootloader, and
  `pw fw update` finds it there.]

== What next

- Lab "Write your own WireWeaver method": build a firmware of your own and flash it the same way.
