local widgets = require("./widgets")

return function()
	return ui.column {
		w_full = true,
		widgets.toggle_row("streamer_mode.enabled", "Streamer Mode", "Hide sensitive details while you stream."),
		widgets.toggle_row(
			"streamer_mode.hide_personal",
			"Hide personal information",
			"Hides your email and phone number."
		),
		widgets.toggle_row("streamer_mode.hide_invites", "Hide invite links", "Hides invite links shown in the app."),
		widgets.toggle_row("streamer_mode.disable_sounds", "Disable sounds", "Mutes notification sounds."),
	}
end
