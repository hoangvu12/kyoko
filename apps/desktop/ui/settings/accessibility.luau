local STICKERS = {
	{ label = "Always animate", value = "always" },
	{ label = "Animate on interaction", value = "on_hover" },
	{ label = "Never animate", value = "never" },
}

local DISPLAY = {
	{ label = "Cozy: modern, beautiful, and easy to read.", value = false },
	{ label = "Compact: fit more messages on screen at one time.", value = true },
}

return {
	headings = {
		{
			key = "reduced-motion",
			title = "Reduced Motion",
			rows = {
				{
					key = "accessibility.reduced_motion",
					title = "Enable Reduced Motion",
					note = "Reduce the motion of animations and transitions. Your system's setting turns it on too.",
					keywords = "animation motion transitions",
				},
				{
					key = "media.gif_autoplay",
					title = "Automatically play GIFs when Kyoko is focused",
					note = "When off, GIFs play while you hover them.",
					keywords = "gif autoplay animate animation",
				},
				{
					key = "media.animate_emoji",
					title = "Play animated emoji",
					note = "When off, animated emoji play while you hover them.",
					keywords = "emoji animate animation",
				},
				{
					key = "chat.typing_animation",
					title = "Typing animation",
					note = "Animate the dots beside who is typing. Reduced motion stops them too.",
					keywords = "typing indicator dots",
				},
			},
		},
		{
			key = "stickers",
			title = "Stickers",
			rows = {
				{
					kind = "radios",
					id = "settings_stickers",
					key = "media.sticker_animation",
					title = "Sticker animation",
					keywords = "sticker animate animation",
					options = STICKERS,
				},
			},
		},
		{
			key = "message-display",
			title = "Chat Message Display",
			rows = {
				{
					kind = "radios",
					key = "appearance.compact_messages",
					title = "Message display",
					note = "Saved for a later version of Kyoko.",
					keywords = "compact cozy density avatars",
					options = DISPLAY,
				},
			},
		},
		{
			key = "text",
			title = "Text",
			rows = {
				{
					kind = "slider",
					key = "accessibility.font_scale",
					title = "Font Scaling",
					note = "Saved for a later version of Kyoko.",
					keywords = "font size text bigger smaller",
					min = 0.8,
					max = 1.5,
					step = 0.05,
					scale = 100,
				},
				{
					kind = "slider",
					key = "accessibility.saturation",
					title = "Saturation",
					note = "Saved for a later version of Kyoko.",
					keywords = "colour color saturation",
					min = 0,
					max = 100,
					step = 1,
					scale = 1,
				},
			},
		},
	},
}
