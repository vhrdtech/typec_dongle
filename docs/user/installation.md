---
icon: lucide/download
---

# Installing

!!! info "Status: CI artifacts, not yet published"

    `.deb` and `.rpm` packages are built on every CI run as downloadable artifacts of the
    `package` job (PKG-3). There is no apt/dnf repository and no GitHub Release upload yet, so
    for now you fetch the package from a CI run's artifact list rather than a package manager.
    See the [packaging and distribution plan](../design/packaging/index.md) for where this is
    headed (Homebrew, Chocolatey, standalone binaries).

## Linux: `.deb` and `.rpm`

Both packages install the same things:

- `donguru`, built in release mode, at `/usr/bin/donguru`.
- `dg`, a symlink to `donguru` at `/usr/bin/dg`, created by the package's post-install step (not
  baked into the package itself, so both names always point at the same binary after an upgrade).
- Man pages for every command and subcommand (`man donguru`, `man donguru-device-list`, ...),
  generated from the CLI's own `clap` tree so they can never drift from `--help`. `man dg` is a
  short page that points to `donguru(1)`.
- Shell completions for bash, zsh and fish, for both `donguru` and `dg`, installed into each
  shell's standard system completion directory.
- A udev rule (`/usr/lib/udev/rules.d/60-donguru.rules`) that tags the dongle's USB device
  (`idVendor=c0de`, `idProduct=cafe`) with `uaccess`, so your regular user can open it without
  `sudo` or a manual udev rule once you unplug and replug the dongle after installing.

Install with your distribution's own tool, after downloading the package from a CI run:

```sh
sudo apt install ./donguru-cli_*.deb      # Debian, Ubuntu
sudo dnf install ./donguru-cli-*.rpm      # Fedora, RHEL
```

Uninstalling removes the `dg` symlink and reloads udev rules through the same post-install /
post-removal scripts.

## From source

Without a package, `just install` in a checkout of this repository builds `donguru` and `dg` in
deploy mode and installs the shell completions for your user (see
[Development](../development/index.md)); it does not install man pages or the udev rule.

## License

The license is still **TBD** (PKG-4): both packages carry that as their license metadata
(`License: TBD` in the `.deb`'s control file and in the `.rpm` header), taken from the
workspace's own `license = "TBD"` rather than written separately for packaging. This will be
replaced before the v1.0 CrowdSupply launch.
