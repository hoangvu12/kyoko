local theme = require("../theme")
local widgets = require("./widgets")

local PLAYBACK = {
	{ label = "Always play GIFs and animated emoji", value = "always" },
	{ label = "Play them when hovered", value = "on_hover" },
	{ label = "Never play them", value = "never" },
}

return function()
	return ui.column {
		w_full = true,
		widgets.heading("Reduced Motion"),
		widgets.toggle_row("accessibility.reduced_motion", "Reduce motion in animations and transitions"),
		widgets.toggle_row(
			"chat.typing_animation",
			"Typing animation",
			"Animate the dots beside who is typing. Reduced motion stops them too."
		),
		ui.div { h = theme.size.space_16 },
		widgets.heading("Animated Media", theme.size.space_8),
		widgets.setting_radios("media.animation", PLAYBACK),
		ui.div { h = theme.size.space_16 },
		widgets.heading("Font Scaling"),
		widgets.slider { key = "accessibility.font_scale", min = 0.8, max = 1.5, step = 0.05, scale = 100 },
		widgets.heading("Saturation"),
		widgets.slider { key = "accessibility.saturation", min = 0, max = 100, step = 1, scale = 1 },
	}
end
