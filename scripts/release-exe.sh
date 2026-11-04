#!/bin/sh
# Builds the release desktop app and copies it to dist/RustyAim.exe.
# Used by the pre-commit hook; run it directly to get a fresh executable.
set -e
cd "$(dirname "$0")/.."

cargo build --release -p rustyaim-app --features custom-protocol --quiet

# The target folder can be moved by a local .cargo/config.toml, so ask cargo where it is.
target=$(cargo metadata --format-version 1 --no-deps | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p' | sed 's/\\\\/\//g')
mkdir -p dist
cp "$target/release/rustyaim-app.exe" dist/RustyAim.exe 2>/dev/null || {
  echo "Couldn't replace dist/RustyAim.exe. Close RustyAim if it's running, then try again." >&2
  exit 1
}
echo "Built dist/RustyAim.exe"
