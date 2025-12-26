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
sudo dpkg -i ../openhush_0.5.0-1_amd64.deb
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
./build-dmg.sh 0.5.0 ../../target/release/openhush

# Optional: Sign and notarize
./sign-and-notarize.sh OpenHush.app OpenHush-0.5.0-macos-universal.dmg
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
.\build-msi.ps1 -Version 0.5.0 -SourceDir ..\..\target\release
```

**Output:** `output/OpenHush-0.5.0-x64.msi`

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

## Release Checklist

Before releasing a new version:

1. **Version Bump:**
   - `Cargo.toml`
   - `packaging/deb/debian/changelog`
   - `packaging/aur/PKGBUILD`
   - `packaging/flatpak/org.openhush.OpenHush.yml`
   - `packaging/flatpak/org.openhush.OpenHush.metainfo.xml`
   - `packaging/homebrew/openhush.rb`
   - `packaging/windows/openhush.wxs`

2. **Update Checksums:**
   - SHA256 in PKGBUILD
   - SHA256 in Homebrew formula

3. **Test Builds:**
   - Flatpak on Ubuntu
   - AUR on Arch
   - Deb on Debian/Ubuntu
   - DMG on macOS
   - MSI on Windows

4. **Upload Artifacts:**
   - Attach .deb, .dmg, .msi to GitHub Release
   - Update Flathub PR
   - Update AUR package

---

## CI/CD Integration

See `.github/workflows/release.yml` for automated packaging during releases.
