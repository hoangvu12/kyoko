# 127 — Server folders and the unread pill like Discord

**What to build:** From the user's real run 4 (2026-10-30), image-07 (Discord) in `C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/run4/` (real people's names: look, never copy). The user asked whether kyoko handles server folders (grouped servers; 22 built folders from the settings proto). Check kyoko against Discord: closed folder tile (2×2 mini icons, folder colour), opening and closing (state kept), open folder background, unread/mention badges rolling up to the closed folder, and the unread pill on the left. The pill must look the same and line up at the same x and height whether a server is inside a folder or not (today it doesn't). Mockcord gets a folder with unread servers if it lacks one.

**Blocked by:** 125 (same rail files)

**Status:** ready-for-agent

- [ ] Folders open, close and show their colour and mini icons like Discord; open state survives a restart
- [ ] Closed folders roll up unreads and mention counts
- [ ] The unread, selected and hover pills have the same size and x position inside and outside folders
- [ ] In-process UI tests; kyoko-only captures (folder closed, open, with unreads) checked by eye against image-07
