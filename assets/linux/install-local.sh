#!/bin/sh
# Instaluje Millwise dla bieżącego użytkownika (Linux): program, ikonę i skrót w menu.
# Użycie: assets/linux/install-local.sh   (z katalogu projektu, po `cargo build --release`)
set -e
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BIN="${XDG_BIN_HOME:-$HOME/.local/bin}"
DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
mkdir -p "$BIN" "$DATA/applications"
install -m 755 "$ROOT/target/release/millwise" "$BIN/millwise"
for s in 16 32 48 64 128 256 512; do
  mkdir -p "$DATA/icons/hicolor/${s}x${s}/apps"
  install -m 644 "$ROOT/assets/icon/millwise-$s.png" "$DATA/icons/hicolor/${s}x${s}/apps/millwise.png"
done
mkdir -p "$DATA/icons/hicolor/scalable/apps"
install -m 644 "$ROOT/assets/icon/millwise.svg" "$DATA/icons/hicolor/scalable/apps/millwise.svg"
install -m 644 "$ROOT/assets/linux/millwise.desktop" "$DATA/applications/millwise.desktop"
command -v update-desktop-database >/dev/null && update-desktop-database "$DATA/applications" || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q "$DATA/icons/hicolor" || true
echo "Zainstalowano: $BIN/millwise, ikona i skrót „Millwise” w menu."
