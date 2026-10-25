# 45 — Plugin runtime with ClearURLs

**What to build:** Users can enable, configure and disable Luau plugins, approving each one's permissions explicitly. ClearURLs, the first plugin, strips tracking parameters from outgoing links.

**Blocked by:** 27 (Composer), 37 (Account and settings pages)

**Status:** ready-for-agent

- [ ] Each plugin declares a manifest (permissions, settings schema, version), and enabling it asks the user to approve its permissions
- [ ] The plugin's logic runs in a sandboxed Luau VM on a worker thread with call deadlines. Repeated errors or overruns disable the plugin with a notice and never crash kyoko
- [ ] Plugins get their own storage and settings. Shared Discord data is read-only to them
- [ ] Discord actions from plugins are refused outside a user gesture, and rate-limited per plugin
- [ ] The plugin settings screen follows oxidcord's visual style, and plugins hot-reload in development
- [ ] The before-send hook exists. ClearURLs ships its rules inside the package and works
- [ ] Session tests cover permissions, the gesture rule, deadlines and the hook

## Comments

From the spike (04): zgx host image names and shared-value names are global, so plugins need their own namespace (prefix per plugin id) to avoid clashing with the app and each other.
