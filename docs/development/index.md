---
icon: lucide/wrench
---

# Development

This page describes how to set up a local development environment.

## Prerequisites

* [uv](https://docs.astral.sh/uv/)
* [rustup](https://rustup.rs)
* [just](https://github.com/casey/just)
* [prek](https://github.com/j178/prek)

## Git hooks

Code quality checks run before commit(s) and/or pushes using [prek](https://github.com/j178/prek).
For details see `.pre-commit-config.yaml`.

Set the hooks up once after cloning:

```bash
just hooks-install
```

## Just Tasks
Generally you find automations or entry points for automated tasks in the `justfile`.
This should and shall be the single source of truth for triggering tasks and actions.

In order to get a list of all available taks just run:

`just` or `just --list`

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
