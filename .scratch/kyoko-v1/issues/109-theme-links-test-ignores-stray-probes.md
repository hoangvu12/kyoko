# 109 — Theme links test ignores stray local probes

**What to build:** `theme_links::theme_links_are_fetched_over_https_only_without_any_discord_credentials` (`crates/session/tests/session/theme_links.rs`, the `plain_served.paths().is_empty()` assertion near line 214) fails on this machine about half the time, alone or under load. A temporary diagnostic in ticket 84 showed why: the plain-http listener receives `GET /` with only `Host: localhost:<port>` and `Connection: close` from some other process that probes newly opened localhost ports (probably the user's Roboco test runs). Kyoko itself checks the scheme before any request and only ever asks for `/ocean.css`. Make the test prove what it means (kyoko never requests the plain http link) without being broken by requests it didn't cause, e.g. assert that no request for the theme's path (`/ocean.css`) reached the plain server, and apply the same rule to any other assertion in the file that counts every request a test listener receives.

**Blocked by:** none

**Status:** resolved

- [x] The test passes 20 runs in a row alone and 10 in a row during a full workspace build, on this machine
- [x] A deliberately broken scheme check (plain http fetched) still fails it
- [x] Any other session test that asserts "no request at all" on a freshly opened localhost listener gets the same treatment (grep for listeners asserting empty request lists)

## Answer

Merged to main as `b822ee1` and `dbfc7ed` (worker `d31072b9`, lane C; full verify re-run by the orchestrator: all green). Test code only; no session bug found.

- Root cause: another process on this machine probes newly opened localhost ports with `GET /`; the test counted every request its plain listener received. Now it asserts `/ocean.css` (`THEME_PATH`) never reached the plain listener, through a new `Served::asked_for` helper (also used on the secure side).
- `alerts::the_hook_hears_only_alerts_the_burst_limit_would_show` relied on fixed 300 ms sleeps covering a hook that sleeps 100 ms per call; it now waits for the hook's answers (3 vetoes before the rings, then 6 answers and 3 shown alerts) and keeps the settle only as a margin.
- Box 1: 20/20 alone and 10/10 while a workspace clippy build ran, for both tests. Box 2: kyoko has two guards (the scheme check in `check_link` and `https_only(true)` on the client); either alone keeps plain http from being fetched, so a test of them must break both. Box 3: no other listener asserts "no request at all" (mockcord records only `/api`, `/upload`, `/cdn`, `/media`; the media origin counts by path; the TLS listeners fail the handshake on a plaintext probe).
- New flake to watch: `kyoko-plugins` `one_ping_per_dm::repeated_direct_messages_ping_once_until_read` (step timeout in a harness wait, once under load; added to rules.md's list).
