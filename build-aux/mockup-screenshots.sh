#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Render docs/design/mockup.html at 1440x900 and 360x640 with WebKitGTK's
# WebDriver and MiniBrowser on a virtual display.
# Usage: build-aux/mockup-screenshots.sh [OUT_DIR]   (default: docs/design)
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
out=${1:-"$root/docs/design"}
mkdir -p "$out"

for size in 1440x900 360x640; do
    xvfb-run -a -s "-screen 0 1600x1000x24" sh -c "
        WebKitWebDriver --port=4444 >/dev/null 2>&1 &
        driver=\$!
        sleep 2
        python3 -I '$root/build-aux/webdriver-screenshot.py' \
            'file://$root/docs/design/mockup.html' '$out/mockup-$size.png' ${size%x*} ${size#*x}
        kill \$driver"
    echo "$out/mockup-$size.png"
done
