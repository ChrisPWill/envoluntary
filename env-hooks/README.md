# env-hooks

Shell integration library for building direnv-like utilities. Provides direnv
core logic for shell-agnostic management of the state of environment variable
exporting and unsetting across different shell types (bash, zsh, fish).

## Features

- **Multi-shell support**: Works with bash, zsh, and fish shells
- **Environment state management**: Manages the export and unset state of
  environment variables, essential for direnv-like functionality
- **JSON output**: Can export environment variables in JSON format for
  programmatic access
- **Environment hooks**: Integration hooks for seamless environment loading

## Part of envoluntary

This library is a core component of [envoluntary](https://github.com/dfrankland/envoluntary),
an automatic Nix development environment management tool for your shell.

For more information about the broader project, see the [main README](https://github.com/dfrankland/envoluntary/blob/main/README.md).
