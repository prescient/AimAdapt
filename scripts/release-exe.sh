#!/bin/sh
# Builds the release desktop app and copies it to dist/AimAdapt.exe.
# Used by the pre-commit hook; run it directly to get a fresh executable.
# On Linux, where the app is only built for development, it's dist/AimAdapt.
set -e
cd "$(dirname "$0")/.."

cargo build --release -p aimadapt-app --features custom-protocol --quiet

# The build is for the machine it runs on: an .exe on Windows, no extension elsewhere.
case "$(rustc -vV | sed -n 's/^host: //p')" in
  *windows*) ext=.exe ;;
  *) ext= ;;
esac

# The target folder can be moved by a local .cargo/config.toml, so ask cargo where it is.
target=$(cargo metadata --format-version 1 --no-deps | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p' | sed 's/\\\\/\//g')
mkdir -p dist
cp "$target/release/aimadapt-app$ext" "dist/AimAdapt$ext" 2>/dev/null || {
  echo "Couldn't replace dist/AimAdapt$ext. Close AimAdapt if it's running, then try again." >&2
  exit 1
}
# The app was called RustyAim. Once AimAdapt has moved the data folder, the old exe
# would start with an empty history, so don't leave it next to the new one.
rm -f dist/RustyAim.exe 2>/dev/null || echo "dist/RustyAim.exe is still running; delete it once it's closed." >&2
echo "Built dist/AimAdapt$ext"
