---
icon: lucide/home
hide:
  - navigation
  - toc
---

# Donguru — ドングル { style="text-align:center" }

A USB-Type-C dongle with a **USB Hub**, **UART / I2C / GPIO / ADC**, a
**power switch** and a **power meter**
{ style="text-align:center" }

![The Donguru USB-Type-C dongle](assets/images/donguru.png){ width="420" style="display:block;margin:0 auto" }

Plus a fast, scriptable command-line interface to drive it all.
{ style="text-align:center" }

<div style="text-align:center" markdown>
[Get started](user/quickstart.md){ .md-button .md-button--primary }
[Explore the CLI](user/usage.md){ .md-button }
</div>

---

## What it does

<div class="grid cards" markdown>

-   :material-usb: __USB Hub__

    ---

    Not enough ports on your machine? Donguru puts a small test rack worth of equipment on just one cable.

-   :material-usb: __Full control of Type-C plug__

    ---

    Turn power on and off, disconnect data lines, communicate over USB-PD protocol, enter alternate or debug-accessory modes and more. 

-   :material-connection: __UART / I2C / GPIO / ADC__

    ---

    General-purpose interfaces for prototyping, bring-up, debugging and testing. On the plug SBU lines and 2.54 header.

-   :material-power-plug: __Power switch__

    ---

    Enable or disable power to downstream device(s) on demand.

-   :material-gauge: __Power meter__

    ---

    Monitor voltage and current draw in real time.

-   :material-creation: __More to come__

    ---

    Hardware also supports SPI, FDCAN, PWM and a whole SWD interface. Which will be released with upcoming software updates.

</div>

## Take the tour

<div class="grid cards" markdown>

-   :lucide-book-open-check: __Documentation__

    ---

    Quickstart and everyday usage of the `donguru` (`dg`) CLI from an end-user
    point of view.

    [:octicons-arrow-right-24: Start here](user/quickstart.md)

-   :lucide-terminal: __CLI__

    ---

    Understand the `donguru` (`dg`) command-line interface: design, command tree and
    it's unixi archtitecture.

    [:octicons-arrow-right-24: Read the design](design/cli/index.md)

-   :material-language-python: __Scripting__

    ---

    Take full control of the hardware through Rust or Python.

    [:octicons-arrow-right-24: Learn more](user/scripting.md)

-   :lucide-wrench: __Development__

    ---

    Set up your environment and build everything yourself.

    [:octicons-arrow-right-24: Start building](development/index.md)

-   :lucide-drafting-compass: __Design__

    ---

    Design documents, the library evaluations etc. to understand our decisions and desings.

    [:octicons-arrow-right-24: Browse designs](design/index.md)

-   :lucide-info: __About__

    ---

    What Donguru is, its feature set, and where to go next.

    [:octicons-arrow-right-24: Learn more](about/index.md)

</div>
