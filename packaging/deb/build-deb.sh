#!/usr/bin/env bash
#
# Build a .deb from an already-compiled openhush binary.
#
# This is the path the release uses: CI builds the binary once, then repacks
# it here. It is deliberately separate from debian/, which is the from-source
# path for distribution packagers using dpkg-buildpackage.
#
# The definition used to live inline in .github/workflows/release.yml. Keeping
# it there meant it could only be exercised by pushing a tag, and it silently
# drifted from reality: v0.8.0 shipped a package that installed cleanly and
# then could not start, because its Depends were written by hand and had
# fallen behind what the binary links against. Now the release calls this
# script, so what CI ships is what you can run locally.
#
# Usage:
#   packaging/deb/build-deb.sh <version> <binary> [outdir] [license]
#
# Verify the result with:
#   packaging/verify-deb.sh <path-to-deb>
#
# Requires: dpkg-dev (dpkg-shlibdeps, dpkg-deb) and fakeroot. The libraries
# the binary links against must be installed, or dpkg-shlibdeps cannot map
# them to packages.

set -euo pipefail

VERSION="${1:-}"
BINARY="${2:-}"
OUTDIR="${3:-.}"
LICENSE_FILE="${4:-LICENSE}"

if [[ -z "${VERSION}" || -z "${BINARY}" ]]; then
    echo "usage: $0 <version> <binary> [outdir] [license]" >&2
    exit 2
fi
if [[ ! -f "${BINARY}" ]]; then
    echo "error: binary not found at ${BINARY}" >&2
    exit 1
fi

VERSION="${VERSION#v}"
mkdir -p "${OUTDIR}"
OUTDIR="$(cd "${OUTDIR}" && pwd)"
BINARY="$(cd "$(dirname "${BINARY}")" && pwd)/$(basename "${BINARY}")"
[[ -f "${LICENSE_FILE}" ]] && LICENSE_FILE="$(cd "$(dirname "${LICENSE_FILE}")" && pwd)/$(basename "${LICENSE_FILE}")"

WORK="$(mktemp -d)"
trap 'rm -rf "${WORK}"' EXIT

PKG_DIR="${WORK}/openhush_${VERSION}_amd64"
mkdir -p "${PKG_DIR}/DEBIAN" \
         "${PKG_DIR}/usr/bin" \
         "${PKG_DIR}/usr/share/applications" \
         "${PKG_DIR}/usr/share/doc/openhush"

install -m 755 "${BINARY}" "${PKG_DIR}/usr/bin/openhush"

# Derive the runtime dependencies from the binary rather than maintaining a
# list by hand. dpkg-shlibdeps reads the ELF NEEDED entries and maps each to
# the package providing it, so the list cannot drift from what is linked.
#
# Deliberately no --ignore-missing-info: that flag drops libraries it cannot
# resolve instead of failing, which is precisely how an incomplete list slips
# through unnoticed.
mkdir -p "${WORK}/shlibdeps/debian"
cat > "${WORK}/shlibdeps/debian/control" <<'CTRL'
Source: openhush

Package: openhush
Architecture: amd64
Depends: ${shlibs:Depends}
Description: placeholder used only to compute dependencies
 dpkg-shlibdeps requires a debian/control to run against.
CTRL

SHLIB_DEPS="$(cd "${WORK}/shlibdeps" && dpkg-shlibdeps -O \
    "${PKG_DIR}/usr/bin/openhush" | sed 's/^shlibs:Depends=//')"

if [[ -z "${SHLIB_DEPS}" ]]; then
    echo "error: dpkg-shlibdeps returned no dependencies" >&2
    exit 1
fi
echo "Computed Depends: ${SHLIB_DEPS}"

cat > "${PKG_DIR}/DEBIAN/control" <<EOF
Package: openhush
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: amd64
Depends: ${SHLIB_DEPS}
Maintainer: OpenHush Team <christian.kamien@gmail.com>
Description: Voice-to-text transcription tool
 OpenHush is a local, privacy-focused voice-to-text tool
 that runs entirely on your machine using Whisper AI models.
EOF

cat > "${PKG_DIR}/usr/share/applications/openhush.desktop" <<'EOF'
[Desktop Entry]
Name=OpenHush
Comment=Voice-to-text transcription
Exec=openhush
Icon=openhush
Terminal=false
Type=Application
Categories=Utility;Audio;
EOF

if [[ -f "${LICENSE_FILE}" ]]; then
    install -m 644 "${LICENSE_FILE}" "${PKG_DIR}/usr/share/doc/openhush/copyright"
fi

# The metadata and data files above are created through redirection, so their
# modes come from the caller's umask. A restrictive umask would otherwise
# produce a package whose files are unreadable by anyone but root.
find "${PKG_DIR}/DEBIAN" "${PKG_DIR}/usr/share" -type f -exec chmod 644 {} +
find "${PKG_DIR}" -type d -exec chmod 755 {} +

fakeroot dpkg-deb --build "${PKG_DIR}" >/dev/null
mv "${PKG_DIR}.deb" "${OUTDIR}/openhush-v${VERSION}-amd64.deb"

echo "Built ${OUTDIR}/openhush-v${VERSION}-amd64.deb"
