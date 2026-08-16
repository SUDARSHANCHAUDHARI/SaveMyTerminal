#!/bin/sh
set -eu

root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"

cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
cargo build --locked --release
output=$(scripts/package.sh)
archive=$(printf '%s\n' "$output" | tail -2 | head -1)
archive_dir=$(CDPATH='' cd -- "$(dirname -- "$archive")" && pwd)
checksum=$(basename "$archive.sha256")

if command -v shasum >/dev/null 2>&1; then
    (cd "$archive_dir" && shasum -a 256 -c "$checksum")
elif command -v sha256sum >/dev/null 2>&1; then
    (cd "$archive_dir" && sha256sum -c "$checksum")
else
    echo "No SHA-256 tool found (need shasum or sha256sum)" >&2
    exit 1
fi

case "$archive" in
    *.tar.gz) tar -tzf "$archive" | grep '/savemyterminal$' >/dev/null ;;
    *) echo "Unexpected host archive: $archive" >&2; exit 1 ;;
esac

if [ -d .github/workflows ] && find .github/workflows -type f -print -quit | grep . >/dev/null; then
    echo "GitHub Actions workflows are not allowed" >&2
    exit 1
fi

if rg -n 'https://' src; then
    echo "Remote runtime URL found in src" >&2
    exit 1
fi

if rg -n 'http://' src | grep -v 'http://{address}'; then
    echo "Non-loopback runtime URL found in src" >&2
    exit 1
fi

private_key_pattern='BEGIN (RSA|OPENSSH|EC) PRIVATE'' KEY'
github_token_pattern='ghp_''[A-Za-z0-9]{20,}'
if git diff HEAD -- . ':!Cargo.lock' | rg -n "$private_key_pattern|$github_token_pattern"; then
    echo "Secret-shaped content found in release diff" >&2
    exit 1
fi

git diff --check
printf 'Release verification passed: %s\n' "$archive"
