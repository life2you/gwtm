[English](README.md) | [简体中文](README.zh-CN.md)

# gwtm

`gwtm` is a Rust-based Git worktree manager for local multi-project workflows.

## What It Does

- Scans a configured projects root and finds Git repositories in its direct subdirectories
- Creates, opens, lists, and removes Git worktrees
- Provides a fullscreen TUI for first-run setup and later reconfiguration
- Supports searchable project, branch, and worktree pickers
- Detects available IDE launchers and lets you choose which one should open worktrees

## Project Layout

- `src/main.rs`: application entrypoint and workflow logic
- `src/tui.rs`: fullscreen TUI components and interaction state
- `Cargo.toml`: Rust package manifest
- `gwtm`: thin launcher that runs the best available local binary or falls back to `cargo run`
- `RELEASING.md`: maintainer release SOP

## Requirements

- Rust toolchain
- Git
- Optional on macOS: `osascript` for the system folder picker
- Optional: `rustrover` command in `PATH`

## Run

Development mode:

```bash
./gwtm
```

Or directly:

```bash
cargo run
```

Release build:

```bash
cargo build --release
./target/release/gwtm
```

## Homebrew

This repository is prepared for publishing through a personal Homebrew tap.

Files related to Homebrew publishing:

- `packaging/homebrew-tap/Formula/gwtm.rb`
- `packaging/homebrew-tap/README.md`
- `RELEASING.md`
- `scripts/update-homebrew-formula.sh`

Recommended release flow:

```bash
git tag -a v<version> -m "v<version>"
git push origin main
git push origin v<version>
./scripts/update-homebrew-formula.sh <version>
```

Then copy the generated formula into:

```text
life2you/homebrew-tap
```

After the tap repository is published, users can install with:

```bash
brew install life2you/tap/gwtm
```

## First Launch

On first launch, `gwtm` asks for:

1. `projects_root_dir`: a folder whose direct children are Git repositories
2. `worktrees_root_dir`: where created worktrees should be stored
3. the IDE or launcher that should open worktrees

The configuration flow runs in the same fullscreen TUI used by the main menu. On macOS, press `f` on a path field to open the system folder picker. After the paths are set, `gwtm` detects available IDE launchers and installed apps so you can choose one explicitly.

Config is saved to:

```text
~/.config/gwtm/config.toml
```

Example:

```toml
projects_root_dir = "/Users/you/code"
worktrees_root_dir = "/Users/you/worktrees"
ide_mode = "rust"
ide_command = "IntelliJ IDEA"
ide_label = "IntelliJ IDEA"
```

## Main Menu

- `Create Worktree`
- `Open Worktree`
- `List Worktrees`
- `Remove Worktree`
- `Reconfigure`
- `Quit`

## Notes

- Worktrees are created under:

```text
<worktrees_root_dir>/<project_name>/<branch_name>
```

- Branch names are mapped to directory names by replacing `/` with `-`
- The default workflow is Rust-oriented, but the tool itself works for any Git repository

## Release Docs

- English: [`RELEASING.md`](RELEASING.md)
- 简体中文: [`RELEASING.zh-CN.md`](RELEASING.zh-CN.md)

## License

MIT
