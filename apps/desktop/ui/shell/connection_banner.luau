local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")

local DOT_SHADES = { 1, 0.6, 0.3 }

local function banner()
	return state.session.connection:get()
end

local function tone()
	return if banner().tone == "danger" then theme.color.red_2 else theme.color.text_3
end

local function dots()
	local children = {}
	for index, shade in DOT_SHADES do
		children[index] = ui.column {
			w = theme.size.space_6,
			h = theme.size.space_6,
			rounded = theme.size.space_6,
			opacity = shade,
			bg = tone,
		}
	end
	return ui.row {
		shrink = 0,
		items = "center",
		gap = theme.size.space_4,
		table.unpack(children),
	}
end

local function retry()
	return ui.button {
		shrink = 0,
		px = theme.size.space_4,
		border = 0,
		focus = { border = 0 },
		rounded = theme.size.radius_xs,
		cursor = "pointer",
		bg = "#00000000",
		hover = { bg = theme.color.hover },
		on_click = actions.reconnect(),
		ui.text {
			"Retry",
			literal = true,
			text_size = theme.size.font_size_xs,
			weight = theme.weight.semibold,
			color = theme.color.text_2,
		},
	}
end

return function()
	return ui.show(
		function()
			return banner().shown
		end,
		ui.row {
			mt = theme.size.gap,
			mx = theme.size.gap,
			h = theme.size.new_messages_bar_height,
			shrink = 0,
			items = "center",
			gap = theme.size.space_10,
			px = theme.size.space_12,
			rounded = theme.size.radius_sm,
			bg = theme.color.bg_3,
			border = theme.size.border_thickness,
			border_color = theme.color.border,
			dots(),
			ui.text {
				function()
					return banner().text
				end,
				literal = true,
				grow = 1,
				min_w = 0,
				text_size = theme.size.font_size_sm,
				weight = theme.weight.medium,
				color = tone,
			},
			ui.show(function()
				return banner().retry
			end, retry()),
		}
	)
end
