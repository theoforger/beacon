#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Headless screenshots for the HIG gate: 1440x900 and 360x640, light and dark.
# Usage: [SIZES="1440x900 360x640"] [SCHEMES="light dark"] build-aux/screenshots.sh [OUT_DIR]
# OUT_DIR defaults to target/screenshots. Take only the views a change needs.
# Needs dbus-run-session, xvfb-run and ImageMagick's `import`.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
out=${1:-"$root/target/screenshots"}
mkdir -p "$out" "$root/target/schemas"

cd "$root"
cargo build --quiet --bin beacon
glib-compile-schemas --strict --targetdir="$root/target/schemas" "$root/data"

config=$(mktemp -d)
trap 'rm -rf "$config"' EXIT

export GSETTINGS_SCHEMA_DIR="$root/target/schemas"
export GSETTINGS_BACKEND=keyfile
export XDG_CONFIG_HOME="$config"
export GDK_BACKEND=x11
export LIBGL_ALWAYS_SOFTWARE=1

for size in ${SIZES:-1440x900 360x640}; do
    width=${size%x*}
    height=${size#*x}
    for scheme in ${SCHEMES:-light dark}; do
        file="$out/$size-$scheme.png"
        # A private session bus, so a Beacon the user has open is never
        # found and activated instead of starting a new instance.
        ADW_DEBUG_COLOR_SCHEME="prefer-$scheme" dbus-run-session -- xvfb-run -a -s "-screen 0 ${size}x24" sh -c "
            gsettings set com.theoforger.Beacon window-width $width
            gsettings set com.theoforger.Beacon window-height $height
            '$root/target/debug/beacon' >/dev/null 2>&1 &
            pid=\$!
            sleep 3
            import -window root '$file'
            kill \$pid"
        echo "$file"
    done
done
