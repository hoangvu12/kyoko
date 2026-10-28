# 110 — Media queue burst test sometimes sees two connections

**What to build:** `media_queue::a_burst_of_images_to_one_host_shares_one_http2_connection` (`crates/bridge/tests/bridge/media_queue.rs:195`, from 93) failed 1 of 5 runs alone on main `be78bfb` and once in a full workspace run: "one TLS connection for the whole burst, as Chrome opens one per origin; left: 2, right: 1". Find out whether the second connection is kyoko's (a race in `crates/media/src/queue.rs`'s unknown-protocol gate, e.g. a second request slipping through before the first response's headers, a retry, or wreq's pool opening another connection) or someone else's (109 found another process on this machine probing newly opened localhost ports with `GET /`; a plaintext probe fails the TLS handshake, but a TLS-capable prober would count). Fix the cause: a real race is fixed in the queue; a stray connection is excluded by counting only connections that carried one of the test's own image requests.

**Blocked by:** none

**Status:** claimed

- [ ] Root cause found and written down (record which connections the listener saw: their requests, SNI, ALPN)
- [ ] The test passes 30 runs in a row alone and 10 during a full workspace build, and still fails with the per-host gate disabled
- [ ] If kyoko itself opened the second connection, `tls-capture --client wreq --h2 --compare chrome-156` stays 0 and 0 after the fix
