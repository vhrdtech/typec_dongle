---
icon: lucide/wrench
---

# Development

This page describes how to set up a local development environment.

## Prerequisites

Install the following tools before you start:

- [uv](https://docs.astral.sh/uv/) — Python package and project manager
    - [Python](https://www.python.org)
- [Rust](https://rust-lang.org)

## Working on the documentation

The documentation site is built with [Zensical](../reference/zensical.md).
Common tasks are exposed through the `justfile`:

```bash
# Build the static site into ./site
just build-docs

# Serve the docs locally with live reload
just serve-docs
```

See the [Reference](../reference/zensical.md) section for details on
authoring content.
