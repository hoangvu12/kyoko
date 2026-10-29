# 122 — Provider embeds and players (from oxidcord fc1dc43)

**What to build:** oxidcord `fc1dc43` adds provider embeds (YouTube and similar players), a web frame (`webframe.rs`), an inline audio/video player and a thread inbox: 86 files, +7105 lines. Too big to port blind: needs triage against kyoko (91 already plays inline video; a web frame means embedding a browser engine, a spec-level choice). Ask the user before building.

**Status:** needs-triage