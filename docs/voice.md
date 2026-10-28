# Voice

`crates/voice` is the voice connection core, ported from oxidcord's `crates/voice` (MIT/Apache) without video and without DAVE. It has no UI dependency. The session drives it (below, "Joining"); DAVE is V3. The transport decision (UDP) is in `docs/fingerprint.md`, "Voice".

## Shape

- `VoiceConnection::connect(VoiceParams, VoiceOptions)` runs the handshake and returns once the session description has arrived and media is running. `VoiceParams` holds the guild (or none for DMs, where `server_id` is the channel), the channel, the user, the main gateway's session id, the voice token and the endpoint from VOICE_SERVER_UPDATE. `VoiceOptions` holds the audio mode, the TLS `Trust`, the user agent and locale for the upgrade, and a `FrameTransform` (the seam DAVE plugs into; `Passthrough` today).
- `signal.rs`: the voice websocket on `net::ws`, the handshake (identify, ready, IP discovery, select protocol, session description, op 5, op 15) and resume.
- `supervisor.rs`: the running connection. It reads frames, forwards op 5 for the media task, and decides what a close means (below).
- `transport.rs`: the `Transport` trait (send a sealed RTP packet, receive and open one) and its only implementation, UDP with IP discovery. A WebRTC transport would implement the same trait.
- `media.rs`: the 10 ms media loop. Capture is resampled to 48 kHz mono, gated (voice activity at -50 dB with a 15-frame hangover, or push to talk), optionally denoised with RNNoise (off by default), encoded with libopus (64 kbps, FEC, DTX) and sent. Receive goes through a per-SSRC jitter buffer and Opus decoder into the mixer, then to the device or the manual output.
- `audio.rs`: cpal devices on their own thread. `AudioIo::Manual` skips devices: `feed_input` queues 20 ms frames and `take_output` returns mixed frames with the SSRCs that made them. Tests use only the manual path.

## Wire

Facts from docs.discord.food voice connections and close codes (userdoccers master, read 2026-10-28):

- `v=9`. Identify keys in the docs' order: `server_id`, `channel_id`, `user_id`, `session_id`, `token`, `video: true`, `streams` (two video layers, `rid` 100 and 50), `max_dave_protocol_version: 1`. Resume: `server_id`, `channel_id`, `session_id`, `token`, `seq_ack`.
- Heartbeat every `min(heartbeat_interval, 5000)` ms, as the web client does, first beat one period after Hello, `{"t": unix ms, "seq_ack": last seq or -1}`. A server op 3 is answered at once. A missed ack counts as a lost connection.
- Select protocol: `protocol: "udp"`, `data` (`address`, `port`, `mode`), `codecs` (opus 120, H264 101/102), a fresh UUIDv4 `rtc_connection_id`, and Ready's `experiments` echoed back. AES-GCM rtpsize is chosen when offered, XChaCha20-Poly1305 rtpsize otherwise.
- One op 5 (`speaking: 0`) goes out right after the session description, before any media. Speech sends op 5 with flag 1 (VOICE); the end of speech sends five Opus silence frames (`f8 ff fe`) and then op 5 with 0.
- Upgrade headers are the gateway's: User-Agent, `Origin: https://discord.com`, Accept-Language, through `net::ws` with `UPGRADE_ORDER`. Each connection has its own wreq client with no cookie store, so no Cookie is sent.
- Never logged: the voice token, the secret key, raw payloads.

## Closes

| Close | What happens |
|---|---|
| none (network loss), 4015, and codes below 4000 | resume with backoff 1, 2, 4, 8, 8 s; a resume closed with 4006/4009 falls through to a fresh connect |
| 4006, 4009 | a fresh identify (new SSRC, new UDP socket, media restarted), up to 3 attempts |
| every other 4xxx | stop: `VoiceEvent::Stopped(reason)`, where 4014 is `Disconnected`, 4017 `E2eeRequired`, 4022 `CallEnded`, 4021 `RateLimited`, 4004 `AuthenticationFailed`, 4011 `ServerNotFound`, others `Closed(code)` |

Joins and rejoins only ever come from the caller; the core never re-joins after a stop.

## Joining

`crates/session/src/task/voice_link.rs` (ported from oxidcord's `voice_link.rs`) joins, moves and leaves server voice channels. DM and group calls are V12.

- `Command::JoinVoice { guild_id, channel_id }` checks that the channel is a voice or stage channel of that server (anything else is rejected, so a voice row can never start a DM call), sends op 4 and waits. VOICE_STATE_UPDATE for the current user gives the gateway `session_id`, VOICE_SERVER_UPDATE the voice `token` and `endpoint`; once both are in, `VoiceConnection::connect` runs with the identity's user agent and locale. Joining another channel drops the connection and sends op 4 again; the new VOICE_SERVER_UPDATE brings a new token, so the move connects afresh. A VOICE_SERVER_UPDATE while connected (a server change) reconnects too.
- `Command::LeaveVoice` disconnects and sends op 4 with the server's `guild_id` and `channel_id: null`.
- Op 4 keys, in the docs' order: `guild_id`, `channel_id`, `self_mute`, `self_deaf`, `self_video` (`false`), `preferred_region`, `preferred_regions`, `flags`. The regions are READY's `geo_ordered_rtc_regions` (the first is the preferred one), so nothing is fetched from discord.media. `flags` is 2 (`ALLOW_VOICE_RECORDING`); `CLIPS_ENABLED` is a desktop app feature and stays off.
- No answer within 15 seconds (a full channel gets no events) ends the join: op 4 with `channel_id: null` and a toast ("Couldn't connect to voice. The channel may be full."). A failed connection ends it the same way.
- A fresh gateway session (READY after an invalid session or a new identify) drops the voice connection and sends op 4 again for the same channel; Discord then sends a new state and server update and voice reconnects. A resumed gateway session changes nothing.
- A voice stop (`VoiceEvent::Stopped`) ends the call with a toast naming the reason; 4014 (removed from the channel) does not send op 4 again, since Discord already moved the user out. VOICE_STATE_UPDATE with `channel_id: null` for the current user ends the call the same way.
- Joins only ever come from a click; the session never rejoins on its own except to re-announce a channel after the gateway lost its session.

Mute and deafen (`Command::SetSelfMute`, `SetSelfDeaf`) work in and out of a call; `State::voice_self` keeps them between calls. Deafen implies mute: op 4 then carries `self_mute: true, self_deaf: true`, and undeafening restores the earlier mute. Unmuting while deafened undeafens too, as in Discord. In a call each change resends op 4 and is applied at once: the connection's mute stops sending (the voice gate is reset, so only the end-of-speech silence frames follow), deafen silences the mixer and implies mute. Server mute and deafen arrive in VOICE_STATE_UPDATE (`mute`, `deaf`); they are kept in the guild's voice states (`State::own_voice_state`) and also stop sending or playback.

`state.voice()` is the call: channel, server, `VoiceStatus` (Connecting, Connected, Reconnecting) and the self flags. `Session::voice_connection()` hands out the running `VoiceConnection` (tests feed and read audio through it), and `Session::set_voice_setup` picks the audio mode (devices by default, `AudioIo::Manual` in tests), the TLS trust for the voice server and the join timeout.

## Mockcord

Mockcord's voice server runs on TLS when mockcord is built with `tls` (its own self-signed root, `Server::voice().root_pem()`), plain `ws://` otherwise. It records each socket's ClientHello, ALPN and raw upgrade request, every frame both ways (with `seq` on the frames it numbers), closes, and every RTP packet it decrypts. `Server::voice()` also offers `set_modes`, `set_heartbeat_interval`, `close_sockets(code)`, `drop_sockets()` (no close frame) and `speak_as(user, ssrc, speaking)` for a scripted member's op 5. The UDP side answers IP discovery and echoes each packet back from SSRC + 1 in the mode the session selected.

On the gateway side, op 4 answers with VOICE_STATE_UPDATE (broadcast) and, only when the session's channel changes, VOICE_SERVER_UPDATE with the token `mock-voice-token-<channel id>` (the voice server accepts `mock-voice-token` and any token made from it this way). `Control::fill_voice_channel(id)` makes op 4 for that channel go unanswered, as a full channel. READY carries `geo_ordered_rtc_regions` `mock-east`, `mock-central`, `mock-west`. `VoiceControl::hold_echo()` keeps echoed packets back until `release_echo()`, so a test can send while hearing and let the echo arrive later (while deafened, say).

Tests: `crates/voice/tests/voice` (`cargo test -p kyoko-voice --test voice`) for the connection core, `crates/session/tests/session/voice.rs` for joining, moving, leaving, mute and deafen, server mute, the full-channel timeout and the gateway re-identify.
