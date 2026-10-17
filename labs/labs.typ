// The labs in book order, grouped by level. A lab's number is its place here, so adding or moving one renumbers the
// rest; the folder name (the slug) stays the same and is what links and `just lab SLUG` use.
#let parts = (
  (level: "first-hour", labs: (
    "hello",
    "phone-charging",
    "who-feeds-whom",
    "kill-switch",
    "usb-unplug",
    "charger-inspector",
    "button-shortcut",
  )),
  (level: "bench", labs: (
    "overnight-log",
    "standby-current",
    "inrush",
    "cable-check",
    "safe-first-power-on",
    "relay-and-header-power",
    "serial-console",
    "i2c-on-the-plug",
    "header-gpio-adc",
    "update-donguru",
  )),
)
