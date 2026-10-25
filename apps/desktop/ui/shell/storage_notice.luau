local theme = require("../theme")
local state = require("../state")

return function()
	return ui.show(
		function()
			return state.session.storage_warning:get() ~= ""
		end,
		ui.row {
			w_full = true,
			shrink = 0,
			px = theme.size.space_16,
			py = theme.size.space_8,
			bg = theme.color.warning,
			ui.text {
				function()
					return state.session.storage_warning:get()
				end,
				literal = true,
				wrap = true,
				text_size = theme.size.font_sm,
				weight = theme.weight.medium,
				color = theme.color.background_tertiary,
			},
		}
	)
end