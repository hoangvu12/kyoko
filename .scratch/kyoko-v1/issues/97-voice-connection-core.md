# 97 — Voice connection core on mockcord (audio, DAVE off)

**What to build:** Port `crates/voice` without video. Signalling on `net::ws` (v9, web heartbeat, `rtc_connection_id`, two simulcast layers in identify), UDP transport with both rtpsize modes, Opus via libopus, jitter, mixer, resample and gate. Mockcord's voice server moves onto TLS and records frames.

Plan ticket V1 of `nguyenvu/.research/voice-plan/plan.md` (read its sections 1–5 and 7 for the facts and the port list). Supersedes 78.

**Blocked by:** 96

**Status:** claimed

- [ ] The voice upgrade presents `UPGRADE_ORDER` and the Chrome wss ClientHello (same check as the gateway's, `tls_fingerprint`); no Cookie
- [ ] Identify is v9 with `channel_id`, `video: true`, layers 100 and 50, `max_dave_protocol_version: 1`; heartbeat every `min(interval, 5000)` ms; resume carries `seq_ack` and `channel_id`
- [ ] Select protocol picks AES-GCM when offered and XChaCha otherwise; both seal and open against mockcord's echo
- [ ] A 1 kHz tone fed to the manual input comes back decoded from the echo SSRC; 5 silence frames follow the end of speech; op 5 is sent before media
- [ ] Close 4006/4009 starts fresh, 4015 and network loss resume, 4014/4017 stop with a reason
