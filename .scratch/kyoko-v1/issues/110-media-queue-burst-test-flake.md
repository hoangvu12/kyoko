# 110 — Media queue burst test sometimes sees two connections

**What to build:** `media_queue::a_burst_of_images_to_one_host_shares_one_http2_connection` (`crates/bridge/tests/bridge/media_queue.rs:195`, from 93) failed 1 of 5 runs alone on main `be78bfb` and once in a full workspace run: "one TLS connection for the whole burst, as Chrome opens one per origin; left: 2, right: 1". Find out whether the second connection is kyoko's (a race in `crates/media/src/queue.rs`'s unknown-protocol gate, e.g. a second request slipping through before the first response's headers, a retry, or wreq's pool opening another connection) or someone else's (109 found another process on this machine probing newly opened localhost ports with `GET /`; a plaintext probe fails the TLS handshake, but a TLS-capable prober would count). Fix the cause: a real race is fixed in the queue; a stray connection is excluded by counting only connections that carried one of the test's own image requests.

**Blocked by:** none

**Status:** resolved

- [x] Root cause found and written down (record which connections the listener saw: their requests, SNI, ALPN)
- [x] The test passes 30 runs in a row alone and 10 during a full workspace build, and still fails with the per-host gate disabled
- [x] If kyoko itself opened the second connection, `tls-capture --client wreq --h2 --compare chrome-156` stays 0 and 0 after the fix

## Answer

Merged to main as `6ea0754` (worker `2d49562b`, lane C; full verify re-run by the orchestrator: green apart from the known `favourites::...frecency...` flake in `xtask test`, which passed in the same run's `test --workspace`). Test code and `docs/testing.md` only; `crates/media/src/queue.rs` unchanged.

- Root cause: not kyoko. Reproduced 8 of 30; every failure's extra connection was a plaintext `HEAD / HTTP/1.1` (`Host: localhost:<port>`, `Connection: close`, no TLS hello, no SNI or ALPN) 35-440 ms after the port opened, from `roboco.exe` (the Roboco desktop app, `AppData\Local\Programs\Roboco`), the same prober 109 found (then with `GET`). Kyoko's own connection was always exactly one (SNI `localhost`, ALPN `h2`, all 10 image requests).
- Fix: the test counts only connections that opened with a TLS ClientHello. 30/30 alone, 10/10 during a workspace clippy build; with the per-host gate off it fails 5/5 (10 connections).
- Deviation: counting only connections that carried an image request would hide the regression (with the gate off, wreq's extra cold handshakes stay idle once one negotiates h2), so the filter is "sent a TLS hello" instead.
- For later: tests counting connections on `HelloListener` should count only `hello.is_some()` or their own paths. A TLS-capable prober would get through this filter.
