local theme = require("../theme")

return function()
	return ui.column {
		w = theme.size.member_list_width,
		h_full = true,
		shrink = 0,
		bg = theme.color.background_secondary,
	}
end
