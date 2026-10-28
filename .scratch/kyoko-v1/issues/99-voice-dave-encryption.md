# 99 — DAVE end-to-end encryption

**What to build:** Port `dave/*` and wire it into 97's transform for Opus. Mockcord gains a DAVE server side from `dave::testing` and remote members that encrypt.

Plan ticket V3 of `nguyenvu/.research/voice-plan/plan.md` (read its sections 1–5 and 7 for the facts and the port list). Supersedes 78.

**Blocked by:** 97

**Status:** resolved

- [x] With `dave_protocol_version: 1`, kyoko sends its key package, applies the welcome or commit, and sends transition-ready; the frames it sends carry the 0xFAFA marker and decrypt for the scripted member
- [x] A member joining and leaving moves the epoch; media continues across the transition; an invalid commit sends op 31 and recovers
- [x] Interop: a `davey` member (dev-dependency) and a kyoko member in one group decrypt each other's Opus frames
- [x] Downgrade to 0 (op 21) and upgrade (op 24) handled; Opus silence frames pass through unencrypted
- [x] The MLS and DAVE crypto runs on BoringSSL (see "Crypto provider" below): `cargo tree -p kyoko -e normal -i crypto-bigint` finds nothing, and `deny.toml`'s RUSTSEC-2026-0387 ignore names only xtask's str0m capture (or is gone)

## Crypto provider (user decision, 2026-10-28)

Do it the way Discord does. Discord's libdave runs MLS on mlspp over OpenSSL or BoringSSL; oxidcord, davey (songbird, discord.js) use `openmls_rust_crypto`, whose `p256`/`p384` 0.13 pull `crypto-bigint` 0.5 (RUSTSEC-2026-0387, non-constant-time arithmetic, no fixed 0.5 release). The terminal clients avoid it too (endcord wraps libdave; discordo is pure Go). So kyoko keeps oxidcord's DAVE protocol code but gives openmls its own crypto provider on BoringSSL through `btls` (already linked for TLS), not `openmls_rust_crypto`, `openmls_basic_credential`'s RustCrypto signer or `hpke-rs-rust-crypto`.

- DAVE v1 uses MLS ciphersuite `MLS_128_DHKEMP256_AES128GCM_SHA256_P256`: the provider needs P-256 ECDSA sign and verify, HPKE with DHKEM(P-256, HKDF-SHA256) and AES-128-GCM, HKDF-SHA256, SHA-256, HMAC and a CSPRNG, all from BoringSSL. Other ciphersuites may return `UnsupportedCiphersuite`.
- If `btls` doesn't expose BoringSSL's HPKE P-256 KEM, build RFC 9180 HPKE from BoringSSL's ECDH, HKDF and AES-GCM primitives, and check it against the RFC 9180 test vectors for that suite.
- The DAVE frame cipher (AES-128-GCM with the truncated tag) also uses BoringSSL.
- The `davey` interop test (dev-dependency only) proves the provider talks to the RustCrypto stack; davey may keep its own crypto since it never ships.
- Add the provider to `docs/fingerprint.md` ("Voice") and update the deny.toml reason.

## Answer

Merged to main (worker `817d8af8`, lane B; "Run DAVE's MLS on a BoringSSL provider" and "Encrypt voice end to end with DAVE"; full verify re-run by the orchestrator: all green; `tls-capture --compare chrome-156` 0 and 0; `cargo tree -p kyoko -e normal -i crypto-bigint` matches no package). Docs: `docs/voice.md`, `docs/fingerprint.md` ("Voice"), `THIRD_PARTY_NOTICES.md` (DAVE section: openmls, tls_codec).

- Root cause: kyoko advertised DAVE v1 in identify but ignored `dave_protocol_version` and the DAVE opcodes and had no MLS; oxidcord's MLS sits on RustCrypto, which pulls `crypto-bigint` 0.5 (RUSTSEC-2026-0387).
- New headless crate `crates/dave` (`kyoko-dave`): oxidcord's `dave/*` port plus kyoko's openmls crypto provider on BoringSSL. A `testing` feature adds a mock DAVE gateway, scripted members and a harness.
- btls coverage at rev `1298875`: AEAD with any tag length (16-byte MLS, 8-byte DAVE frame), HMAC, SHA-256, EC keys and points, ECDSA (DER), `rand_bytes` are in the safe API. HPKE is not usable there (`HpkeKey::dhkem_p256_sha256` initialises X25519; no seal, open or export) and HKDF is missing, so `crates/dave/src/hpke.rs` and the provider call BoringSSL's `EVP_hpke_p256_hkdf_sha256`, `EVP_HPKE_*` and `HKDF_extract`/`HKDF_expand` through btls-sys. Checked against RFC 9180 A.3.1 (base mode).
- Voice core: each `VoiceConnection` runs its own DAVE session (opcodes 21-31, binary frames with the 2-byte seq); Opus sealed and opened through the frame transform; silence frames pass unencrypted. `VoiceEvent::Dave`/`DaveFailure`, `VoiceConnection::dave()` counters; only epoch and member count are logged. `VoiceOptions.transform` and `Passthrough` are removed: DAVE's handshake rides the voice websocket, so DAVE is built into every connection.
- Mockcord: `set_dave_version(1)` (default 0, so `--mock` and echo tests are unchanged), a DAVE room per channel with scripted members; controls `add_member`, `remove_member`, `send_as`, `downgrade_dave`, `upgrade_dave`, `send_bogus_commit`. Mockcord depends on `dave` with `testing`.
- Tests: `dave::kyoko_joins_the_dave_group_and_its_opus_frames_decrypt_for_the_member`, `voice::a_joined_call_is_end_to_end_encrypted_in_each_channels_dave_group` (session seam, including a channel move), epoch moves 1 -> 2 -> 3 with no gap over 500 ms, op 31 recovery, op 21/24, `crates/dave/tests/interop.rs` with davey 0.1.5 (dev-dependency) both directions.
- deny.toml: the RUSTSEC-2026-0387 ignore is gone (str0m no longer brings crypto-bigint); `davey` is excluded from cargo-deny's graph (`[graph] exclude`) since it is test-only and carries 0387, 0207, 0208, 0212.
- For later: the group id is the channel id; Go Live (106) needs `rtc_server_id - 1` passed into `DaveSession::new`. Video (103/104) must port `dave/codec.rs` (H.264 unencrypted ranges, packetizability retry) and add `Codec::H264` to `dave::Codec` and `voice::Codec`. `--mock` still answers DAVE 0; a later ticket can turn it on. 102 should check the web client shows the call as end-to-end encrypted (`VoiceEvent::Dave` gives epoch and members). libopus (BSD-3) is still missing from THIRD_PARTY_NOTICES (leftover from 97).
