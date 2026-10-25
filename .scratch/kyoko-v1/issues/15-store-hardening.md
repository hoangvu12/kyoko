# 15 — Store hardening

**What to build:** The token never lands in plaintext unless the user opts in, and on-disk data doesn't grow forever.

**Blocked by:** 05 (Session test harness on mockcord)

**Status:** claimed

- [ ] The token lives only in Windows Credential Manager. Plaintext storage needs an explicit setting and shows a warning
- [ ] A keyring error never silently falls back to a plaintext file
- [ ] The local message table and the media cache are pruned by size and age
- [ ] Signing out wipes the token, cached messages and kept deleted messages
- [ ] Session tests cover storage, the opt-in path and pruning
