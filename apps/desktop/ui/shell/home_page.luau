local theme = require("../theme")
local state = require("../state")

local TITLES = {
	friends = "Friends",
	nitro = "Nitro",
	shop = "Shop",
	requests = "Message Requests",
}

local function title(): string
	return TITLES[state.home.page:get()] or ""
end

return function()
	return ui.show(
		function()
			return state.home.page:get() ~= ""
		end,
		ui.column {
			absolute = true,
			left = 0,
			top = 0,
			right = 0,
			bottom = 0,
			gap = theme.size.space_8,
			justify = "center",
			items = "center",
			bg = theme.color.background_primary,
			ui.text {
				title,
				text_size = theme.size.font_lg,
				weight = theme.weight.semibold,
				color = theme.color.header_primary,
			},
			ui.text {
				"Coming in a later version.",
				text_size = theme.size.font_sm,
				color = theme.color.text_muted,
			},
		}
	)
end
