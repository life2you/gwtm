# gwtm

`gwtm` is a Rust implementation of a Git worktree manager for local multi-project workflows.

## What It Does

- Scans a configured projects root and finds Git repositories in its direct subdirectories
- Creates, lists, and removes Git worktrees
- Runs a first-launch setup wizard
- Lets you manually choose the projects root folder on first launch
- Uses `RustRover` as the default IDE command after creating a worktree

## Project Layout

- `src/main.rs`: main Rust implementation
- `Cargo.toml`: Rust package manifest
- `gwtm`: thin launcher that runs the compiled binary or falls back to `cargo run`

## Requirements

- Rust toolchain
- Git
- Optional on macOS: `osascript` for system folder picker
- Optional: `rustrover` command in PATH

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
- `scripts/update-homebrew-formula.sh`

Recommended release flow:

```bash
git tag v0.1.1
git push origin v0.1.1
./scripts/update-homebrew-formula.sh
```

Then copy `packaging/homebrew-tap/*` into the repository:

```text
life2you/homebrew-tap
```

After the tap repository is published, users can install with:

```bash
brew install life2you/tap/gwtm
```

## First Launch

On first launch, `gwtm` will ask for:

1. `projects_root_dir`: a folder whose direct children are Git repositories
2. `worktrees_root_dir`: where created worktrees should be stored

On macOS it will try to open a folder picker first. If that is unavailable or canceled, it falls back to terminal input.

Config is saved to:

```bash
~/.config/gwtm/config.toml
```

Example:

```toml
projects_root_dir = "/Users/you/code"
worktrees_root_dir = "/Users/you/worktrees"
ide_mode = "rust"
ide_command = "rustrover"
ide_label = "RustRover"
```

## Menu

- `创建 Worktree`
- `列出 Worktree`
- `删除 Worktree`
- `重新配置`
- `退出程序`

## Notes

- Worktrees are created under:

```text
<worktrees_root_dir>/<project_name>/<branch_name>
```

- Branch names are mapped to directory names by replacing `/` with `-`
- The default workflow is Rust-oriented, but the tool itself works for any Git repo

## License

MIT
