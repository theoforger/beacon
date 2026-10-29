#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Called by Meson: cargo-build.sh CARGO SOURCE_ROOT TARGET_DIR PROFILE OUTPUT
set -eu

cargo=$1
source_root=$2
target_dir=$3
profile=$4
output=$5

if [ "$profile" = release ]; then
    "$cargo" build --release --manifest-path "$source_root/Cargo.toml" --target-dir "$target_dir" --bin beacon
else
    "$cargo" build --manifest-path "$source_root/Cargo.toml" --target-dir "$target_dir" --bin beacon
fi

cp "$target_dir/$profile/beacon" "$output"
