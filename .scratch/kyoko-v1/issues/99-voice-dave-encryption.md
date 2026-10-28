# 99 — DAVE end-to-end encryption

**What to build:** Port `dave/*` and wire it into 97's transform for Opus. Mockcord gains a DAVE server side from `dave::testing` and remote members that encrypt.

Plan ticket V3 of `nguyenvu/.research/voice-plan/plan.md` (read its sections 1–5 and 7 for the facts and the port list). Supersedes 78.

**Blocked by:** 97

**Status:** claimed

- [ ] With `dave_protocol_version: 1`, kyoko sends its key package, applies the welcome or commit, and sends transition-ready; the frames it sends carry the 0xFAFA marker and decrypt for the scripted member
- [ ] A member joining and leaving moves the epoch; media continues across the transition; an invalid commit sends op 31 and recovers
- [ ] Interop: a `davey` member (dev-dependency) and a kyoko member in one group decrypt each other's Opus frames
- [ ] Downgrade to 0 (op 21) and upgrade (op 24) handled; Opus silence frames pass through unencrypted
- [ ] The MLS and DAVE crypto runs on BoringSSL (see "Crypto provider" below): `cargo tree -p kyoko -e normal -i crypto-bigint` finds nothing, and `deny.toml`'s RUSTSEC-2026-0387 ignore names only xtask's str0m capture (or is gone)

## Crypto provider (user decision, 2026-10-28)

Do it the way Discord does. Discord's libdave runs MLS on mlspp over OpenSSL or BoringSSL; oxidcord, davey (songbird, discord.js) use `openmls_rust_crypto`, whose `p256`/`p384` 0.13 pull `crypto-bigint` 0.5 (RUSTSEC-2026-0387, non-constant-time arithmetic, no fixed 0.5 release). The terminal clients avoid it too (endcord wraps libdave; discordo is pure Go). So kyoko keeps oxidcord's DAVE protocol code but gives openmls its own crypto provider on BoringSSL through `btls` (already linked for TLS), not `openmls_rust_crypto`, `openmls_basic_credential`'s RustCrypto signer or `hpke-rs-rust-crypto`.

- DAVE v1 uses MLS ciphersuite `MLS_128_DHKEMP256_AES128GCM_SHA256_P256`: the provider needs P-256 ECDSA sign and verify, HPKE with DHKEM(P-256, HKDF-SHA256) and AES-128-GCM, HKDF-SHA256, SHA-256, HMAC and a CSPRNG, all from BoringSSL. Other ciphersuites may return `UnsupportedCiphersuite`.
- If `btls` doesn't expose BoringSSL's HPKE P-256 KEM, build RFC 9180 HPKE from BoringSSL's ECDH, HKDF and AES-GCM primitives, and check it against the RFC 9180 test vectors for that suite.
- The DAVE frame cipher (AES-128-GCM with the truncated tag) also uses BoringSSL.
- The `davey` interop test (dev-dependency only) proves the provider talks to the RustCrypto stack; davey may keep its own crypto since it never ships.
- Add the provider to `docs/fingerprint.md` ("Voice") and update the deny.toml reason.
