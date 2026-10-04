<div align="center">
  <img src="../data/io.github.irelandqlan.Obelisk.svg" width="96" height="96" alt="Obelisk Icon">

  # Development Guide

  <p>
    <a href="../README.md">Back to README</a> &bull;
    <a href="INSTALL.md">Install Guide</a> &bull;
    <a href="SCREENSHOTS.md">Screenshots</a> &bull;
    <a href="https://github.com/irelandqlan/obelisk/issues">Issues</a>
  </p>
</div>

## Basics

### Prerequisites
- **Rust Toolchain**: Stable (via [rustup](https://rustup.rs/))
- **Command Runner**: [just](https://github.com/casey/just) (`cargo install just`)
- **System Libraries**: GTK4 (`libgtk-4-dev` / `gtk4-devel`), Libadwaita (`libadwaita-1-dev` / `libadwaita-devel`), OpenSSL, `pkg-config`, `tar`, `unzip`
- **Flatpak Tools** *(optional, for Flatpak testing)*: `flatpak`, `flatpak-builder`

### Everyday Commands
Run `just` with no arguments to list all available recipes:

```bash
just                 # List all available commands
just run             # Run Obelisk locally with cargo
just demo            # Run Obelisk in demo / screenshot mode (mock data)
just test            # Run the full unit test suite
just check           # Fast syntax & type check
just build-release   # Build release binary in target/release/obelisk

just merge-to-master # Merge development branch to master
# Verifies, tests, pulls/rebases, merges, pushes to master, and then switches back to develop
```

## Local Flatpak Development

Obelisk is packaged with App ID `io.github.irelandqlan.Obelisk` against the GNOME 50 runtime.

```bash
# Build and install Flatpak into user environment
just flatpak-build

# Run the installed Flatpak
just flatpak-run

# Create a standalone offline .flatpak bundle for distribution
just flatpak-bundle
```

> **Note on configuration**: Obelisk detects if it is running inside Flatpak with `--filesystem=host` and prioritizes your host `~/.config/obelisk/` and `~/.local/share/obelisk/` directories. This means that the native `cargo run` and Flatpak version share the same accounts, instance settings, and game assets.

## Automated Releases

Releasing a new version is automated with a single command:

```bash
just release 0.6.2
```

### What `just release <version>` does:
1. Verifies you have a clean working tree.
2. If run from `develop`, automatically runs `merge-to-master` first.
3. Runs the full test suite (`cargo test`).
4. Bumps the version in `Cargo.toml` and updates `Cargo.lock`.
5. Adds a new release entry with today's date in `flatpak/io.github.irelandqlan.Obelisk.metainfo.xml`.
6. Commits: `chore: bump version to <version>`.
7. Creates annotated git tag: `v<version>`.
8. Pushes `master` and the tag to GitHub.
9. Fast-forwards `develop` with the release commit so both branches stay in sync.
10. GitHub Actions detects the `v*` tag push and automatically builds `io.github.irelandqlan.Obelisk.flatpak` and attaches it to the GitHub Release.
