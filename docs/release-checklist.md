# Release Checklist

## Candidate

1. Confirm `main` is clean and the local Git identity is personal.
2. Run `scripts/verify-release.sh` on each publishable native target.
3. Inspect each archive and verify its adjacent SHA-256 file.
4. Run `smt --version`, `smt doctor`, and a short `smt run -- <command>` smoke test.
5. Complete the manual rows in [compatibility.md](compatibility.md).
6. Confirm `.github/workflows` has no files and no secret or private configuration is staged.

## Publish

Publishing requires explicit approval after the release candidate is merged.

1. Create signed or annotated tag `v1.0.0` at the verified `main` commit.
2. Create the GitHub Release from `CHANGELOG.md`.
3. Upload only verified target archives and their `.sha256` sidecars.
4. Download one uploaded artifact, verify its checksum, install it, and run `smt --version`.

## Rollback

1. Mark the GitHub Release as a prerelease or remove affected assets.
2. Publish a clear advisory naming affected targets and checksums.
3. Fix forward with a patch version; never move or overwrite the published tag.
