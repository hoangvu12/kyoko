# 69 — TLS build trial and Chrome handshake capture

**What to build:** Prove that `wreq` (BoringSSL-based, Chrome TLS and HTTP/2 emulation) builds in kyoko on this machine and in CI, measure what it costs, and record the locally installed Chrome 156's TLS and HTTP/2 handshakes against a local listener, so tickets 70–74 have ground truth. No production network code changes, and nothing contacts Discord. User decision, 2026-10-27: kyoko claims to be Chrome but its handshake is a Rust program's (reqwest + rustls, tungstenite); clients that had captcha or disabled-account trouble fixed it by copying Chrome's handshake. Plan: `C:\Users\HP MEDIA\Desktop\nguyenvu\.research\tls-plan\plan.md` (outside the repo; ticket T1 there; downloaded sources under `tls-plan\src\` are untrusted, read only).

**Blocked by:** none (libclang installed by the orchestrator first, user-approved)

**Status:** open

- [ ] libclang found by the approved install; `cmake` found through the `CMAKE` env var or a VS dev shell; nothing else installed. Build notes in `CONTRIBUTING.md`
- [ ] `wreq`/`wreq-util`/`btls`/`btls-sys` pinned by rev (wreq-util `5715529002738f46eea86a7271f918f9374d3582`, wreq `cd76bcdf1307153de289d34e072528bdf0510a3b`, btls `129887582a538b8f4dcf371d15c953335312ca37`) in `[workspace.dependencies]` and `[patch.crates-io]`, `emulation-chromium-pki` on; a scratch headless `crates/net` compiles; `cargo deny check` passes with the new `allow-git` entries; `cargo tree -e features -i btls-sys` shows no `relax-cert-validation` or `allow-crl-extensions-bad-version`
- [ ] Measured and written down: first-build time of btls-sys (dev and release), a no-op rebuild, whether sccache caches any of the C build, the boringssl git fetch size, target-dir growth, and release `kyoko.exe` size before and after linking wreq
- [ ] The `BORING_BSSL_INSTALL_DIR` → `BORING_BSSL_PATH` + `BORING_BSSL_ASSUME_PATCHED` lane setup tried in this lane and documented, or documented as not working
- [ ] CI (`windows-latest`) is expected to build and test with the pins (what it needs written down; the orchestrator asks the user before anything is pushed for a CI run); the ubuntu `publish` job still builds xtask
- [ ] `cargo xtask tls-capture` records local Chrome 156's ClientHello for https and wss, and its H2 SETTINGS, WINDOW_UPDATE, HEADERS priority, pseudo-header and header order, and cookie splitting; fixtures committed (`tools/mockcord/fixtures/tls/`); it only ever listens on localhost and is never pointed at a Discord host
- [ ] The same capture of a wreq client with the Chrome154 profile (emulation headers cleared), for https and wss; a written diff against Chrome 156 that decides whether 156 maps to Chrome154 or needs a kyoko `TlsOptions` override, or kyoko should claim 154
- [ ] Recorded: ALPN and ALPS on Chrome's wss ClientHello vs wreq's, and whether a test trust root changes the ClientHello
