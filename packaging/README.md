# OpenHush Packaging

This directory contains packaging files for all supported platforms.

## Overview

| Format | Platform | Location | Status |
|--------|----------|----------|--------|
| Flatpak | Linux | `flatpak/` | Ready |
| AUR | Arch Linux | `aur/` | Ready |
| Deb | Debian/Ubuntu | `deb/` | Ready |
| Homebrew | macOS | `homebrew/` | Ready |
| MSI | Windows | `windows/` | Ready |
| DMG | macOS | `macos/` | Ready |

---

## Linux

### Flatpak

**Location:** `flatpak/`

```bash
# Build and install locally
flatpak-builder --user --install --force-clean build-dir \
    packaging/flatpak/org.openhush.OpenHush.yml
```

**Flathub Submission:**
1. Fork https://github.com/flathub/flathub
2. Add manifest and submit PR

### AUR (Arch Linux)

**Location:** `aur/`

```bash
cd packaging/aur
makepkg -si
```

**AUR Submission:**
1. Create account at https://aur.archlinux.org
2. `makepkg --printsrcinfo > .SRCINFO`
3. Push to AUR

### Debian/Ubuntu (.deb)

**Location:** `deb/`

```bash
# Install build dependencies
sudo apt install debhelper cargo rustc libasound2-dev libdbus-1-dev libgtk-3-dev

# Build package (from project root)
cp -r packaging/deb/debian .
dpkg-buildpackage -us -uc -b

# Install
sudo dpkg -i ../openhush_0.8.0-1_amd64.deb
```

---

## macOS

### Homebrew

**Location:** `homebrew/`

```bash
# Install from formula (after tap setup)
brew install openhush

# Or install directly from file
brew install --formula packaging/homebrew/openhush.rb
```

**Homebrew Submission:**
1. Create tap: `brew tap-new claymore666/openhush`
2. Add formula to tap
3. Or submit to homebrew-core (requires popularity)

### DMG Installer

**Location:** `macos/`

```bash
# Requires: create-dmg
brew install create-dmg

# Build DMG
cd packaging/macos
./build-dmg.sh 0.8.0 ../../target/release/openhush

# Optional: Sign and notarize
./sign-and-notarize.sh OpenHush.app OpenHush-0.8.0-macos-universal.dmg
```

**Code Signing:**
- Requires Apple Developer account ($99/year)
- Set `SIGNING_IDENTITY` environment variable
- Set up notarization profile with `xcrun notarytool store-credentials`

---

## Windows

### MSI Installer

**Location:** `windows/`

```powershell
# Requires: WiX Toolset v4+
# Install from https://wixtoolset.org/

# Build MSI
cd packaging\windows
.\build-msi.ps1 -Version 0.8.0 -SourceDir ..\..\target\release
```

**Output:** `output/OpenHush-0.8.0-x64.msi`

---

## Icons

Icons are needed for all packaging formats. Place them in `assets/icons/`:

```
assets/icons/
├── openhush.ico          # Windows (256x256 multi-resolution)
├── AppIcon.icns          # macOS (1024x1024 with mipmaps)
├── 16x16/openhush.png
├── 32x32/openhush.png
├── 48x48/openhush.png
├── 64x64/openhush.png
├── 128x128/openhush.png
├── 256x256/openhush.png
└── scalable/openhush.svg
```

---

## Verifying Packages

A green packaging job only proves a file was produced, not that it works.
The v0.8.0 `.deb` installed cleanly and then refused to start, because its
`Depends` omitted `libpulse0`, `libx11-6` and `libxtst6`. Always verify the
artifact, never the checkmark.

### Debian/Ubuntu (.deb) — automated

```bash
# Defaults to ubuntu:22.04 (the build target) and debian:13
packaging/verify-deb.sh path/to/openhush-v0.8.0-amd64.deb

# Or against specific images
packaging/verify-deb.sh openhush.deb ubuntu:24.04 debian:12
```

Requires `docker`. For each image it installs the package **via apt**, so
declared dependencies must actually resolve — `dpkg -i` would leave them
unsatisfied and hide exactly this class of bug. It then checks `ldd` reports
no missing libraries and that `--version`, `--help` and `config --show` all
work. Exits non-zero on the first failure.

This runs automatically in `release.yml` as the `verify-linux-package` job,
which gates the `release` job: if the package does not install and run, no
release is created.

### macOS (.dmg) and Windows (.msi) — manual

Not yet automated; both need a real OS. A macOS VM is documented in
`wiki/Platform-Support.md` (OSX-KVM). At minimum, confirm the image mounts,
the binary launches, and it reports the expected version.

---

## Release Checklist

Before releasing a new version:

1. **Version Bump:**
   - `Cargo.toml`
   - `packaging/deb/debian/changelog`
   - `packaging/aur/PKGBUILD`
   - `packaging/flatpak/org.openhush.OpenHush.yml`
   - `packaging/flatpak/org.openhush.OpenHush.metainfo.xml`
   - `packaging/homebrew/openhush.rb`
   - `packaging/homebrew/openhush.cask.rb`
   - `packaging/windows/openhush.wxs`
   - `packaging/windows/build-msi.ps1`
   - `packaging/macos/build-dmg.sh`

2. **Update Checksums:**

   Only possible once the tag exists, since the digests cover the published
   source archive and artifacts. Re-tagging invalidates them — recompute if
   the tag moves.
   - SHA256 in PKGBUILD (source archive)
   - SHA256 in Homebrew formula (source archive)
   - SHA256 in Homebrew cask (the built `.dmg`)

3. **Verify Packages:**
   - `.deb` — automatic via `verify-linux-package`; run `packaging/verify-deb.sh`
     locally to reproduce
   - `.dmg` on macOS — manual
   - `.msi` on Windows — manual
   - Flatpak on Ubuntu, AUR on Arch

4. **Upload Artifacts:**
   - `release.yml` attaches .deb, .dmg, .msi automatically and creates the
     release as a **draft** — inspect, then publish by hand
   - Update Flathub PR
   - Update AUR package

---

## CI/CD Integration

See `.github/workflows/release.yml` for automated packaging during releases.
