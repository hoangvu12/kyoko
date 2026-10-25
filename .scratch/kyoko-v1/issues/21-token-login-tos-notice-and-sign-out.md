# 21 — Token login, ToS notice and sign-out

**What to build:** A user can sign in by pasting a token on a login screen identical to oxidcord's, which includes the Terms of Service notice, and can sign out cleanly.

**Blocked by:** 20 (Theme, fonts, icons and app frame), 15 (Store hardening)

**Status:** resolved

- [x] The login screen and its ToS notice match oxidcord in the parity captures
- [x] A pasted token is validated and stored in Credential Manager, and the app moves on to the main view
- [x] A 401 at any time returns to this screen
- [x] Signing out wipes the account's data and returns here

## Answer

Merged to main as `0079201..06b44a4`. Docs: `docs/sign-in.md`, `docs/ui.md`, `docs/parity.md`.

- `crates/bridge/src/sign_in.rs` (no UI dependency): `Gate` validates through `login::TokenLogin::validate` with the launch's one `ClientIdentity` and records the ToS acknowledgement in `settings.first_run_acknowledged`; `Flow` owns the `Session` and builds new ones through a `StartSession` closure. Screens: `Login(Ready | Checking | Failed | Unsaved | Expired | SignedOut | Unavailable)`, `Main`, `SigningOut`. Every sign-in is a new `Session`, so safety locks are re-read and risky confirmations cleared (11).
- Launch: a developer token (`KYOKO_TOKEN`, or mockcord's on `--mock`) starts a session that never stores the token; otherwise the saved current account; otherwise the login screen. `--login` starts mock runs at the login screen. Real runs without `--data-dir` now use the real data directory (`Paths::discover()`).
- Errors: rejected token stores nothing; network → "Could not reach Discord."; captcha/429/suspended → new `LoginError::Refused`, never retried (`max_retry_wait` zero); keyring failure → "Continue for this session only" (`remember_account: false`, nothing in plain text). Signing in a different account wipes the shared cache first.
- 401 or close 4004 → `Flow::observe` sees `Disconnected(AuthenticationFailed)` → "Your session expired". Sign-out: `Command::Logout` via the `login.sign_out` action (right-click the user panel → Log Out), gallery budget to 0, wait for the wipe, login screen. The timeline slot is collapsed whenever the window leaves the main view. `RailView::reset()` (in `rail.rs`) clears 22's rail on session change and reloads open folders from the store.
- Window side: `apps/desktop/src/ui/sign_in.rs`; Luau `ui/shell/login.luau` beside the shell in `frame.luau`; `session.storage_warning` bar (`storage_notice.luau`, off under mock).
- 8 seam tests in `crates/bridge/tests/bridge/sign_in.rs`. Parity: login-tos 2.3%, login-token 2.4% (QR column says "coming soon"), server-rail 10.2% (other panels).
- For 30: replace the panel's right-click Log Out with the real menu entry, keep `login.sign_out`. 37: Log Out button calls the same action. 41: tokens are keyed per account; the cache is one shared database (switching wipes it); per-account databases plus a `Flow::switch` fit the shape. 42/43: replace the "coming soon" text in `qr_column()` and the Web login handler in `login.luau`, hand tokens to `Gate::check` → `Flow::checked`.
