local theme = require("../theme")

return function()
	return ui.row {
		w_full = true,
		h = theme.size.top_strip_height,
		shrink = 0,
		window_drag = true,
	}
end
