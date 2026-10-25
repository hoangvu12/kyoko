local theme = require("../theme")
local actions = require("../actions")
local widgets = require("./widgets")

local M = {}

function M.page(icon: string, note: string, web: boolean?)
	return function()
		return ui.column {
			w_full = true,
			gap = theme.size.space_16,
			items = "start",
			widgets.placeholder(icon, note),
			if web
				then widgets.button {
					text = "Open Discord",
					on_click = function()
						actions.settings_action("open_web")
					end,
				}
				else nil,
		}
	end
end

return M
