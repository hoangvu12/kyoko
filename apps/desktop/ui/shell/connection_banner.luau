local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")

local HEIGHT = 32
local RETRY_HEIGHT = 20

local function banner()
	return state.session.connection:get()
end

local function retry()
	return ui.button {
		h = RETRY_HEIGHT,
		px = theme.size.space_8,
		shrink = 0,
		border = 1,
		border_color = theme.color.button_secondary,
		focus = { border = 1 },
		rounded = theme.size.radius_sm,
		cursor = "pointer",
		bg = theme.color.banner_button,
		on_click = actions.reconnect(),
		ui.text {
			"Retry",
			literal = true,
			text_size = theme.size.font_tiny,
			weight = theme.weight.semibold,
			color = theme.color.text_normal,
		},
	}
end

return function()
	return ui.show(
		function()
			return banner().shown
		end,
		ui.row {
			w_full = true,
			h = HEIGHT,
			shrink = 0,
			items = "center",
			justify = "center",
			gap = theme.size.space_12,
			bg = function()
				return if banner().tone == "danger" then theme.color.danger else theme.color.warning
			end,
			ui.text {
				function()
					return banner().text
				end,
				literal = true,
				text_size = theme.size.font_sm,
				weight = theme.weight.semibold,
				color = function()
					return if banner().tone == "danger"
						then theme.color.text_on_brand
						else theme.color.background_tertiary
				end,
			},
			ui.show(function()
				return banner().retry
			end, retry()),
		}
	)
end
