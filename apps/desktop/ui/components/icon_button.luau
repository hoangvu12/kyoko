local theme = require("../theme")
local icon = require("./icon")

export type Props = {
	name: string | () -> string,
	id: string?,
	tooltip: any?,
	icon_size: number?,
	hit_size: number?,
	active: (() -> boolean)?,
	danger: (() -> boolean)?,
	filled_hover: boolean?,
	on_click: any?,
	on_press: any?,
}

return ui.component(function(props: Props)
	local hovered = ui.use_state(false)
	local function active(): boolean
		return if props.active then props.active() else false
	end
	local function danger(): boolean
		return if props.danger then props.danger() else false
	end
	return ui.button {
		id = props.id,
		w = props.hit_size or 28,
		h = props.hit_size or 28,
		shrink = 0,
		p = 0,
		bg = "#00000000",
		border = 0,
		focus = { border = 0 },
		hover = if props.filled_hover then { bg = theme.color.modifier_hover } else nil,
		rounded = theme.size.radius_sm,
		justify = "center",
		items = "center",
		cursor = "pointer",
		tooltip = props.tooltip,
		on_hover = function(inside: boolean)
			hovered:set(inside)
		end,
		on_click = props.on_click,
		on_press = props.on_press,
		icon {
			name = props.name,
			size = props.icon_size or 20,
			color = function()
				if active() then
					return theme.color.interactive_active
				elseif danger() then
					return theme.color.danger
				end
				return if hovered:get() then theme.color.interactive_hover else theme.color.interactive_normal
			end,
		},
	}
end)
