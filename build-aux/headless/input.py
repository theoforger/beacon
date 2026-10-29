#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Send input to the X display in $DISPLAY through XTest.

    input.py key NAME...          press NAMEs together (for example Control_L f)
    input.py click X Y [BUTTON]   move the pointer and click (8 is the back button)
    input.py type TEXT            type TEXT (ASCII only)
"""

import sys
import time

from Xlib import X, XK, display
from Xlib.ext import xtest


NAMES = {
    " ": "space", ":": "colon", "/": "slash", ".": "period", "-": "minus",
    "_": "underscore", "?": "question", "=": "equal", "&": "ampersand",
}


def main(args):
    d = display.Display()
    if args[0] == "key":
        codes = [d.keysym_to_keycode(XK.string_to_keysym(name)) for name in args[1:]]
        for code in codes:
            xtest.fake_input(d, X.KeyPress, code)
        for code in reversed(codes):
            xtest.fake_input(d, X.KeyRelease, code)
    elif args[0] == "click":
        x, y = int(args[1]), int(args[2])
        button = int(args[3]) if len(args) > 3 else 1
        xtest.fake_input(d, X.MotionNotify, x=x, y=y)
        d.sync()
        time.sleep(0.2)
        xtest.fake_input(d, X.ButtonPress, button)
        xtest.fake_input(d, X.ButtonRelease, button)
    elif args[0] == "type":
        for char in " ".join(args[1:]):
            keysym = XK.string_to_keysym(NAMES.get(char, char))
            code = d.keysym_to_keycode(keysym)
            shift = d.keycode_to_keysym(code, 0) != keysym
            shift_code = d.keysym_to_keycode(XK.string_to_keysym("Shift_L"))
            if shift:
                xtest.fake_input(d, X.KeyPress, shift_code)
            xtest.fake_input(d, X.KeyPress, code)
            xtest.fake_input(d, X.KeyRelease, code)
            if shift:
                xtest.fake_input(d, X.KeyRelease, shift_code)
            d.sync()
            time.sleep(0.02)
    else:
        sys.exit(__doc__)
    d.sync()


if __name__ == "__main__":
    main(sys.argv[1:])
