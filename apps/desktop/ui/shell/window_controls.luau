local theme = require("../theme")
local actions = require("../actions")

local HOVER_BRIGHTNESS = 0.7
local PRESSED_SCALE = 0.9

local function control(id: string, tooltip: string, color: string, action: any)
	return ui.button {
		id = id,
		tooltip = tooltip,
		w = theme.size.window_control_hit,
		h_full = true,
		shrink = 0,
		p = 0,
		border = 0,
		rounded = 0,
		bg = "#00000000",
		focus = { border = 0 },
		justify = "center",
		items = "center",
		cursor = "pointer",
		hover = { brightness = HOVER_BRIGHTNESS },
		active = { scale = PRESSED_SCALE },
		on_click = action,
		ui.div {
			w = theme.size.window_control_size,
			h = theme.size.window_control_size,
			shrink = 0,
			rounded = theme.size.window_control_size,
			bg = color,
		},
	}
end

return function()
	return ui.row {
		absolute = true,
		top = theme.number(theme.size.window_controls_top),
		right = theme.number(theme.size.window_controls_right),
		w = theme.size.window_controls_width,
		h = theme.size.button_size,
		justify = "end",
		pr = theme.size.space_xs,
		items = "center",
		ui.row {
			h_full = true,
			items = "center",
			pr = theme.size.window_controls_padding,
			ui.row {
				w = theme.size.space_xs,
				h = theme.size.space_md,
				shrink = 0,
				ui.div { w = theme.size.border_thickness, h_full = true, bg = theme.color.border },
			},
			control("window_minimize", "Minimize", theme.color.idle, actions.minimize_window()),
			control("window_maximize", "Maximize", theme.color.online, actions.maximize_window()),
			control("window_close", "Close", theme.color.dnd, actions.close_window()),
		},
	}
end
