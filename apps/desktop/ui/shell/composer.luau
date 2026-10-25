local theme = require("../theme")
local state = require("../state")
local icon = require("@components/icon")

local FIELD = 44
local LEADING = 56

return function()
	return ui.column {
		w_full = true,
		shrink = 0,
		px = theme.size.space_16,
		pb = theme.size.space_24,
		bg = theme.color.background_primary,
		ui.row {
			w_full = true,
			h = FIELD,
			items = "center",
			rounded = theme.size.radius_md,
			bg = theme.color.input_background,
			ui.row {
				w = LEADING,
				h = FIELD,
				shrink = 0,
				justify = "center",
				items = "center",
				icon { name = "plus", size = 24, color = theme.color.interactive_normal },
			},
			ui.text {
				function()
					local name = state.timeline.channel_name:get()
					return if name == "" then "" else "Message #" .. name
				end,
				literal = true,
				grow = 1,
				min_w = 0,
				truncate = true,
				text_size = theme.size.font_md,
				color = theme.color.text_muted,
			},
		},
	}
end
