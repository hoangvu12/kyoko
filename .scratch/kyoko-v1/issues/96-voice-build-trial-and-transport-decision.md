# 96 — Voice build trial and transport decision

**What to build:**
- Prove the voice dependencies build in kyoko on Windows: libopus via CMake under kyoko's toolchain file, openmls, MF and WGC `windows` features, and openh264 behind a feature. Measure them.
- Record what Chrome does for WebRTC on localhost only: a local page that creates `RTCPeerConnection({bundlePolicy:"max-bundle"})` with recvonly audio and video transceivers, records its offer SDP (codecs, payload types, extmaps), and its DTLS ClientHello against a local str0m peer.
- Write up A vs B vs C for the user's decision. No production code changes; nothing contacts Discord or discord.media.

Plan ticket V0 of `nguyenvu/.research/voice-plan/plan.md` (read its sections 1–5 and 7 for the facts and the port list). Supersedes 78.

**Blocked by:** none

**Status:** claimed

- [ ] Scratch `crates/voice` with oxidcord's deps compiles; libopus builds with `HOST_CMAKE_TOOLCHAIN_FILE` set (SSE kept, `/MD`), or a fix is written down
- [ ] `cargo deny check` passes; `cargo tree -p kyoko -e normal` shows no rustls, aws-lc-sys or tokio-tungstenite TLS features
- [ ] Measured: first build and no-op rebuild of opusic-sys, openmls and openh264-sys2 (dev and release), sccache behaviour, and release `kyoko.exe` size with and without openh264
- [ ] `xtask` capture records Chrome 156's offer SDP and DTLS ClientHello on localhost; fixture committed; a str0m (rust-crypto or wincrypto) hello compared against it
- [ ] Decision recorded in `docs/fingerprint.md` ("Voice"): transport A, B or C, and openh264 in or out
