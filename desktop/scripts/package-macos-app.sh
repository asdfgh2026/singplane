#!/bin/bash
# Assemble desktop/dist/SingPanel.app and copy it to /Applications or ~/Applications.
set -euo pipefail

REPO="$(cd "$(dirname "$0")/../.." && pwd)"
DESKTOP="$REPO/desktop"
DIST="$DESKTOP/dist"
APP="$DIST/SingPanel.app"
ICONSET="$DIST/AppIcon.iconset"
GPUI="$DESKTOP/target/release/singpanel-gpui"
HOST="$REPO/core/host/target/release/singpanel-host"

if [ ! -x "$GPUI" ]; then
  echo "missing $GPUI — cargo build --release first" >&2
  exit 1
fi
if [ ! -x "$HOST" ]; then
  echo "missing $HOST — cargo build --release -p singpanel-host first" >&2
  exit 1
fi

rm -rf "$APP" "$ICONSET"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$ICONSET"

PNG="$DESKTOP/assets/app_icon.png"
sips -z 16 16     "$PNG" --out "$ICONSET/icon_16x16.png" >/dev/null
sips -z 32 32     "$PNG" --out "$ICONSET/icon_16x16@2x.png" >/dev/null
sips -z 32 32     "$PNG" --out "$ICONSET/icon_32x32.png" >/dev/null
sips -z 64 64     "$PNG" --out "$ICONSET/icon_32x32@2x.png" >/dev/null
sips -z 128 128   "$PNG" --out "$ICONSET/icon_128x128.png" >/dev/null
sips -z 256 256   "$PNG" --out "$ICONSET/icon_128x128@2x.png" >/dev/null
sips -z 256 256   "$PNG" --out "$ICONSET/icon_256x256.png" >/dev/null
sips -z 512 512   "$PNG" --out "$ICONSET/icon_256x256@2x.png" >/dev/null
sips -z 512 512   "$PNG" --out "$ICONSET/icon_512x512.png" >/dev/null
sips -z 1024 1024 "$PNG" --out "$ICONSET/icon_512x512@2x.png" >/dev/null
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"
rm -rf "$ICONSET"

cp "$DESKTOP/macos/Info.plist" "$APP/Contents/Info.plist"
VER="${SINGPANEL_VERSION:-}"
if [ -z "$VER" ]; then
  VER="$(sed -nE 's/^version = "([^"]+)".*/\1/p' "$DESKTOP/Cargo.toml" | head -1)"
fi
if [ -n "$VER" ]; then
  python3 - "$APP/Contents/Info.plist" "$VER" <<'PY'
from pathlib import Path
import re
import sys
path, ver = Path(sys.argv[1]), sys.argv[2]
text = path.read_text()
text = re.sub(
    r"(<key>CFBundleShortVersionString</key>\s*<string>)[^<]+",
    rf"\g<1>{ver}",
    text,
    count=1,
)
text = re.sub(
    r"(<key>CFBundleVersion</key>\s*<string>)[^<]+",
    rf"\g<1>{ver}",
    text,
    count=1,
)
path.write_text(text)
PY
fi
cp "$DESKTOP/macos/SingPanel-launch" "$APP/Contents/MacOS/SingPanel"
chmod 755 "$APP/Contents/MacOS/SingPanel"
cp "$GPUI" "$APP/Contents/MacOS/singpanel-gpui"
cp "$HOST" "$APP/Contents/MacOS/singpanel-host"
chmod 755 "$APP/Contents/MacOS/singpanel-gpui" "$APP/Contents/MacOS/singpanel-host"

xattr -cr "$APP" 2>/dev/null || true
codesign --force --deep --sign - "$APP"

DEST=""
if mkdir -p /Applications 2>/dev/null && [ -w /Applications ]; then
  DEST="/Applications/SingPanel.app"
elif mkdir -p "$HOME/Applications" && [ -w "$HOME/Applications" ]; then
  DEST="$HOME/Applications/SingPanel.app"
else
  echo "built $APP (no writable Applications folder)"
  exit 0
fi

rm -rf "$DEST"
cp -R "$APP" "$DEST"
xattr -cr "$DEST" 2>/dev/null || true
codesign --force --deep --sign - "$DEST"
echo "installed $DEST"
