#!/bin/sh
set -eu

root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"

version=$(cargo metadata --locked --no-deps --format-version 1 | sed -n 's/.*"version":"\([^"]*\)".*/\1/p')
host=$(rustc -vV | sed -n 's/^host: //p')
target=${1:-$host}
package="savemyterminal-$version-$target"
stage="$root/dist/stage/$package"
archive="$root/dist/$package.tar.gz"

if [ "$target" = "$host" ]; then
    cargo build --locked --release
    binary="$root/target/release/smt"
else
    cargo build --locked --release --target "$target"
    binary="$root/target/$target/release/smt"
fi

test -x "$binary"
rm -rf "$stage"
mkdir -p "$stage/docs"
cp "$binary" "$stage/smt"
cp README.md LICENSE CHANGELOG.md "$stage/"
cp docs/compatibility.md "$stage/docs/compatibility.md"

mkdir -p "$root/dist"
tar -C "$root/dist/stage" -czf "$archive" "$package"

cd "$root/dist"
archive_name=$(basename "$archive")
if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$archive_name" > "$archive_name.sha256"
elif command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$archive_name" > "$archive_name.sha256"
else
    echo "No SHA-256 tool found (need shasum or sha256sum)" >&2
    exit 1
fi

printf '%s\n%s\n' "$archive" "$archive.sha256"
