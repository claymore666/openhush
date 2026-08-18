#!/usr/bin/env bash
#
# Build the macOS .app bundle and .dmg from an already-compiled binary.
#
# This is the definition the release uses. It used to be duplicated inline in
# .github/workflows/release.yml, and the two copies drifted: PR #240 corrected
# LSMinimumSystemVersion here while the release kept shipping the stale value
# from its own copy, and NSAppleEventsUsageDescription existed only here even
# though the workflow's plist is the one that reaches users. There is now one
# copy, and the release calls it.
#
# Usage:
#   packaging/macos/build-dmg.sh <version> <binary> [outdir]
#
# Produces <outdir>/openhush-v<version>-macos-universal.dmg. That exact name is
# what packaging/homebrew/openhush.cask.rb downloads; do not change it without
# changing the cask.
#
# Requires: macOS (hdiutil, plutil).

set -euo pipefail

VERSION="${1:-}"
BINARY="${2:-}"
OUTDIR="${3:-.}"

if [[ -z "${VERSION}" || -z "${BINARY}" ]]; then
    echo "usage: $0 <version> <binary> [outdir]" >&2
    exit 2
fi
if [[ ! -f "${BINARY}" ]]; then
    echo "error: binary not found at ${BINARY}" >&2
    echo "build it with: cargo build --release --features metal" >&2
    exit 1
fi

VERSION="${VERSION#v}"
mkdir -p "${OUTDIR}"
OUTDIR="$(cd "${OUTDIR}" && pwd)"
BINARY="$(cd "$(dirname "${BINARY}")" && pwd)/$(basename "${BINARY}")"

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DMG="${OUTDIR}/openhush-v${VERSION}-macos-universal.dmg"

WORK="$(mktemp -d)"
trap 'rm -rf "${WORK}"' EXIT

# Everything in STAGE becomes the contents of the mounted volume.
STAGE="${WORK}/stage"
APP_DIR="${STAGE}/OpenHush.app"
mkdir -p "${APP_DIR}/Contents/MacOS" "${APP_DIR}/Contents/Resources"

install -m 755 "${BINARY}" "${APP_DIR}/Contents/MacOS/openhush"

cat > "${APP_DIR}/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>openhush</string>
    <key>CFBundleIdentifier</key>
    <string>org.openhush.OpenHush</string>
    <key>CFBundleName</key>
    <string>OpenHush</string>
    <key>CFBundleDisplayName</key>
    <string>OpenHush</string>
    <key>CFBundleVersion</key>
    <string>${VERSION}</string>
    <key>CFBundleShortVersionString</key>
    <string>${VERSION}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleSignature</key>
    <string>????</string>
    <!-- The binary hard-links ScreenCaptureKit (LC_LOAD_DYLIB, not weak), so
         dyld refuses to start it on a system without that framework. It
         arrived in macOS 12.3, and the system audio capture built on it is
         documented as needing 13+. Claiming 11.0 let the app install where it
         cannot launch at all. -->
    <key>LSMinimumSystemVersion</key>
    <string>13.0</string>
    <key>LSUIElement</key>
    <true/>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSMicrophoneUsageDescription</key>
    <string>OpenHush needs microphone access for voice-to-text transcription.</string>
    <!-- src/context.rs shells out to osascript to read the frontmost
         application. That sends Apple Events, and macOS terminates a bundled
         app that does so without this key. -->
    <key>NSAppleEventsUsageDescription</key>
    <string>OpenHush needs to query the frontmost application to adapt transcription to its context.</string>
EOF

# Only declare an icon when one is actually present. The previous version
# referenced AppIcon unconditionally and passed --volicon for a file that does
# not exist in this repo, which made every create-dmg run fail into a silent
# fallback.
if [[ -f "${HERE}/AppIcon.icns" ]]; then
    cp "${HERE}/AppIcon.icns" "${APP_DIR}/Contents/Resources/"
    cat >> "${APP_DIR}/Contents/Info.plist" <<'EOF'
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
EOF
else
    echo "note: no AppIcon.icns in ${HERE}; building without an icon"
fi

cat >> "${APP_DIR}/Contents/Info.plist" <<'EOF'
</dict>
</plist>
EOF

# A malformed plist produces a bundle that silently refuses to launch, so
# reject it here rather than at the user's machine.
plutil -lint "${APP_DIR}/Contents/Info.plist"

# Lets someone who opens the .dmg drag the app across without leaving the
# window. Homebrew's cask ignores it and installs OpenHush.app directly.
ln -s /Applications "${STAGE}/Applications"

rm -f "${DMG}"
hdiutil create -volname "OpenHush ${VERSION}" \
    -srcfolder "${STAGE}" \
    -ov -format UDZO \
    "${DMG}"

echo "Built ${DMG}"
shasum -a 256 "${DMG}"
