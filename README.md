# [![Balatro Mod Manager](images/title.svg)](#)

The Balatro Mod Manager by _Skyline_.

Balatro Mod Manager is a standalone tool made for [Balatro](https://store.steampowered.com/app/2379780/Balatro/) that makes finding, downloading, and installing mods easy.

<p align="center">
    <a href="https://star-history.dera.page/#skyline69/balatro-mod-manager&type=Date">
        <picture>
            <source media="(prefers-color-scheme: dark)" srcset="https://star-history.dera.page/svg?repos=skyline69/balatro-mod-manager&type=Date&theme=dark" />
            <source media="(prefers-color-scheme: light)" srcset="https://star-history.dera.page/svg?repos=skyline69/balatro-mod-manager&type=Date" />
            <img width="75%" alt="Star History Chart" src="https://star-history.dera.page/svg?repos=skyline69/balatro-mod-manager&type=Date" />
        </picture>
    </a>
</p>

![GitHub Downloads (all assets, all releases)](https://img.shields.io/github/downloads/skyline69/balatro-mod-manager/total)
![GitHub License](https://img.shields.io/github/license/skyline69/balatro-mod-manager)
![GitHub Tag](https://img.shields.io/github/v/tag/skyline69/balatro-mod-manager)
![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/skyline69/balatro-mod-manager/ci.yml)
![Website](https://img.shields.io/website?url=https%3A%2F%2Fbalatro-mod-manager.dasguney.com%2F)

![Rust](https://img.shields.io/badge/rust-%23000000.svg?logo=rust&logoColor=white)
![TypeScript](https://img.shields.io/badge/typescript-%23007ACC.svg?logo=typescript&logoColor=white)
![Fortran](https://img.shields.io/badge/Fortran-%23734F96.svg?logo=fortran&logoColor=white)
![Tauri](https://img.shields.io/badge/tauri-%2324C8DB.svg?logo=tauri&logoColor=%23FFFFFF)
![Svelte](https://img.shields.io/badge/svelte-%23f1413d.svg?logo=svelte&logoColor=white)

# [![Download](images/download.svg)](https://github.com/skyline69/balatro-mod-manager/releases/latest)

Balatro Mod Manager is available for Windows, macOS, and Linux. The installer is standalone and does not require any external libraries.

> Note: The Balatro Mod Manager is **NOT** compatible with the Xbox Gamepass version of Balatro

[Download the Balatro Mod Manager installer here](https://github.com/skyline69/balatro-mod-manager/releases/latest).

Scroll down to find **▸Assets** and download the right version of the installer for your system.

- Windows: `Balatro.Mod.Manager_…_x64-setup.exe` or `Balatro.Mod.Manager_…_x64_en-US.msi`
- macOS: `Balatro.Mod.Manager_…_universal.dmg`
- Linux: Flatpak (recommended), AUR, or `Balatro.Mod.Manager_…_amd64.AppImage`

### Installing on macOS

The macOS build is signed ad-hoc (not notarized by Apple), so Gatekeeper blocks the first launch. After installing once, the app opens normally on every subsequent launch.

1. Mount the `.dmg` and drag **Balatro Mod Manager** into `/Applications`.
2. Try to open it. macOS will refuse with either *"Apple cannot verify..."* or *"is damaged and can't be opened"*. Close the dialog.
3. Pick one of the following bypasses:

   **Option A — System Settings (macOS 14 and newer, no Terminal):**
   - Open **System Settings → Privacy & Security**.
   - Scroll to the **Security** section. You should see a line about Balatro Mod Manager being blocked, with an **Open Anyway** button. Click it and confirm.
   - If no button appears (macOS 15+ sometimes hides it for "damaged" bundles), use Option B.

   **Option B — Terminal one-liner (works on every macOS version):**
   ```bash
   xattr -dr com.apple.quarantine "/Applications/Balatro Mod Manager.app"
   ```
   Then double-click the app normally.

   **Option C — Older macOS (13 and below):** right-click the app → **Open** → confirm in the dialog.

Why this happens: shipping notarized builds requires an active Apple Developer Program membership ($99/year). The app is ad-hoc signed so it runs on Apple Silicon, but Apple's notarization service is what would silence Gatekeeper. The friction is one-time per install.

### Arch Linux (AUR)

Install using your preferred AUR helper:

```bash
yay -S balatro-mod-manager-bin
# or
paru -S balatro-mod-manager-bin
```

### Debian / Ubuntu

Download the `.deb` from the [latest release](https://github.com/skyline69/balatro-mod-manager/releases/latest) and install:

```bash
sudo apt install ./Balatro.Mod.Manager_*_amd64.deb
```

## Flatpak (Steam Deck/Linux)

> You need [flatpak-builder](https://docs.flatpak.org/en/latest/flatpak-builder.html) for this.

- Run from a local checkout:
  ```bash
  git clone https://github.com/skyline69/balatro-mod-manager.git
  cd balatro-mod-manager
  ```
- Install runtimes once (GNOME 50 + toolchain extensions):
  ```bash
  flatpak install org.gnome.Platform//50 org.gnome.Sdk//50 \
    org.freedesktop.Sdk.Extension.node20//25.08 \
    org.freedesktop.Sdk.Extension.rust-stable//25.08
  ```
- Build + bundle from this repo:
  ```bash
  flatpak-builder --force-clean --repo=repo build-dir packaging/flatpak/io.balatro.ModManager.json
  flatpak build-bundle repo balatro-mod-manager.flatpak io.balatro.ModManager master
  ```
- Install/run (on the Deck or any Flatpak host):
  ```bash
  flatpak install --user balatro-mod-manager.flatpak
  flatpak run io.balatro.ModManager
  ```
  AppImage/Deb/RPM still land in `target/release/bundle/` during the Flatpak build if you need them.

# [![Build](images/build.svg)](#build-prerequisites)

Alternatively, if you would prefer to build Balatro Mod Manager yourself instead of downloading the [prebuilt installer](https://github.com/skyline69/balatro-mod-manager/releases/latest), Balatro Mod Manager can be compiled from source using the instructions below.

## Build Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (for the backend)
- [Bun](https://bun.sh/) (for the frontend)
- [Tauri CLI](https://tauri.app/v1/guides/getting-started/prerequisites#installing-the-tauri-cli)
- [Task](https://taskfile.dev/) (for running task commands)
  - **Arch Linux**: `sudo pacman -S go-task` (package is named `go-task` to avoid conflicts)
  - **Fedora**: `sudo dnf install go-task`
  - **Ubuntu/Debian**: Install via the [official script](https://taskfile.dev/installation/#install-script) or Snap: `sudo snap install task --classic`
  - **macOS**: `brew install go-task`
  - **Windows**: `choco install go-task` or `winget install Task.Task`

## Automatic Installation

### For Windows

open Powershell & run this command:

```powershell
iwr https://raw.githubusercontent.com/skyline69/balatro-mod-manager/main/scripts/install.ps1 -useb | iex
```

### For macOS

run this command:

```bash
curl -sL https://raw.githubusercontent.com/skyline69/balatro-mod-manager/main/scripts/install.sh | bash
```

### For Linux (Flatpak)

From a local checkout (respects your current branch/changes):

```bash
./scripts/linux-install.sh
```

If you want to install from the latest GitHub main without cloning manually, pass `--clone`:

```bash
curl -sL https://raw.githubusercontent.com/skyline69/balatro-mod-manager/main/scripts/linux-install.sh | bash -s -- --clone
```

The script now prefers the latest GitHub Release Flatpak if available, and falls back to a local build if not.

Launch after install:

```bash
flatpak run io.balatro.ModManager
```

Linux builds use Flatpak, so you’ll need `flatpak` and `flatpak-builder` installed.
On Linux the manager currently supports the Steam version of Balatro only.

## Manual Installation

1. Clone the repository & install bun's dependencies:
   ```sh
   git clone https://github.com/skyline69/balatro-mod-manager.git
   cd balatro-mod-manager && bun install --allow-scripts
   ```
2. Run the task based on your OS
   - For Windows:
     ```sh
     task release-windows
     ```
   - For macOS:
     ```sh
     task release-macos
     ```
   - For Linux:
     ```sh
     task release-linux
     ```

## Running the Project

### Development Mode

To start the project in development mode, use the provided taskfile:

1. Run the debug target:
   ```sh
   task debug
   ```

> Linux/Wayland: on Wayland sessions the app now prefers X11 (XWayland) to avoid compositor protocol errors. Set `BMM_ALLOW_WAYLAND=1` before running if you want to keep native Wayland.

### Production Mode

To build the project for production:

1. Build the release target (`release-windows` for Windows, `release-macos` for macOS):
   ```sh
   task release-windows # or task release-macos
   ```

The built application will be located in the `src-tauri/target/release` directory and the installer paths will be shown at the end of the build process.

## Cleaning the Build

To clean the build files, use the provided taskfile:

1. Run the clean target:
   ```sh
   task clean
   ```

> Font by Daniel Linssen


# Code Signing

Balatro Mod Manager releases are code-signed using [SignPath](https://signpath.io) to ensure authenticity and security. This helps verify that the downloaded software hasn't been tampered with and comes from a trusted source.
