# Releasing

Kyoko v1 ships for Windows only and has no self-updater. Users install a new version by running the new installer or replacing the portable build.

## Cutting a release

1. Bump `version` in `[workspace.package]` and commit.
2. Tag the commit `vX.Y.Z` (`vX.Y.Z-beta.1` for a prerelease) and push the tag.
3. `.github/workflows/release.yml` checks the tag against the crate version, builds the Windows installer and portable zip, and creates the GitHub release with the commit subjects since the previous tag as notes (`cargo xtask release notes`).

## Asset names

`kyoko-<version>-windows-x86_64-<kind>.<ext>` where kind is `installer` (`exe`) or `portable` (`zip`).
