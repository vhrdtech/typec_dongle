---
icon: lucide/package
---

# Packaging and distribution plan

This document outlines how we intend to ship Donguru. It is not an installation guide.

!!! info "Status: Draft"

    Package formats and distribution details are subject to change.

## Goals and scope

Ship the `donguru` CLI with its supporting files, reliable upgrades and a
clean uninstall, in this order:

1. **Linux:** `.rpm` and `.deb`
2. **macOS:** a Homebrew formula
3. **Windows:** a Chocolatey package

Start with x86-64 binaries for each OS and publish standalone binaries with
each release as a fallback for users without a package. DMG, MSI and other Linux formats can follow if needed.

!!! info

    A standalone binary will not set up shell completions, udev access or other system integration automatically.

!!! tip

    In the long run, using a tool like [nFPM](https://nfpm.goreleaser.com/) would make sense to provide a broader range of Linux packages.

## What packages contain

| Asset | Intended treatment |
| --- | --- |
| CLI | `donguru`, with `dg` installed as a symlink or Windows shim to the same binary. |
| Completions and man pages | Generated from the released CLI and installed in the appropriate locations for each package manager. |
| Linux udev rules | A package-owned rule in the distro's vendor rules directory; we will also ship a separate *inactive* group-rule example for headless/SSH hosts. |
| Config | A documented `donguru.toml.example` only. Never install an active system config or overwrite a user's config. |


## References

- CLI asset generation: [clap_complete](https://docs.rs/clap_complete/latest/clap_complete/), [clap_mangen](https://docs.rs/clap_mangen/latest/clap_mangen/).
- Linux packaging: [RPM documentation](https://rpm.org/docs/), [Debian Policy](https://www.debian.org/doc/debian-policy/), [nFPM](https://nfpm.goreleaser.com/), [udev rules](https://www.freedesktop.org/software/systemd/man/latest/udev.html).
- macOS: [Homebrew Formula Cookbook](https://docs.brew.sh/Formula-Cookbook) and [bottles](https://docs.brew.sh/Bottles).
- Windows: [Chocolatey package creation](https://docs.chocolatey.org/en-us/create/create-packages/).
