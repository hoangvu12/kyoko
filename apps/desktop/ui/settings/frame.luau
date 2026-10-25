local theme = require("../theme")
local icon = require("@components/icon")

local CLOSE = 36
local CLOSE_ICON = 18

local M = {}

export type NavItem = {
	title: string,
	icon: string,
	selected: () -> boolean,
	on_click: () -> (),
	height: number,
	pad: number,
	glyph: number,
	id: string?,
}

M.nav_item = ui.component(function(props: NavItem)
	local hovered = ui.use_state(false)
	local function color(): string
		if props.selected() then
			return theme.color.interactive_active
		end
		return if hovered:get() then theme.color.interactive_hover else theme.color.interactive_normal
	end
	return ui.button {
		id = props.id,
		w_full = true,
		h = props.height,
		px = props.pad,
		gap = theme.size.space_8,
		items = "center",
		border = 0,
		focus = { border = 0 },
		rounded = theme.size.radius_sm,
		cursor = "pointer",
		bg = function()
			if props.selected() then
				return theme.color.modifier_selected
			end
			return if hovered:get() then theme.color.modifier_hover else "#00000000"
		end,
		on_hover = function(inside: boolean)
			hovered:set(inside)
		end,
		on_click = props.on_click,
		icon { name = props.icon, size = props.glyph, color = color },
		ui.text {
			props.title,
			literal = true,
			min_w = 0,
			truncate = true,
			text_size = theme.size.font_md,
			weight = theme.weight.medium,
			color = color,
		},
	}
end)

M.close_button = ui.component(function(props: { on_close: () -> (), esc: boolean, top: number })
	local hovered = ui.use_state(false)
	return ui.column {
		absolute = true,
		top = props.top,
		right = theme.number(theme.size.space_16),
		items = "center",
		gap = theme.size.space_4,
		ui.button {
			w = CLOSE,
			h = CLOSE,
			p = 0,
			rounded = CLOSE,
			border = 2,
			focus = { border = 2 },
			justify = "center",
			items = "center",
			cursor = "pointer",
			tooltip = "Close",
			border_color = function()
				return if hovered:get() then theme.color.interactive_hover else theme.color.interactive_normal
			end,
			bg = function()
				return if hovered:get() then theme.color.modifier_hover else "#00000000"
			end,
			on_hover = function(inside: boolean)
				hovered:set(inside)
			end,
			on_click = props.on_close,
			icon { name = "close", size = CLOSE_ICON, color = theme.color.interactive_normal },
		},
		if props.esc
			then ui.text {
				"ESC",
				text_size = theme.size.font_xs,
				weight = theme.weight.bold,
				color = theme.color.interactive_normal,
			}
			else nil,
	}
end)

return M
