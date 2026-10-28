local theme = require("../theme")
local unread_pill = require("./unread_pill")
local mention_badge = require("./mention_badge")
local icon = require("./icon")

export type Style = {
	rest_bg: string,
	rest_fg: string,
	hover_bg: string?,
	hover_fg: string?,
	active_bg: string,
	active_fg: string,
}

export type Drop = { drop: string, target: string, accepts: (payload: any) -> boolean }

export type Props = {
	face: (foreground: () -> string, hovered: any) -> any,
	style: Style,
	tooltip: any,
	tooltip_placement: any?,
	on_click: any,
	id: string?,
	size: string?,
	radius: string?,
	selected: (() -> boolean)?,
	unread: (() -> boolean)?,
	mentions: (() -> number)?,
	speaker: boolean?,
	drag: (() -> any)?,
	preview: ((payload: any) -> any)?,
	onto: Drop?,
	on_drop: ((payload: any, drop: string, target: string) -> ())?,
}

local function never(): boolean
	return false
end

local function none(): number
	return 0
end

local selected_pill = theme.number(theme.size.guild_pill_selected_height)
local hover_pill = theme.number(theme.size.guild_pill_hover_height)
local unread_pill_height = theme.number(theme.size.guild_pill_unread_height)
local badge_inset = theme.number(theme.size.space_4)

local function speaker_badge()
	return ui.div {
		id = "rail_voice_badge",
		absolute = true,
		top = -badge_inset(),
		right = -badge_inset(),
		w = theme.size.icon_size_md,
		ui.row {
			w = theme.size.icon_size_md,
			h = theme.size.icon_size_md,
			justify = "center",
			items = "center",
			rounded = theme.size.icon_size_md,
			bg = theme.color.bg_1,
			border = theme.size.space_2,
			border_color = theme.color.bg_4,
			icon { name = "volume_fill", size = theme.number(theme.size.icon_size_xs)(), color = theme.color.text_2 },
		},
	}
end

return ui.component(function(props: Props)
	local hovered = ui.use_state(false)
	local over = ui.use_state(false)
	local selected = props.selected or never
	local unread = props.unread or never
	local mentions = props.mentions or none
	local size = props.size or theme.size.guildbar_avatar_size
	local radius = props.radius or theme.size.radius_md
	local pill = ui.computed(function()
		if selected() then
			return selected_pill()
		elseif hovered:get() then
			return hover_pill()
		elseif unread() then
			return unread_pill_height()
		end
		return 0
	end)
	local function foreground(): string
		if selected() then
			return props.style.active_fg
		end
		return if hovered:get() then props.style.hover_fg or props.style.active_fg else props.style.rest_fg
	end
	local function background(): string
		if selected() then
			return props.style.active_bg
		end
		return if hovered:get() then props.style.hover_bg or props.style.active_bg else props.style.rest_bg
	end
	local onto = props.onto
	return ui.button {
		id = props.id,
		relative = true,
		w_full = true,
		h = size,
		shrink = 0,
		p = 0,
		border = 0,
		focus = { border = 0 },
		bg = "#00000000",
		justify = "center",
		cursor = "pointer",
		tooltip = props.tooltip,
		tooltip_placement = props.tooltip_placement,
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
		unread_pill { height = pill },
		ui.div {
			relative = true,
			w = size,
			h = size,
			shrink = 0,
			ui.row {
				w = size,
				h = size,
				justify = "center",
				items = "center",
				overflow = "hidden",
				rounded = radius,
				bg = background,
				props.face(foreground, hovered),
			},
			ui.show(
				over,
				ui.div {
					absolute = true,
					top = 0,
					left = 0,
					w = size,
					h = size,
					rounded = radius,
					border = 2,
					border_color = theme.color.accent_2,
				}
			),
			ui.show(function()
				return mentions() > 0
			end, ui.div {
				absolute = true,
				right = -6,
				bottom = -6,
				mention_badge { count = mentions, ring = theme.color.bg_4 },
			}),
			if props.speaker then speaker_badge() else nil,
		},
	}
end)
