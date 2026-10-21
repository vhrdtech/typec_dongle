alias ci := verify
alias cheat := cheatsheet
alias fmt := format
alias fmt-check := format-check

# List all available targets/actions
default:
    @just --list

# Open the just cheet sheet in the browser
[group: 'just']
cheatsheet:
    uv run python -m webbrowser -t https://cheatography.com/linux-china/cheat-sheets/justfile/

# Build the documentation
[group: 'docs']
build-docs:
    uv run --with zensical zensical build --clean

# Build and Serve the documentation
[group: 'docs']
serve-docs:
    uv run --with zensical zensical serve

# Typst flags of the lab book: the vhrd package's fonts and the build stamp on every page
labs-fonts := env("HOME") / ".local/share/typst/packages/local/vhrd/0.9.2/fonts"
labs-flags := "--root . --font-path " + labs-fonts + " --input git=" + `git describe --always --dirty --exclude '*'` + " --input 'built=" + `date +'%-d %b %Y %H:%M'` + "'"

# Lab book for the website: HTML pages, the book and every lab as PDF, into site/labs
[group: 'docs']
build-labs:
    typst compile {{ labs-flags }} --features html,bundle -f bundle labs/site.typ site/labs

# The whole lab book as one PDF, into target/labs
[group: 'docs']
labs-book:
    mkdir -p target/labs
    typst compile {{ labs-flags }} labs/book.typ target/labs/donguru-lab-book.pdf

# One lab as PDF, rebuilt on every save while writing it, e.g. `just lab phone-charging`
[group: 'docs']
lab SLUG:
    mkdir -p target/labs
    typst watch {{ labs-flags }} --input lab={{ SLUG }} labs/preview.typ target/labs/{{ SLUG }}.pdf

# Rust edition, used when formatting individual files. `cargo fmt` reads
# this from Cargo.toml, but a bare `rustfmt` invocation does not and
# would otherwise fall back to edition 2015.
edition := "2024"

# Run rustfmt either on the workspace or specific files
[group: 'rust']
[positional-arguments]
format *FILES:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ $# -eq 0 ]; then
        cargo fmt --all
    else
        rustfmt --edition {{ edition }} "$@"
    fi

# Check formatting without writing changes (workspace or individual FILES)
[group: 'rust']
[positional-arguments]
format-check *FILES:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ $# -eq 0 ]; then
        cargo fmt --all --check
    else
        rustfmt --edition {{ edition }} --check "$@"
    fi

# Lint (warnings are errors)
[group: 'rust']
clippy:
    cargo clippy --workspace --all-targets --all-features --locked -- -D warnings

# Run check on the workspace
[group: 'rust']
check:
    cargo check --workspace --all-targets --all-features --locked

# Run all tests
[group: 'rust']
test:
    cargo test --workspace --all-features --locked

# Format check and clippy, the static part of verify
[group: 'rust']
lint: format-check clippy

# Format check, lint and test
[group: 'rust']
verify: lint test

# Install the git hooks (pre-commit and pre-push)
[group: 'hooks']
hooks-install:
    prek install --install-hooks

# Remove the installed git hooks
[group: 'hooks']
hooks-uninstall:
    prek uninstall

# Run default hooks against every file in the repository
[group: 'hooks']
hooks:
    prek run --all-files

# Run all hooks, including the slower pre-push ones
[group: 'hooks']
hooks-all:
    prek run --all-files --stage pre-push

# Update hook repositories to their latest revisions
[group: 'hooks']
hooks-update:
    prek update

# The WireWeaver CLI, with `ww stm32dfu` (wire_weaver master since 21 Oct 2026); `just ww=PATH update ...` for another
ww := "ww"

# Update the firmware over USB without a probe (FW-12): build, reboot into the STM32 ROM bootloader, write, back.
# SERIAL is the full USB serial (`ww list`), the same in the firmware and in the ROM.
[group: 'fw']
[working-directory('fw/donguru')]
update SERIAL:
    cargo build --release
    {{ ww }} dfu -s {{ SERIAL }} reboot
    {{ ww }} stm32dfu -s {{ SERIAL }} flash target/thumbv6m-none-eabi/release/donguru --wait 10 --follow

# Build firmware in release, print FLASH/RAM usage (limits from memory.x) and top 25 symbols
[group: 'fw']
[working-directory('fw/donguru')]
fw-size:
    #!/usr/bin/env bash
    set -euo pipefail
    # size report only: hide rustc warnings (does not invalidate the build cache)
    export CARGO_BUILD_WARNINGS=allow
    cargo build --release
    elf=target/thumbv6m-none-eabi/release/donguru
    uv run --quiet scripts/size.py "$elf" memory.x
    if ! cargo bloat --version >/dev/null 2>&1; then
        echo "cargo-bloat not found: cargo install cargo-bloat" >&2
        exit 1
    fi
    cargo bloat --release --bin donguru -n 25
    if ! cnt --help >/dev/null 2>&1; then
        echo "cnt not found: cargo install cnt_cli" >&2
        exit 1
    fi
    printf "\nCounters:\n"
    cnt list "$elf"
    cnt elf add "$elf"

# Fast-forward the sibling checkouts this repo builds against by path (wire_weaver, embedded_bedrock)
[group: 'rust']
deps-pull:
    git -C ../wire_weaver pull --ff-only
    git -C ../embedded_bedrock pull --ff-only
