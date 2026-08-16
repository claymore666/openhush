#!/usr/bin/env bash
#
# Verify that a built .deb actually installs and runs.
#
# A green packaging job only proves a file was produced. The v0.8.0 .deb
# installed cleanly and then failed to start, because the control file
# declared libasound2 and libdbus-1-3 but the binary also needs libpulse,
# libX11 and libXtst. This script exists so that can never ship silently
# again: it installs the package in clean containers and runs the binary.
#
# Usage:
#   packaging/verify-deb.sh <path-to-deb> [image ...]
#
# Defaults to the build target plus current Debian stable. Any extra
# arguments replace the default image list.
#
# Requires docker. Exits non-zero on the first image that fails.

set -euo pipefail

DEB_PATH="${1:-}"
if [[ -z "${DEB_PATH}" || ! -f "${DEB_PATH}" ]]; then
    echo "usage: $0 <path-to-deb> [image ...]" >&2
    exit 2
fi
shift || true

if [[ $# -gt 0 ]]; then
    IMAGES=("$@")
else
    # ubuntu:22.04 is what release.yml builds on; debian:13 catches the
    # t64 library renames that a newer stable can introduce.
    IMAGES=("ubuntu:22.04" "debian:13")
fi

DEB_ABS="$(cd "$(dirname "${DEB_PATH}")" && pwd)/$(basename "${DEB_PATH}")"
DEB_NAME="$(basename "${DEB_ABS}")"

# Version the binary must report, taken from the package metadata so the
# script has an independent expectation rather than trusting whatever runs.
EXPECTED_VERSION="$(dpkg-deb -f "${DEB_ABS}" Version 2>/dev/null || true)"
if [[ -z "${EXPECTED_VERSION}" ]]; then
    echo "FAIL: could not read Version from ${DEB_NAME}" >&2
    exit 1
fi

echo "Package:  ${DEB_NAME}"
echo "Version:  ${EXPECTED_VERSION}"
echo "Depends:  $(dpkg-deb -f "${DEB_ABS}" Depends)"
echo "Images:   ${IMAGES[*]}"
echo

# Runs inside the container. Kept as a single here-doc so the script has
# no runtime dependency on files other than the .deb itself.
read -r -d '' PROBE <<'PROBE_EOF' || true
set -u
fail() { echo "FAIL: $*"; exit 1; }

export DEBIAN_FRONTEND=noninteractive
apt-get update -qq >/dev/null 2>&1 || fail "apt-get update"

# Install through apt so declared dependencies are actually resolved.
# dpkg -i would happily leave them unsatisfied and hide the bug.
apt-get install -y -qq "/work/${DEB_NAME}" >/dev/null 2>&1 \
    || fail "apt-get install refused the package (unsatisfiable Depends?)"
echo "  ok  install"

MISSING="$(ldd /usr/bin/openhush 2>/dev/null | grep -i 'not found' || true)"
[ -n "${MISSING}" ] && fail "unresolved shared libraries:
${MISSING}"
echo "  ok  no missing shared libraries"

ACTUAL="$(/usr/bin/openhush --version 2>&1 | head -1 || true)"
case "${ACTUAL}" in
    *"${EXPECTED_VERSION}"*) echo "  ok  --version reports '${ACTUAL}'" ;;
    *) fail "--version printed '${ACTUAL}', expected it to contain '${EXPECTED_VERSION}'" ;;
esac

/usr/bin/openhush --help >/dev/null 2>&1 || fail "--help exited non-zero"
echo "  ok  --help"

# Exercises config loading and the XDG path logic, which is the first
# thing to break in a bare environment with no HOME contents.
/usr/bin/openhush config --show >/dev/null 2>&1 || fail "config --show exited non-zero"
echo "  ok  config --show"
PROBE_EOF

STATUS=0
for image in "${IMAGES[@]}"; do
    echo "=== ${image} ==="
    if docker run --rm \
        -v "${DEB_ABS}:/work/${DEB_NAME}:ro" \
        -e "DEB_NAME=${DEB_NAME}" \
        -e "EXPECTED_VERSION=${EXPECTED_VERSION}" \
        "${image}" bash -c "${PROBE}"
    then
        echo "PASS ${image}"
    else
        echo "FAIL ${image}"
        STATUS=1
    fi
    echo
done

if [[ ${STATUS} -eq 0 ]]; then
    echo "All images passed."
else
    echo "One or more images failed." >&2
fi
exit ${STATUS}
