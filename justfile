# just cheatsheet https://cheatography.com/linux-china/cheat-sheets/justfile/

# List all available targets
default:
    @just --list

[group: 'docs']
build-docs:
    uv run --with zensical zensical build --clean

[group: 'docs']
serve-docs:
    uv run --with zensical zensical serve
