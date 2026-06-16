#!/bin/sh
set -eu

version=${SMT_VERSION:-1.0.0}
if [ -n "${SMT_TARGET:-}" ]; then
    target=$SMT_TARGET
else
    os=$(uname -s)
    arch=$(uname -m)
    case "$os:$arch" in
        Darwin:arm64|Darwin:aarch64) target=aarch64-apple-darwin ;;
        Darwin:x86_64) target=x86_64-apple-darwin ;;
        Linux:x86_64) target=x86_64-unknown-linux-gnu ;;
        Linux:arm64|Linux:aarch64) target=aarch64-unknown-linux-gnu ;;
        *) echo "Unsupported platform; set SMT_TARGET explicitly" >&2; exit 1 ;;
    esac
fi
install_dir=${SMT_INSTALL_DIR:-"$HOME/.local/bin"}
download_tmp=
extract_tmp=

cleanup() {
    if [ -n "$download_tmp" ]; then rm -rf "$download_tmp"; fi
    if [ -n "$extract_tmp" ]; then rm -rf "$extract_tmp"; fi
}
trap cleanup EXIT HUP INT TERM

if [ "$#" -gt 0 ]; then
    archive=$1
    checksum="$archive.sha256"
else
    download_tmp=$(mktemp -d)
    name="savemyterminal-$version-$target.tar.gz"
    base="https://github.com/SUDARSHANCHAUDHARI/SaveMyTerminal/releases/download/v$version"
    archive="$download_tmp/$name"
    checksum="$archive.sha256"
    curl -fL "$base/$name" -o "$archive"
    curl -fL "$base/$name.sha256" -o "$checksum"
fi

test -f "$archive"
test -f "$checksum"
archive_dir=$(CDPATH='' cd -- "$(dirname -- "$archive")" && pwd)
if command -v shasum >/dev/null 2>&1; then
    (cd "$archive_dir" && shasum -a 256 -c "$(basename "$checksum")")
elif command -v sha256sum >/dev/null 2>&1; then
    (cd "$archive_dir" && sha256sum -c "$(basename "$checksum")")
else
    echo "No SHA-256 tool found (need shasum or sha256sum)" >&2
    exit 1
fi

extract_tmp=$(mktemp -d)
tar -xzf "$archive" -C "$extract_tmp"
binary=$(find "$extract_tmp" -type f -name smt -print -quit)
test -n "$binary"
mkdir -p "$install_dir"
install -m 0755 "$binary" "$install_dir/smt"
if [ "$(uname -s)" = Darwin ]; then
    codesign --force --sign - "$install_dir/smt"
fi
printf 'Installed %s\n' "$install_dir/smt"
