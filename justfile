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

# Format check, lint and test
[group: 'rust']
verify: format-check clippy test

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
