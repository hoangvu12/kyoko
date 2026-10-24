# 01 — Rebrand fork to kyoko

**What to build:** kyoko stops presenting itself as oxidcord anywhere a user, the OS or a network peer can see it. The fork credits oxidcord and keeps its licences. Contributor rules and CI reflect kyoko's decisions.

**Blocked by:** None — can start immediately

**Status:** resolved

- [x] Crate packages, binary, app id, data and config directories, env prefixes, window titles and icon all say kyoko. oxidcord's name and icon are no longer used
- [x] oxidcord's update-signing key, plugin-catalog signing key and GitHub update/catalog endpoints are removed or replaced
- [x] LICENSE-MIT, LICENSE-APACHE and NOTICE are kept with oxidcord's copyright line. The README has a credit section that links oxidcord and explains what kyoko changes
- [x] The emoji table's origin is identified and attributed, or the table is replaced
- [x] Contributor rules require behaviour tests at the session seam, replacing the "no unit tests" rule
- [x] CI runs on Windows only: build, fmt, and clippy with warnings denied
- [x] The headless mock run passes as before

## Answer

Merged to main as `0eec701..62d72c4` (four commits, ending at "Rename oxidcord identity to kyoko").

- App id `io.github.hoangvu12.Kyoko` (AUMID, single instance, NSIS). Data in `%APPDATA%\hoangvu12\Kyoko\{data,config}`, cache in `%LOCALAPPDATA%\hoangvu12\Kyoko\cache`. Binary `kyoko`, packages `kyoko-*`, env prefix `KYOKO_*`, keyring service `kyoko`, store canary `b"kyoko"`.
- Icon `assets/icon.{png,ico,svg}` is a generated placeholder (white K on indigo). Replace with real artwork before release (58).
- Upstream keys are gone: `UPDATE_PUBLIC_KEY` and `CATALOG_PUBLIC_KEY` are `None`, verification fails closed and nothing is fetched. Default catalog URLs are empty. `packs/`, `plugins/catalog.json(.sig)` and `themes/catalog.json(.sig)` were deleted because they were signed with upstream's key. Ticket 57 adds kyoko's own key and a new catalog; no key pair has been generated yet.
- Emoji table is from github/gemoji (all 1,870 entries, tags identical in order) plus Discord-style flag shortcodes and 53 newer emoji of unconfirmed source. `THIRD_PARTY_NOTICES.md` carries gemoji's MIT licence; NOTICE and README point to it.
- For 02: CI still installs `wasm32-wasip2` and headless-run still builds the two wasm plugins; drop both, plus the `wasm32-wasip2` entry in `rust-toolchain.toml`. `release.yml` macOS/Linux jobs and `UPDATE_SIGNING_KEY` must go. `cargo deny` (kept as a Windows job) fails until 02 removes `ui` (gpui-fast git sources) and `voice` (RUSTSEC-2026-0387 via openmls/crypto-bigint); a yanked `clipboard-win` warning in `platform` remains.
