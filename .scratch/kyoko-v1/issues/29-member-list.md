# 29 — Member list

**What to build:** The member list matches oxidcord. It is grouped by role and status, and loads more members as the user scrolls.

**Blocked by:** 20 (Theme, fonts, icons and app frame), 14 (Member list ranges and member-request scheduler)

**Status:** claimed

- [ ] The member list matches oxidcord in the parity captures
- [ ] Scrolling requests further ranges and fills them in
- [ ] Presence and activity update live

## Comments

From the spike (04): one open real server produced 60–76 member-list updates in about 4 minutes; coalesce them in the bridge and keep them off the message list's update path.
