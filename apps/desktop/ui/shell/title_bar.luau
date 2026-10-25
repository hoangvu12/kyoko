local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")

local CONTROL_WIDTH = 36
local CONTROL_ICON = 10
local MARK = 10

local function control(src: () -> string, action: any, closes: boolean)
	return ui.button {
		w = CONTROL_WIDTH,
		h_full = true,
		shrink = 0,
		p = 0,
		border = 0,
		rounded = 0,
		bg = "#00000000",
		focus = { border = 0 },
		justify = "center",
		items = "center",
		hover = if closes then { bg = theme.color.danger } else { bg = theme.color.modifier_selected },
		on_click = action,
		ui.svg { src = src, w = CONTROL_ICON, h = CONTROL_ICON, tint = theme.color.interactive_normal },
	}
end

local function fixed(name: string): () -> string
	local path = "icons/" .. name .. ".svg"
	return function()
		return path
	end
end

return function()
	return ui.row {
		w_full = true,
		h = theme.size.title_bar_height,
		shrink = 0,
		items = "center",
		bg = theme.color.background_tertiary,
		ui.row {
			grow = 1,
			h_full = true,
			items = "center",
			gap = theme.size.space_8,
			pl = theme.size.space_12,
			window_drag = true,
			ui.div { w = MARK, h = MARK, rounded = MARK / 2, bg = theme.color.brand },
			ui.text {
				"Kyoko",
				text_size = theme.size.font_tiny,
				weight = theme.weight.bold,
				color = theme.color.text_muted,
			},
		},
		control(fixed("window_minimize"), actions.minimize_window(), false),
		control(function()
			return if state.window.maximized:get() then "icons/window_restore.svg" else "icons/window_maximize.svg"
		end, actions.maximize_window(), false),
		control(fixed("window_close"), actions.close_window(), true),
	}
end
