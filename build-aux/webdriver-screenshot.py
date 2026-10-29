#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Screenshot a page through WebKitWebDriver on port 4444: URL OUT WIDTH HEIGHT. Used by mockup-screenshots.sh."""
import base64, json, sys, time, urllib.request
url, out, w, h = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4])
B = "http://127.0.0.1:4444"
def call(method, path, body=None):
    req = urllib.request.Request(B + path, method=method, data=json.dumps(body).encode() if body is not None else None,
                                 headers={"Content-Type": "application/json"})
    return json.load(urllib.request.urlopen(req, timeout=60))["value"]
caps = {"capabilities": {"alwaysMatch": {"webkitgtk:browserOptions": {"binary": "/usr/lib/webkitgtk-6.0/MiniBrowser", "args": ["--automation"]}}}}
sid = call("POST", "/session", caps)["sessionId"]
try:
    call("POST", f"/session/{sid}/window/rect", {"width": w, "height": h})
    call("POST", f"/session/{sid}/url", {"url": url})
    time.sleep(6)
    open(out, "wb").write(base64.b64decode(call("GET", f"/session/{sid}/screenshot")))
finally:
    call("DELETE", f"/session/{sid}")
