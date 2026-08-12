[group: 'docs']
build-docs:
    uv run --with zensical zensical build --clean

[group: 'docs']
serve-docs:
    uv run --with zensical zensical serve
