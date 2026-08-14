---
icon: lucide/info
---

# About

## What is Donguru?

Donguru (**ドングル**) is a USB-Type-C dongle that turns a single Type-C port into
a small, controllable lab bench for **hardware debugging, board bring-up and
testing**.

It provides a **USB hub** that controls the downstream USB port(s), together
with **USART / I2C / GPIO / ADC** interfaces to probe and drive a target. It
also includes a **power switch** for downstream power and a **power meter** to
monitor the power consumption of devices attached to the downstream USB port(s).

Because every function can be driven from the command line, Donguru is equally at
home manually poking around and running one-shot test or debug sessions as it is
with **automated testing, scripting and validation**.

For the full feature list and where to go next, see the
[home page](../index.md).

## What it's good for

- **Terminal** — access your SoC or MCU shell through UART.
- **Debugging** — poke at UART/I2C/GPIO/ADC and read the power meter to see what a
  board is actually doing.
- **Testing** — exercise interfaces and power in a controlled, observable way.
- **Automated testing & scripting** — wrap the `donguru` CLI or Python/Rust native interface in scripts and CI
  to power-cycle, stimulate and measure hardware reproducibly.

## Why it exists

Bringing up, debugging and testing new hardware can be tricky, and it often
means juggling several separate tools, such as a hub, a logic or serial adapter,
a bench supply and a meter. Each of those varies in quality and in how well it
supports things like pipeable CLI output and integration into CI.

As the creator of this project has felt this pain first-hand on many occasions,
he decided to finally address it once and for all: good hardware, good firmware
and a well-designed, Unix-style CLI that is flexible and integrates nicely
wherever it is needed.

## Project status

!!! warning "Pre-alpha"

    Donguru is at a very early stage. The project is still being shaped: the
    hardware, firmware and CLI are being designed and not yet built or
    released. Everything here describes intended direction and is subject to
    change.

## License

License terms are not finalised yet *(TBD)*, though it will be open source.
