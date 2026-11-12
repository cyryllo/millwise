#!/bin/sh
# Składa AppImage z gotowego programu.
#
#   packaging/linux/build-appimage.sh [ścieżka do programu, domyślnie target/release/millwise]
#
# Potrzebuje appimagetool (APPIMAGETOOL=…); bez niego pobiera go do target/tools/.
# Wynik: dist/Millwise-<wersja>-x86_64.AppImage
#
# Copyright (C) 2026 Cyryl Sochacki
# Licencja GNU GPL v3 lub późniejsza (plik LICENSE).
set -eu
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BIN="${1:-$ROOT/target/release/millwise}"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"
TOOL="${APPIMAGETOOL:-$ROOT/target/tools/appimagetool-x86_64.AppImage}"
if [ ! -x "$TOOL" ]; then
    mkdir -p "$(dirname "$TOOL")"
    curl -fsSL -o "$TOOL" \
        https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
    chmod +x "$TOOL"
fi

APPDIR="$ROOT/target/appimage/Millwise.AppDir"
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" "$APPDIR/usr/share/metainfo"
install -m 755 "$BIN" "$APPDIR/usr/bin/millwise"
strip "$APPDIR/usr/bin/millwise" 2>/dev/null || true
install -m 644 "$ROOT/packaging/linux/millwise.desktop" "$APPDIR/usr/share/applications/millwise.desktop"
cp "$ROOT/packaging/linux/millwise.desktop" "$APPDIR/millwise.desktop"
for s in 16 32 48 64 128 256 512; do
    mkdir -p "$APPDIR/usr/share/icons/hicolor/${s}x${s}/apps"
    install -m 644 "$ROOT/assets/icon/millwise-$s.png" "$APPDIR/usr/share/icons/hicolor/${s}x${s}/apps/millwise.png"
done
cp "$ROOT/assets/icon/millwise-256.png" "$APPDIR/millwise.png"
cp "$ROOT/assets/icon/millwise-256.png" "$APPDIR/.DirIcon"
sed "s/@VERSION@/$VERSION/" "$ROOT/packaging/linux/io.github.cyryllo.millwise.metainfo.xml" \
    > "$APPDIR/usr/share/metainfo/io.github.cyryllo.millwise.appdata.xml"
cat > "$APPDIR/AppRun" <<'RUN'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/millwise" "$@"
RUN
chmod 755 "$APPDIR/AppRun"

mkdir -p "$ROOT/dist"
OUT="$ROOT/dist/Millwise-$VERSION-x86_64.AppImage"
# bez FUSE (np. w kontenerze) appimagetool działa po rozpakowaniu
ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "$TOOL" --no-appstream "$APPDIR" "$OUT" >/dev/null
chmod +x "$OUT"
echo "Zapisano: $OUT"
