local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")

local SLOT = "timeline"

local function by_id(item: { id: string })
	return item.id
end

local function message_row(message: state.Message)
	return ui.column {
		w_full = true,
		px = theme.size.message_gutter,
		py = theme.size.space_4,
		gap = theme.size.space_2,
		hover = { bg = theme.color.modifier_hover },
		ui.row {
			gap = theme.size.space_8,
			items = "center",
			ui.text {
				message.author,
				literal = true,
				text_size = theme.size.font_md,
				weight = theme.weight.medium,
				color = theme.color.header_primary,
			},
			ui.text { message.timestamp, literal = true, text_size = theme.size.font_tiny, color = theme.color.text_muted },
		},
		ui.text {
			message.content,
			literal = true,
			text_size = theme.size.font_md,
			color = theme.color.text_normal,
			wrap = true,
			w_full = true,
		},
	}
end

return function()
	return ui.scroll {
		grow = 1,
		min_h = 0,
		w_full = true,
		follow = true,
		on_resize = function(_, element)
			actions.report_slot(SLOT, element)
		end,
		ui.each(state.timeline.messages, message_row, { key = by_id, w_full = true, py = theme.size.space_8 }),
	}
end
