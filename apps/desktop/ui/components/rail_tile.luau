local theme = require("../theme")
local unread_pill = require("./unread_pill")
local mention_badge = require("./mention_badge")

export type Style = {
	rest_bg: string,
	rest_fg: string,
	active_bg: string,
	active_fg: string,
}

export type Drop = { drop: string, target: string, accepts: (payload: any) -> boolean }

export type Props = {
	face: (foreground: () -> string) -> any,
	style: Style,
	tooltip: string,
	on_click: any,
	selected: (() -> boolean)?,
	unread: (() -> boolean)?,
	mentions: (() -> number)?,
	pill_inset: number?,
	drag: (() -> any)?,
	preview: ((payload: any) -> any)?,
	onto: Drop?,
	on_drop: ((payload: any, drop: string, target: string) -> ())?,
}

local SIZE = 48

local function never(): boolean
	return false
end

local function none(): number
	return 0
end

return ui.component(function(props: Props)
	local hovered = ui.use_state(false)
	local over = ui.use_state(false)
	local selected = props.selected or never
	local unread = props.unread or never
	local mentions = props.mentions or none
	local pill = ui.computed(function()
		if selected() then
			return 40
		elseif hovered:get() then
			return 20
		elseif unread() then
			return 8
		end
		return 0
	end)
	local round = ui.computed(function()
		return selected() or hovered:get()
	end)
	local function active(): boolean
		return round:get()
	end
	local function foreground(): string
		return if active() then props.style.active_fg else props.style.rest_fg
	end
	local onto = props.onto
	return ui.button {
		relative = true,
		w_full = true,
		h = SIZE,
		shrink = 0,
		p = 0,
		border = 0,
		focus = { border = 0 },
		bg = "#00000000",
		justify = "center",
		cursor = "pointer",
		tooltip = props.tooltip,
		on_hover = function(inside: boolean)
			hovered:set(inside)
		end,
		on_click = props.on_click,
		on_drag = props.drag,
		drag_preview = props.preview,
		drop_when = if onto then onto.accepts else nil,
		on_drag_over = if onto
			then function()
				over:set(true)
			end
			else nil,
		on_drag_leave = if onto
			then function()
				over:set(false)
			end
			else nil,
		on_drop = if onto
			then function(payload: any)
				over:set(false)
				if props.on_drop then
					props.on_drop(payload, onto.drop, onto.target)
				end
			end
			else nil,
		unread_pill { height = pill, inset = props.pill_inset },
		ui.div {
			relative = true,
			w = SIZE,
			h = SIZE,
			shrink = 0,
			ui.row {
				w = SIZE,
				h = SIZE,
				justify = "center",
				items = "center",
				overflow = "hidden",
				rounded = ui.tween(round, { from = 24, to = 16, duration = 140, ease = "outQuint" }),
				bg = function()
					return if active() then props.style.active_bg else props.style.rest_bg
				end,
				props.face(foreground),
			},
			ui.show(
				over,
				ui.div {
					absolute = true,
					top = 0,
					left = 0,
					w = SIZE,
					h = SIZE,
					rounded = theme.size.radius_lg,
					border = 2,
					border_color = theme.color.brand,
				}
			),
			ui.show(function()
				return mentions() > 0
			end, ui.div {
				absolute = true,
				right = -6,
				bottom = -6,
				mention_badge { count = mentions, ring = theme.color.background_tertiary },
			}),
		},
	}
end)
