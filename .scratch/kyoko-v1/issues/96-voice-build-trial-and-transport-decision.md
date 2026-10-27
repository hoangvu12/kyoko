# 96 — Voice build trial and transport decision

**What to build:**
- Prove the voice dependencies build in kyoko on Windows: libopus via CMake under kyoko's toolchain file, openmls, MF and WGC `windows` features, and openh264 behind a feature. Measure them.
- Record what Chrome does for WebRTC on localhost only: a local page that creates `RTCPeerConnection({bundlePolicy:"max-bundle"})` with recvonly audio and video transceivers, records its offer SDP (codecs, payload types, extmaps), and its DTLS ClientHello against a local str0m peer.
- Write up A vs B vs C for the user's decision. No production code changes; nothing contacts Discord or discord.media.

Plan ticket V0 of `nguyenvu/.research/voice-plan/plan.md` (read its sections 1–5 and 7 for the facts and the port list). Supersedes 78.

**Blocked by:** none

**Status:** resolved

- [x] Scratch `crates/voice` with oxidcord's deps compiles; libopus builds with `HOST_CMAKE_TOOLCHAIN_FILE` set (SSE kept, `/MD`), or a fix is written down
- [x] `cargo deny check` passes; `cargo tree -p kyoko -e normal` shows no rustls, aws-lc-sys or tokio-tungstenite TLS features
- [x] Measured: first build and no-op rebuild of opusic-sys, openmls and openh264-sys2 (dev and release), sccache behaviour, and release `kyoko.exe` size with and without openh264
- [x] `xtask` capture records Chrome 156's offer SDP and DTLS ClientHello on localhost; fixture committed; a str0m (rust-crypto or wincrypto) hello compared against it
- [x] Decision recorded in `docs/fingerprint.md` ("Voice"): transport A, B or C, and openh264 in or out

## Answer

Merged to main as `35042ea`, `b7af958`, `4ebdcdd` (worker `6197a52b`, lane B; checks re-run by the orchestrator: all green, `xtask test` rerun once after an sccache connection drop; `tls-capture --client wreq --h2 --compare chrome-156` 0 differences for https and wss; rebased onto 87, clean, rebuilt). Docs: `docs/fingerprint.md` ("Voice"), `docs/testing.md` (`webrtc-capture`), `docs/architecture.md` (scratch crate).

- **Decision (the user, 2026-10-28): UDP, option A.** "if others use udp then i dont know why we would use webrtc". UDP with IP discovery, `aead_aes256_gcm_rtpsize` (xchacha only when that is all the server offers) and DAVE, as oxidcord, the desktop app, discordo and endcord do. The mismatch with Chrome's WebRTC is a recorded known gap. A thin `Transport` seam stays; no WebRTC work planned. openh264 stays out; Media Foundation does H.264.
- Scratch `crates/voice` (nothing depends on it) builds and runs libopus 1.6.1, openmls 0.9.2 (two DAVE members agree a key), rubato, RNNoise, cpal, both AEAD modes, Media Foundation H.264 (3 hardware + 1 software encoder), Graphics Capture, openh264 behind an off-by-default feature. libopus builds under kyoko's toolchain file with `-MD` and SSE/AVX2 runtime detection.
- Cost (3 jobs, other lanes busy, about ±30%): cold trial dev 378 s / release 199 s; opusic-sys build script ~54 s; openh264-sys2 ~62 s; openmls 73 s dev / 17 s release; no-op ~0.9 s. sccache never caches libopus (VS generator ignores the launcher); warm dev 96 s. Release `kyoko.exe` 79.5 MB, +4.38 MiB with the trial, +0.69 MiB more with openh264.
- `cargo deny`: new ignore RUSTSEC-2026-0387 (crypto-bigint 0.5.5 via p256/p384 0.13 under openmls and str0m; no fixed 0.5). kyoko's normal tree has no rustls, aws-lc-sys or ring.
- `cargo xtask webrtc-capture` (Windows, 127.0.0.1 only): Chrome 156's offer and DTLS ClientHello (1,413 bytes, DTLS 1.3+1.2, X25519MLKEM768, two datagrams; `mockcord::hello::DtlsHelloReader`) in `tools/mockcord/fixtures/webrtc/`; str0m differs in 10-12 fields; btls supports DTLS+SRTP if WebRTC ever comes.
- For 97+: Media Foundation code must accept COM already initialised by cpal; `THIRD_PARTY_NOTICES.md` needs hpke-rs (MPL-2.0) and libopus (BSD-3) when voice ships; decide on RUSTSEC-2026-0387 before DAVE (99) ships (accept, or a BoringSSL openmls provider).
