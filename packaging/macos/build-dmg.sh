#!/bin/bash
# Build macOS DMG installer
# Requires: create-dmg (brew install create-dmg)

set -e

VERSION="${1:-0.5.0}"
BINARY_PATH="${2:-../../target/release/openhush}"
OUTPUT_DIR="${3:-.}"
APP_NAME="OpenHush"
DMG_NAME="OpenHush-${VERSION}-macos-universal"

echo "Building OpenHush DMG v${VERSION}"

# Check prerequisites
if ! command -v create-dmg &> /dev/null; then
    echo "Error: create-dmg not found"
    echo "Install with: brew install create-dmg"
    exit 1
fi

if [ ! -f "$BINARY_PATH" ]; then
    echo "Error: Binary not found at $BINARY_PATH"
    echo "Build with: cargo build --release --features metal"
    exit 1
fi

# Create temporary app bundle directory
APP_DIR="$(mktemp -d)/${APP_NAME}.app"
mkdir -p "${APP_DIR}/Contents/MacOS"
mkdir -p "${APP_DIR}/Contents/Resources"

# Copy binary
cp "$BINARY_PATH" "${APP_DIR}/Contents/MacOS/openhush"
chmod +x "${APP_DIR}/Contents/MacOS/openhush"

# Create Info.plist
cat > "${APP_DIR}/Contents/Info.plist" << EOF
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
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>
    <key>LSUIElement</key>
    <true/>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSMicrophoneUsageDescription</key>
    <string>OpenHush needs microphone access for voice-to-text transcription.</string>
    <key>NSAppleEventsUsageDescription</key>
    <string>OpenHush needs accessibility access to type transcribed text.</string>
</dict>
</plist>
EOF

# Copy icon if exists
if [ -f "AppIcon.icns" ]; then
    cp "AppIcon.icns" "${APP_DIR}/Contents/Resources/"
fi

# Create DMG
echo "Creating DMG..."
mkdir -p "$OUTPUT_DIR"

create-dmg \
    --volname "OpenHush ${VERSION}" \
    --volicon "AppIcon.icns" \
    --window-pos 200 120 \
    --window-size 600 400 \
    --icon-size 100 \
    --icon "${APP_NAME}.app" 150 185 \
    --hide-extension "${APP_NAME}.app" \
    --app-drop-link 450 185 \
    --no-internet-enable \
    "${OUTPUT_DIR}/${DMG_NAME}.dmg" \
    "${APP_DIR}/../" \
    2>/dev/null || {
        # Fallback if create-dmg options fail
        echo "Using simple DMG creation..."
        hdiutil create -volname "OpenHush ${VERSION}" \
            -srcfolder "${APP_DIR}/../" \
            -ov -format UDZO \
            "${OUTPUT_DIR}/${DMG_NAME}.dmg"
    }

# Calculate SHA256
if [ -f "${OUTPUT_DIR}/${DMG_NAME}.dmg" ]; then
    echo "DMG created: ${OUTPUT_DIR}/${DMG_NAME}.dmg"
    shasum -a 256 "${OUTPUT_DIR}/${DMG_NAME}.dmg"
else
    echo "Error: DMG creation failed"
    exit 1
fi

# Cleanup
rm -rf "$(dirname "$APP_DIR")"

echo "Done!"
