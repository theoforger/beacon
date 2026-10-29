#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Screenshot target/debug/beacon headless after driving it into a state.
#
#   shot.sh SIZE SCHEME OUT [STEP...]
#   STEP: key:NAME[+NAME…]   e.g. key:F10, key:Control_L+f
#         click:X,Y[,BUTTON]  e.g. click:28,27, click:860,450,8 (mouse back)
#
# Runs on a private session bus so an open Beacon is never activated instead
# (which leaves a black capture). Build first with `cargo build`.
set -eu

root=$(cd "$(dirname "$0")/../.." && pwd)
size=$1 scheme=$2 out=$3
shift 3
width=${size%x*}
height=${size#*x}

steps=""
for step in "$@"; do
    case $step in
        key:*) steps="$steps python3 -I '$root/build-aux/headless/input.py' key $(echo "${step#key:}" | tr + ' '); sleep 1;" ;;
        click:*) steps="$steps python3 -I '$root/build-aux/headless/input.py' click $(echo "${step#click:}" | tr , ' '); sleep 1;" ;;
        *) echo "unknown step: $step" >&2; exit 2 ;;
    esac
done

mkdir -p "$root/target/schemas"
glib-compile-schemas --targetdir="$root/target/schemas" "$root/data"
config=$(mktemp -d)
trap 'rm -rf "$config"' EXIT

export GSETTINGS_SCHEMA_DIR="$root/target/schemas" GSETTINGS_BACKEND=keyfile
export XDG_CONFIG_HOME="$config" GDK_BACKEND=x11 LIBGL_ALWAYS_SOFTWARE=1
export ADW_DEBUG_COLOR_SCHEME="prefer-$scheme"

dbus-run-session -- xvfb-run -a -s "-screen 0 ${size}x24" sh -c "
    gsettings set com.theoforger.Beacon window-width $width
    gsettings set com.theoforger.Beacon window-height $height
    '$root/target/debug/beacon' >/dev/null 2>&1 &
    pid=\$!
    sleep 3
    $steps
    sleep 1
    import -window root '$out'
    kill \$pid" >/dev/null 2>&1

printf '%s brightness %s\n' "$out" "$(magick "$out" -format '%[fx:mean]' info:)"
