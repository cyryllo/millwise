#!/bin/sh
# Buduje AppImage lokalnie: program kompilowany w kontenerze Ubuntu 22.04
# (glibc 2.35 — działa na Debianie 12/13 i Ubuntu 22.04+), potem składany
# w AppImage. Zbudowany wprost na nowszym systemie nie uruchomiłby się na starszych.
#
#   packaging/linux/build-appimage-docker.sh [tag lub commit, domyślnie HEAD]
#
# Potrzebuje Dockera. Buduje z czystej kopii tej wersji (git archive), sprawdza
# `--version` gotowego AppImage i zapisuje go w dist/.
#
# Copyright (C) 2026 Cyryl Sochacki
# Licencja GNU GPL v3 lub późniejsza (plik LICENSE).
set -eu
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
REF="${1:-HEAD}"
IMAGE=millwise-build:ubuntu22.04
CACHE="$ROOT/target/docker"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/millwise-appimage-XXXXXX")"
trap 'rm -rf "$WORK"' EXIT

docker build -q -t "$IMAGE" "$ROOT/packaging/linux" >/dev/null
mkdir -p "$WORK/src" "$CACHE/cargo" "$CACHE/target"
git -C "$ROOT" archive "$REF" | tar -x -C "$WORK/src"

# kompilacja jako bieżący użytkownik (pliki nie należą do roota); pakiety crates i
# wyniki kompilacji zostają w target/docker na następny raz
docker run --rm --user "$(id -u):$(id -g)" \
    -e HOME=/tmp -e CARGO_HOME=/cache/cargo -e CARGO_TARGET_DIR=/cache/target \
    -v "$WORK/src:/src" -v "$CACHE:/cache" -w /src \
    "$IMAGE" cargo build --release --locked -p millwise

BIN="$CACHE/target/release/millwise"
echo "Wymagana glibc: $(objdump -T "$BIN" | grep -o 'GLIBC_[0-9.]*' | sort -V | tail -1)"
# składanie z plików tej samej wersji
APPIMAGETOOL="${APPIMAGETOOL:-$ROOT/target/tools/appimagetool-x86_64.AppImage}" \
    sh "$WORK/src/packaging/linux/build-appimage.sh" "$BIN"
mkdir -p "$ROOT/dist"
cp "$WORK/src/dist/"Millwise-*-x86_64.AppImage "$ROOT/dist/"
OUT="$(ls "$ROOT"/dist/Millwise-*-x86_64.AppImage | tail -1)"
APPIMAGE_EXTRACT_AND_RUN=1 "$OUT" --version
echo "Gotowe: $OUT"
