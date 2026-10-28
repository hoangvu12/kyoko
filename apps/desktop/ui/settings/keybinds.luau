local theme = require("../theme")
local widgets = require("./widgets")
local keybind = require("./keybind")

local BINDS = {
	{ key = "keybinds.toggle_window", label = "Show or Hide Kyoko" },
	{ key = "keybinds.push_to_talk", label = "Push to Talk" },
}

return function()
	local rows = {}
	for _, bind in BINDS do
		table.insert(rows, keybind(bind))
	end
	table.insert(
		rows,
		widgets.label(
			"Keybinds work while Kyoko is in the background. Push to Talk listens only while Push to Talk is the input mode.",
			theme.size.font_xs,
			theme.color.text_muted,
			{ literal = true }
		)
	)
	return ui.column { w_full = true, gap = theme.size.space_12, table.unpack(rows) }
end
