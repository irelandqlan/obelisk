# Development Guide

This document outlines the development workflow, branch model, build tools, and release procedures for **Obelisk**. It is intended for both human contributors and AI coding assistants.

---

## 1. Quick Start

### Prerequisites
- **Rust Toolchain**: Stable (via [rustup](https://rustup.rs/))
- **Command Runner**: [just](https://github.com/casey/just) (`cargo install just`)
- **System Libraries**: GTK4 (`libgtk-4-dev` / `gtk4-devel`), Libadwaita (`libadwaita-1-dev` / `libadwaita-devel`), OpenSSL, `pkg-config`, `tar`, `unzip`
- **Flatpak Tools** *(optional, for Flatpak testing)*: `flatpak`, `flatpak-builder`

### Everyday Commands
Run `just` with no arguments to list all available recipes:

```bash
just              # List all available commands
just run          # Run Obelisk locally with cargo
just demo         # Run Obelisk in demo / screenshot mode (mock data)
just test         # Run the full unit test suite
just check        # Fast syntax & type check
just build-release# Build release binary in target/release/obelisk
```

---

## 2. Branching Model

Obelisk uses a streamlined two-branch workflow:

```
[develop]  ──(daily coding & features)───────┬──> (continue dev)
                                             │  (just merge-to-master)
[master]   ──────────────────────────────────┴───[v0.6.2 Tag] (Release)
```

- **`develop`** *(Default working branch)*:
  - All day-to-day coding, experimentation, and bug fixes happen here.
  - You can push small commits freely without triggering noisy CI runs.
- **`master`** *(Production / Release branch)*:
  - Represents stable, release-ready code.
  - All changes enter `master` through `just merge-to-master` (which verifies tests locally first).
  - Merges to `master` and pull requests trigger cached CI test runs on GitHub Actions.
  - Pushing a version tag (`v*`) triggers the containerized Flatpak release builder.

---

## 3. Local Flatpak Development

Obelisk is packaged with App ID `io.github.irelandqlan.Obelisk` against the GNOME 50 runtime.

```bash
# Build and install Flatpak into user environment
just flatpak-build

# Run the installed Flatpak
just flatpak-run

# Create a standalone offline .flatpak bundle for distribution
just flatpak-bundle
```

> **Note on Configuration Sharing**: Obelisk detects if it is running inside Flatpak with `--filesystem=host` and prioritizes your host `~/.config/obelisk/` and `~/.local/share/obelisk/` directories. This ensures that the native `cargo run` and Flatpak versions seamlessly share the same accounts, instance settings, and game assets.

---

## 4. Merging `develop` into `master`

To merge tested changes from `develop` into `master` safely without manual mistakes:

```bash
just merge-to-master
```

### What `just merge-to-master` does automatically:
1. **Verifies working tree**: Aborts if you have uncommitted or unstaged changes to prevent accidental loss.
2. **Runs test suite**: Executes `cargo test` and stops if any test fails.
3. **Pulls latest master**: Fetches and rebases with `origin/master`.
4. **Merges develop**: Performs the merge cleanly with a descriptive message.
5. **Pushes to GitHub**: Pushes the updated `master` branch.
6. **Switches back**: Returns your working branch to `develop`.

---

## 5. Automated Releases

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

---

## 6. Guidelines for Contributors & AI Agents

1. **Namespace & App ID**:
   - App ID is `io.github.irelandqlan.Obelisk` (CamelCase/PascalCase at the end).
   - Desktop file: `flatpak/io.github.irelandqlan.Obelisk.desktop`
   - Metainfo file: `flatpak/io.github.irelandqlan.Obelisk.metainfo.xml`
   - Flatpak manifest: `flatpak/io.github.irelandqlan.Obelisk.yaml`
   - App icon: `data/io.github.irelandqlan.Obelisk.svg`
   - GResource prefix: `/io/github/irelandqlan/Obelisk/icons/scalable/actions`
2. **Name Reference**:
   - The application is named **Obelisk** (not "Obelisk Launcher").
3. **Paths & Backwards Compatibility**:
   - Host config: `~/.config/obelisk/config.json` (with fallback to `obelisk-launcher`)
   - Host data: `~/.local/share/obelisk/` (with fallback to `obelisk-launcher`)
   - Host cache: `~/.cache/obelisk/`
   - Keyring service name: `obelisk` (with fallback to `obelisk-launcher`)
4. **Pre-commit Verification**:
   - Always run `just test` before creating commits or pull requests.
