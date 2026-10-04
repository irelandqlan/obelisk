<div align="center">
  <img src="data/io.github.irelandqlan.Obelisk.svg" width="96" height="96" alt="Obelisk Icon">

  # Installation Guide

  Instructions for building and running Obelisk either through Flatpak or natively with Cargo.

  <p>
    <a href="README.md">Back to README</a> &bull;
    <a href="docs/SCREENSHOTS.md">Screenshots</a> &bull;
    <a href="https://github.com/irelandqlan/obelisk/issues">Issues</a>
  </p>
</div>

## Contents
- [Native Build (Cargo)](#native-build-cargo)
  - [System Dependencies](#system-dependencies)
  - [Build and Run](#build-and-run)
- [Flatpak Build](#flatpak-build)
  - [Prerequisites](#flatpak-prerequisites)
  - [Runtimes](#installing-gnome-sdk-and-platform-runtimes)
  - [Building and Installing](#building-and-installing-the-flatpak)
  - [Running](#running-the-flatpak)
  - [Creating an Offline Bundle](#creating-an-offline-bundle)
- [Runtime Requirements](#runtime-requirements)
- [Troubleshooting](#troubleshooting)

## Native Build (Cargo)

### System Dependencies

Building natively requires development headers for GTK4, Libadwaita, OpenSSL, and standard build tools.

#### Debian / Ubuntu / Linux Mint
```bash
sudo apt update
sudo apt install build-essential pkg-config libssl-dev libgtk-4-dev libadwaita-1-dev unzip tar
```

#### Fedora / RHEL
```bash
sudo dnf groupinstall "Development Tools"
sudo dnf install pkgconf-pkg-config openssl-devel gtk4-devel libadwaita-devel unzip tar
```

#### Arch Linux / Manjaro
```bash
sudo pacman -Syu base-devel pkgconf openssl gtk4 libadwaita unzip tar
```

### Build and Run

With a current stable Rust toolchain (via [rustup](https://rustup.rs/)):

```bash
git clone https://github.com/irelandqlan/obelisk.git
cd obelisk

# Run in debug mode
cargo run

# Or build release binary
cargo build --release
./target/release/obelisk
```

## Flatpak Build

Flatpak isolates the launcher and bundles the required GNOME 50 runtime libraries.

### Flatpak Prerequisites

Install `flatpak` and `flatpak-builder` if they are not already installed:

- **Ubuntu / Debian**: `sudo apt install flatpak flatpak-builder`
- **Fedora**: `sudo dnf install flatpak flatpak-builder`
- **Arch Linux**: `sudo pacman -S flatpak flatpak-builder`

Ensure the Flathub remote is configured:
```bash
flatpak remote-add --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
```

### Installing GNOME SDK and Platform Runtimes

Obelisk targets the GNOME 50 platform runtime:

```bash
flatpak install flathub org.gnome.Platform//50 org.gnome.Sdk//50 org.freedesktop.Sdk.Extension.rust-stable//25.08
```

### Building and Installing the Flatpak

Build and install directly into your user environment:

```bash
flatpak-builder --user --install --force-clean build-dir flatpak/io.github.irelandqlan.Obelisk.yaml
```

Once installed, Obelisk will show up in your desktop environment's app launcher.

### Running the Flatpak

```bash
flatpak run io.github.irelandqlan.Obelisk
```

Or run directly out of the build directory without installing to the system:
```bash
flatpak-builder --run build-dir flatpak/io.github.irelandqlan.Obelisk.yaml obelisk
```

### Creating an Offline Bundle

To package a standalone `.flatpak` file for distribution:

```bash
flatpak-builder --bundle build-dir flatpak/io.github.irelandqlan.Obelisk.yaml io.github.irelandqlan.Obelisk.flatpak
```

Install it with:
```bash
flatpak install io.github.irelandqlan.Obelisk.flatpak
```

## Runtime Requirements

- **tar** and **unzip**: Needed to unpack downloaded Java runtimes and Minecraft game files.
- **Java**: Java 8 for legacy Minecraft versions, Java 17 for 1.18 to 1.20.4, Java 21+ for modern releases. The built-in Java installer in Settings can download and manage these.

## Troubleshooting

- **Flatpak filesystem access**: If you save instances on another drive or non-standard path, grant the Flatpak permission using [Flatseal](https://flathub.org/apps/com.github.tchx84.Flatseal) or override via CLI:
  ```bash
  flatpak override --user --filesystem=/path/to/instances io.github.irelandqlan.Obelisk
  ```
