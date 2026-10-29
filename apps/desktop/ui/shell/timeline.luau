local theme = require("../theme")
local actions = require("../actions")

local SLOT = "timeline"

return function()
	return ui.column {
		id = "chat_timeline",
		grow = 1,
		min_h = 0,
		mt = theme.size.space_6,
		mx = theme.size.space_4,
		on_resize = function(_, element)
			actions.report_slot(SLOT, element)
		end,
	}
end
