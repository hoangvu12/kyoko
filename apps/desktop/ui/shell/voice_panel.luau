local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")
local icon_button = require("@components/icon_button")

local SIGNAL = 16

local function panel(): state.VoicePanel
	return state.voice.panel:get()
end

local function status_color(): string
	return if panel().connected then theme.color.green_2 else theme.color.idle
end

local function status_line()
	return ui.row {
		items = "center",
		gap = theme.size.space_6,
		min_w = 0,
		icon {
			name = function()
				return if panel().connected then "signal_high" else "signal_low"
			end,
			size = SIGNAL,
			color = status_color,
		},
		ui.text {
			function()
				return panel().status
			end,
			min_w = 0,
			truncate = true,
			text_size = theme.size.font_size_sm,
			line_height = theme.size.line_height_sm,
			weight = theme.weight.semibold,
			color = status_color,
		},
	}
end

local function place()
	return ui.text {
		function()
			return panel().place
		end,
		id = "voice_panel_place",
		literal = true,
		min_w = 0,
		truncate = true,
		cursor = "pointer",
		text_size = theme.size.font_size_xs,
		line_height = theme.size.line_height_xs,
		color = theme.color.text_4,
		on_click = actions.open_call(),
	}
end

return function()
	return ui.row {
		w_full = true,
		px = theme.size.space_8,
		py = theme.size.space_8,
		gap = theme.size.space_8,
		items = "center",
		ui.column {
			grow = 1,
			min_w = 0,
			gap = theme.size.space_2,
			status_line(),
			place(),
		},
		icon_button {
			id = "voice_panel_disconnect",
			name = "phone_off",
			tooltip = "Disconnect",
			hit_size = theme.number(theme.size.button_size)(),
			filled_hover = true,
			on_click = actions.leave_voice(),
		},
	}
end
