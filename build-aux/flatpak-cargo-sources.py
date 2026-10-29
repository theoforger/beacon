#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Generate Flatpak sources that vendor every crates.io dependency in Cargo.lock.

Writes the same layout as flatpak-builder-tools' generator: crate archives in
cargo/vendor/, an inline .cargo-checksum.json per crate, and a cargo config
that replaces crates.io with the vendored directory. Set CARGO_HOME to
/run/build/<module>/cargo in the manifest. Only crates.io sources are supported;
git dependencies make this script fail.
"""

import json
import sys
import tomllib

CRATES_IO = "registry+https://github.com/rust-lang/crates.io-index"

CARGO_CONFIG = """\
[source.vendored-sources]
directory = "cargo/vendor"

[source.crates-io]
replace-with = "vendored-sources"
"""


def main(lock_path: str, out_path: str) -> None:
    with open(lock_path, "rb") as f:
        lock = tomllib.load(f)

    sources = []
    for package in lock["package"]:
        source = package.get("source")
        if source is None:
            continue  # workspace member
        if source != CRATES_IO:
            sys.exit(f"unsupported source for {package['name']}: {source}")

        name, version, checksum = package["name"], package["version"], package["checksum"]
        dest = f"cargo/vendor/{name}-{version}"
        sources.append(
            {
                "type": "archive",
                "archive-type": "tar-gzip",
                "url": f"https://static.crates.io/crates/{name}/{name}-{version}.crate",
                "sha256": checksum,
                "dest": dest,
            }
        )
        sources.append(
            {
                "type": "inline",
                "contents": json.dumps({"package": checksum, "files": {}}),
                "dest": dest,
                "dest-filename": ".cargo-checksum.json",
            }
        )

    sources.append(
        {
            "type": "inline",
            "contents": CARGO_CONFIG,
            "dest": "cargo",
            "dest-filename": "config.toml",
        }
    )

    with open(out_path, "w") as f:
        json.dump(sources, f, indent=2)
        f.write("\n")


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit("usage: flatpak-cargo-sources.py Cargo.lock cargo-sources.json")
    main(sys.argv[1], sys.argv[2])
