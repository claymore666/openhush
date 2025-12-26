# OpenHush Packaging

This directory contains packaging files for various Linux distribution formats.

## Flatpak

**Location:** `flatpak/`

### Files

- `org.openhush.OpenHush.yml` - Flatpak manifest
- `org.openhush.OpenHush.desktop` - Desktop entry
- `org.openhush.OpenHush.metainfo.xml` - AppStream metadata

### Building Locally

```bash
# Install flatpak-builder
sudo apt install flatpak-builder

# Build and install locally
flatpak-builder --user --install --force-clean build-dir \
    packaging/flatpak/org.openhush.OpenHush.yml
```

### Submitting to Flathub

1. Fork https://github.com/flathub/flathub
2. Create a new branch with the manifest
3. Submit PR to Flathub

## AUR (Arch Linux)

**Location:** `aur/`

### Files

- `PKGBUILD` - Stable release package
- `PKGBUILD-git` - Development (git) package

### Building Locally

```bash
cd packaging/aur

# For stable release
makepkg -si

# For git version
cp PKGBUILD-git PKGBUILD
makepkg -si
```

### Submitting to AUR

1. Create AUR account at https://aur.archlinux.org
2. Generate .SRCINFO: `makepkg --printsrcinfo > .SRCINFO`
3. Clone AUR repo: `git clone ssh://aur@aur.archlinux.org/openhush.git`
4. Copy PKGBUILD and .SRCINFO
5. Push to AUR

## Icons

Icons are needed for packaging. Place them in `assets/icons/`:

```
assets/icons/
├── 16x16/openhush.png
├── 32x32/openhush.png
├── 48x48/openhush.png
├── 64x64/openhush.png
├── 128x128/openhush.png
├── 256x256/openhush.png
└── scalable/openhush.svg
```

## Desktop Entry

The desktop entry (`org.openhush.OpenHush.desktop`) is used by both Flatpak and AUR packages. It provides:

- Application launcher entry
- Quick actions (Start, Stop, Status)
- MIME type associations
- Keywords for search

## AppStream Metadata

The `metainfo.xml` file provides:

- Application description for software centers
- Screenshots (when available)
- Release notes
- Keywords for discovery
- Content rating

## Release Checklist

Before releasing a new version:

1. Update version in `PKGBUILD` (pkgver)
2. Update version in Flatpak manifest (tag)
3. Update `metainfo.xml` with new release entry
4. Generate new checksums for PKGBUILD
5. Test builds on both Flatpak and Arch
