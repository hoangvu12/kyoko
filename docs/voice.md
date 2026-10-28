# Voice

`crates/voice` is the voice connection core, ported from oxidcord's `crates/voice` (MIT/Apache) without video. It has no UI dependency. The session drives it (below, "Joining"); DAVE end-to-end encryption comes from `crates/dave` (below, "DAVE"). The transport decision (UDP) is in `docs/fingerprint.md`, "Voice".

## Shape

- `VoiceConnection::connect(VoiceParams, VoiceOptions)` runs the handshake and returns once the session description has arrived and media is running. `VoiceParams` holds the guild (or none for DMs, where `server_id` is the channel), the channel, the user, the main gateway's session id, the voice token and the endpoint from VOICE_SERVER_UPDATE. `VoiceOptions` holds the audio mode, the TLS `Trust`, and the user agent and locale for the upgrade. Every connection has its own `DaveSession`; its frame crypto sits in the media path behind the `FrameTransform` trait, and the supervisor feeds it the DAVE opcodes.
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

## DAVE

`crates/dave` (`kyoko-dave`, headless) is oxidcord's `dave/*` port: the MLS session, the sender key ratchet, the frame cryptor and the frame format, for DAVE protocol version 1 with MLS ciphersuite `MLS_128_DHKEMP256_AES128GCM_SHA256_P256`. Facts from docs.discord.food voice connections ("DAVE") and the DAVE whitepaper (daveprotocol.com), read 2026-10-28.

- Identify sends `max_dave_protocol_version: 1`. The session description's `dave_protocol_version` starts the session: with 1 the client creates its MLS signing key and sends its key package (op 26, binary); with 0 it sends and accepts plain Opus.
- Binary frames from the server carry a 2-byte big-endian `seq` before the opcode (it counts towards `seq_ack` like the JSON `seq`); binary frames from the client are the opcode and the payload only.
- Op 25 (external sender) creates the pending group with the external-senders extension. Op 27 (proposals, append or revoke) queues only Add and Remove proposals from the external sender, for users the call has announced (op 11, op 5), and commits them (op 28: the commit, then the welcome when someone is added). Op 29 (announced commit) and op 30 (welcome) move the group to the next epoch; the welcome must be for this channel's group, with the same external sender and only announced users.
- After a commit or welcome, each member's 16-byte media key base is the MLS exporter secret `"Discord Secure Frames v0"` with the member's user id as little-endian u64 context. The keys install at once for receiving, and the client sends op 23 (transition ready). Op 22 (execute transition) switches sending to the new key; transition id 0 applies at once. Old receive keys keep working for 10 seconds after a transition.
- A commit or welcome that cannot be processed sends op 31 (invalid commit or welcome) with its transition id, resets the group and sends a fresh key package, so the server can remove and re-add the member.
- Op 21 with version 0 (downgrade) lets plain frames in and, once executed, sends plain Opus; op 24 (prepare epoch) with epoch 1 starts a fresh group (upgrade).
- Frames: AES-128-GCM with an 8-byte tag, a 32-bit nonce (top 8 bits are the key generation) in a 12-byte IV, the tag, the ULEB128 nonce, the supplemental size and the `0xFAFA` marker appended. Opus is encrypted whole. The Opus silence frame `f8 ff fe` passes unencrypted both ways.
- `VoiceEvent::Dave { epoch, version, members }` follows each applied transition and `VoiceEvent::DaveFailure(reason)` each failure; `VoiceConnection::dave()` gives the counters (encrypted, decrypted, rejected, passed silence frames) and whether sending is encrypted. The session logs the epoch and member count at debug level. Secret keys, exporter secrets and MLS state are never logged.
- The MLS provider is kyoko's own, on BoringSSL (`docs/fingerprint.md`, "Voice", "DAVE crypto provider").

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

DAVE on mockcord: `VoiceControl::set_dave_version(1)` makes the session description announce DAVE v1 (the default is 0, so the echo tests and `--mock` stay plain). Each channel then has a DAVE room built from `dave::testing` (`Harness`): a `Gateway` that acts as the external sender (op 25, add and remove proposals, announcing the first matching commit, welcomes, transitions), the connected clients as external members whose DAVE frames go out over their sockets with the session `seq`, and scripted members that run kyoko's own `DaveSession` in process. `add_member(channel, user)` and `remove_member` add and remove a scripted member, `send_as(channel, user, ssrc, sequence, opus)` sends that member's DAVE-encrypted Opus over UDP to every client in the channel (pair it with `speak_as` so the client maps the SSRC), `downgrade_dave` and `upgrade_dave` send op 21 with version 0 and op 24 for epoch 1, and `send_bogus_commit(channel, user)` sends an op 29 that cannot be processed. Every RTP packet a client sends is also offered to the room's members, and `VoicePacket::opened` holds the frame one of them decrypted. Binary frames are recorded too (`VoiceFrame::is_binary`, `payload`).

Tests: `crates/voice/tests/voice` (`cargo test -p kyoko-voice --test voice`) for the connection core and DAVE (`dave.rs`: joining the group, epochs moving with members, the invalid commit, downgrade and upgrade, silence frames), `crates/dave/tests/interop.rs` (`cargo test -p kyoko-dave --features testing`) for interop with `davey` both ways and, in `crates/dave/src/hpke.rs`, the RFC 9180 test vectors, and `crates/session/tests/session/voice.rs` for joining, moving, leaving, mute and deafen, server mute, the full-channel timeout, the gateway re-identify and a DAVE call in each channel's group.
