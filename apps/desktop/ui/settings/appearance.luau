local theme = require("../theme")
local widgets = require("./widgets")

local THEMES = {
	{ label = "Midnight", value = "midnight" },
	{ label = "Dark", value = "dark" },
	{ label = "Light", value = "light" },
	{ label = "AMOLED", value = "amoled" },
	{ label = "System", value = "system" },
}

local DISPLAY = {
	{ label = "Cozy: modern, beautiful, and easy to read.", value = false },
	{ label = "Compact: fit more messages on screen at one time.", value = true },
}

local function block(title: string, body: any)
	return ui.column { w_full = true, widgets.heading(title, theme.size.space_8), body }
end

return function()
	return ui.column {
		w_full = true,
		gap = theme.size.space_24,
		block("Theme", widgets.setting_radios("theme", THEMES)),
		block("Message Display", widgets.setting_radios("appearance.compact_messages", DISPLAY)),
		ui.column {
			w_full = true,
			widgets.heading("DPI Scale", theme.size.space_8),
			widgets.slider { key = "appearance.ui_scale", min = 0.5, max = 2, step = 0.05, scale = 100, reset = 1, tight = true },
			ui.div { pt = theme.size.space_8, widgets.hint("Saved for a later version of Kyoko, which will scale the interface.") },
		},
	}
end
