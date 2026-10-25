local actions = require("../actions")

local SLOT = "timeline"

return function()
	return ui.column {
		grow = 1,
		min_h = 0,
		w_full = true,
		on_resize = function(_, element)
			actions.report_slot(SLOT, element)
		end,
	}
end
