Two changes across the project, then re-export every page.

1. The signed-in user is "hoangvu12" on every page, not "refact0r": display name and username "hoangvu12", in the user panel, the status menu, settings (profile card, My Account), the full profile and popout when they show the signed-in user, the login pages, the interaction header ("hoangvu12 used /roll"), "Replying to" and every message the signed-in user wrote. Keep the same avatar placeholder style. Other people keep their names. Where refact0r appears as someone else (for example the Midnight theme's author in a message or an embed), leave it.

2. Kyoko supports all of Discord's built-in commands, so the command picker shows them as Discord does. Measured from the live client: the Built-In section comes last, its rail icon is a slash in a rounded square (the last rail item, after a small divider), each row has "Built-In" at the right, and the rows are, in order:
- /gif query: "Search Animated GIFs on the Web"
- /me: "Displays text with emphasis."
- /msg: "Message user"
- /nick: "Change nickname on this server."
- /schedule: "Schedule a message to send later"
- /shrug: "Appends ¯\_(ツ)_/¯ to your message."
- /spoiler: "Marks your message as a spoiler."
- /sticker: "Search your stickers"
- /tableflip: "Appends (╯°□°)╯︵ ┻━┻ to your message."
- /thread: "Start new thread"
- /unflip: "Appends ┬─┬ノ( º _ ºノ) to your message."
With the permission, /kick, /ban and /timeout appear too (Kick user, Ban user, Timeout user), sorted in among the others alphabetically.

Pages:
- Update "Command Picker" so its rail ends with the Built-In icon and the Frequently Used section includes a built-in (/shrug).
- New "Command Picker - Built-In": the picker scrolled to the Built-In section with the full list above (as a moderator, so /ban, /kick and /timeout show), /gif selected.
- New "Built-In - GIF": after "/gif cat", Discord shows the results as a horizontal strip of GIFs above the composer (one row, about 120 high, scrolling sideways, the first one selected with the --accent-2 border); Enter or a click sends it. Same strip for "/sticker wave" with stickers, as "Built-In - Sticker".
- New "Built-In - Thread": /thread with its options "name" and "message" filled; the hint bar reads "name  Name your thread".
