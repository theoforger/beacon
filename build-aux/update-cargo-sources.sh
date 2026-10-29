#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Regenerate build-aux/cargo-sources.json from Cargo.lock so the Flatpak build
# can run offline. Run this after every change to Cargo.lock.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
python3 "$root/build-aux/flatpak-cargo-sources.py" \
    "$root/Cargo.lock" "$root/build-aux/cargo-sources.json"
