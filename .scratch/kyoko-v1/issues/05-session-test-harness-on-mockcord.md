# 05 — Session test harness on mockcord

**What to build:** Contributors can write behaviour tests that drive a session against an in-process mockcord. A test checks both what the session publishes and what Discord would have seen on the wire, and can inject failures.

**Blocked by:** 02 (Strip GPUI UI, voice, WASM plugins and updater)

**Status:** claimed

- [ ] Mockcord records the gateway frames and REST requests it receives (method, route, headers, body, timing), and tests can query them
- [ ] Tests can inject 401, 403, 429 with retry-after, captcha errors, close codes, resume failures and late-arriving dispatches
- [ ] Tests run on seeded fixtures with controllable time, and are deterministic
- [ ] A first set of tests covers login through READY to the guild list appearing in the snapshot, and a sent message observed on the wire
- [ ] One command runs the suite, and CI runs it on Windows
