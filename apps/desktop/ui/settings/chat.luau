local widgets = require("./widgets")

return function()
	return ui.column {
		w_full = true,
		widgets.toggle_row("chat.inline_media", "Inline media", "Show images and videos in messages."),
		widgets.toggle_row(
			"media.original_resolution",
			"Original image quality",
			"Load images at their full resolution instead of sized to the window."
		),
		widgets.toggle_row("chat.link_previews", "Link previews", "Show previews for links in messages."),
		widgets.toggle_row("chat.show_reactions", "Reactions", "Show reactions under messages."),
		widgets.toggle_row("chat.time_24h", "24-hour time", "Show times as 14:30 instead of 2:30 PM."),
		widgets.toggle_row("chat.emoji_autocomplete", "Emoji autocomplete", "Suggest emoji while you type."),
	}
end
