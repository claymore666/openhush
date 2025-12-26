#!/bin/bash
# Sign and notarize macOS app for distribution
# Requires: Apple Developer account and valid signing identity

set -e

APP_PATH="${1:-OpenHush.app}"
DMG_PATH="${2:-OpenHush.dmg}"
SIGNING_IDENTITY="${SIGNING_IDENTITY:-Developer ID Application: Your Name (TEAMID)}"
NOTARIZE_PROFILE="${NOTARIZE_PROFILE:-openhush-notarize}"

echo "Signing and notarizing OpenHush..."

# Check prerequisites
if [ ! -d "$APP_PATH" ]; then
    echo "Error: App bundle not found at $APP_PATH"
    exit 1
fi

# Sign the app
echo "Signing app bundle..."
codesign --force --deep --sign "$SIGNING_IDENTITY" \
    --entitlements entitlements.plist \
    --options runtime \
    "$APP_PATH"

# Verify signature
echo "Verifying signature..."
codesign --verify --deep --strict "$APP_PATH"
spctl --assess --type execute "$APP_PATH"

# If DMG exists, sign it too
if [ -f "$DMG_PATH" ]; then
    echo "Signing DMG..."
    codesign --force --sign "$SIGNING_IDENTITY" "$DMG_PATH"

    # Notarize DMG
    echo "Submitting for notarization..."
    xcrun notarytool submit "$DMG_PATH" \
        --keychain-profile "$NOTARIZE_PROFILE" \
        --wait

    # Staple the notarization ticket
    echo "Stapling notarization ticket..."
    xcrun stapler staple "$DMG_PATH"

    echo "Notarization complete!"
fi

echo "Done!"
echo ""
echo "To set up notarization credentials:"
echo "  xcrun notarytool store-credentials openhush-notarize"
echo "  # Enter Apple ID, Team ID, and app-specific password"
