# 55 — ShowHiddenChannels and ShowHiddenThings

**What to build:** Users can see channels they can't access, with a lock icon and their name, topic, last activity and permission overwrites. They can also see moderator-only information the client already has. Both are display only.

**Blocked by:** 45 (Plugin runtime with ClearURLs), 23 (Channel sidebar)

**Status:** ready-for-agent

- [ ] Hidden channels show in the sidebar with a lock icon and an info view
- [ ] No message fetch or subscription is ever made for a hidden channel
- [ ] ShowHiddenThings displays only data already present in state
- [ ] Session tests assert zero traffic to hidden channels
