local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")
local icon_button = require("@components/icon_button")
local mention_badge = require("@components/mention_badge")
local avatar = require("@components/avatar")

local SEARCH_HEIGHT = 28
local SEARCH_SIDES = 10
local NAV_ICON = 24
local AVATAR = 32
local GROUP_ICON = 18
local HEADER_HEIGHT = 24
local HEADER_PLUS = 16
local HEADER_PLUS_HIT = 18
local CLOSE_ICON = 16
local CLOSE_HIT = 20
local CONFIRM_WIDTH = 224
local NAME_LINE = 20
local SUBTITLE_LINE = 14

local function by_key(item: { key: string })
	return item.key
end

local function members_label(count: number): string
	return if count == 1 then "1 Member" else count .. " Members"
end

local function open_dm(id: string): boolean
	return state.home.page:get() == "" and state.timeline.channel_id:get() == id
end

local function search()
	return ui.row {
		w_full = true,
		h = theme.size.top_bar_height,
		shrink = 0,
		px = SEARCH_SIDES,
		items = "center",
		border_sides = { 0, 0, 1, 0 },
		border_color = theme.color.background_tertiary,
		ui.button {
			grow = 1,
			min_w = 0,
			h = SEARCH_HEIGHT,
			px = theme.size.space_8,
			border = 0,
			focus = { border = 0 },
			rounded = theme.size.radius_sm,
			justify = "start",
			items = "center",
			cursor = "pointer",
			bg = theme.color.background_tertiary,
			on_click = actions.dm_action("search"),
			ui.text {
				"Find or start a conversation",
				min_w = 0,
				truncate = true,
				text_size = theme.size.font_sm,
				color = theme.color.text_muted,
			},
		},
	}
end

local nav_row = ui.component(function(props: { page: string, icon: string, label: string, count: (() -> number)? })
	local hovered = ui.use_state(false)
	local function selected(): boolean
		return state.home.page:get() == props.page
	end
	local function rest(): string
		return if selected() then theme.color.interactive_active else theme.color.text_muted
	end
	return ui.row {
		h = theme.size.member_row_height,
		mx = theme.size.space_8,
		px = theme.size.space_8,
		gap = theme.size.space_12,
		rounded = theme.size.radius_sm,
		items = "center",
		cursor = "pointer",
		text_size = theme.size.font_md,
		weight = theme.weight.medium,
		color = rest,
		bg = function()
			if selected() then
				return theme.color.modifier_selected
			end
			return if hovered:get() then theme.color.modifier_hover else nil
		end,
		on_hover = function(inside: boolean)
			hovered:set(inside)
		end,
		on_click = actions.show_home_page(props.page),
		icon {
			name = props.icon,
			size = NAV_ICON,
			color = function()
				if selected() then
					return theme.color.interactive_active
				end
				return if hovered:get() then theme.color.interactive_hover else theme.color.text_muted
			end,
		},
		ui.text { props.label, grow = 1, min_w = 0, truncate = true, color = rest },
		if props.count
			then ui.show(function()
				return (props.count :: () -> number)() > 0
			end, mention_badge { count = props.count :: () -> number })
			else nil,
	}
end)

local section_header = ui.component(function()
	local hovered = ui.use_state(false)
	local function tone(): string
		return if hovered:get() then theme.color.interactive_hover else theme.color.header_secondary
	end
	return ui.div {
		w_full = true,
		pt = theme.size.space_16,
		ui.row {
			w_full = true,
			h = HEADER_HEIGHT,
			px = theme.size.space_8,
			gap = theme.size.space_2,
			items = "center",
			on_hover = function(inside: boolean)
				hovered:set(inside)
			end,
			ui.text {
				"DIRECT MESSAGES",
				grow = 1,
				min_w = 0,
				truncate = true,
				text_size = theme.size.font_tiny,
				weight = theme.weight.semibold,
				color = tone,
			},
			icon_button {
				name = "plus",
				icon_size = HEADER_PLUS,
				hit_size = HEADER_PLUS_HIT,
				tooltip = "Create DM",
				on_click = actions.dm_action("create_group"),
			},
		},
	}
end)

local function group_face()
	return ui.row {
		w = AVATAR,
		h = AVATAR,
		shrink = 0,
		rounded = AVATAR,
		justify = "center",
		items = "center",
		bg = theme.color.brand,
		icon { name = "group", size = GROUP_ICON, color = theme.color.text_on_brand },
	}
end

local function close_confirm(row: state.DmRow, open: any)
	local title = if row.group then "Leave Group" else "Close Conversation"
	local body = if row.group
		then "You will no longer receive messages from this group."
		else "This conversation will be removed from your list. You can reopen it at any time."
	local confirm = if row.group then "Leave Group" else "Close DM"
	return ui.column {
		w = CONFIRM_WIDTH,
		gap = theme.size.space_8,
		ui.text {
			title,
			text_size = theme.size.font_md,
			weight = theme.weight.semibold,
			color = theme.color.header_primary,
		},
		ui.text { body, text_size = theme.size.font_sm, color = theme.color.text_normal },
		ui.row {
			w_full = true,
			pt = theme.size.space_8,
			gap = theme.size.space_8,
			justify = "end",
			ui.button {
				"Cancel",
				px = theme.size.space_12,
				h = 32,
				border = 0,
				rounded = theme.size.radius_sm,
				bg = "#00000000",
				color = theme.color.text_normal,
				text_size = theme.size.font_sm,
				on_click = ui.set(open, false),
			},
			ui.button {
				confirm,
				px = theme.size.space_12,
				h = 32,
				border = 0,
				rounded = theme.size.radius_sm,
				bg = theme.color.danger,
				color = theme.color.text_on_brand,
				text_size = theme.size.font_sm,
				weight = theme.weight.medium,
				on_click = function()
					open:set(false)
					actions.close_dm(row.id)
				end,
			},
		},
	}
end

local dm_row = ui.component(function(props: { row: state.DmRow })
	local row = props.row
	local hovered = ui.use_state(false)
	local confirming = ui.use_state(false)
	local function selected(): boolean
		return open_dm(row.id)
	end
	local function name_color(): string
		if selected() then
			return theme.color.interactive_active
		end
		return if row.unread then theme.color.header_primary else theme.color.text_muted
	end
	local face = if row.group
		then group_face()
		else avatar {
			src = row.avatar,
			size = AVATAR,
			status = row.status,
			cutout = function()
				return if selected() then theme.color.modifier_selected else theme.color.background_secondary
			end,
		}
	local lines = {
		ui.text {
			row.name,
			literal = true,
			w_full = true,
			min_w = 0,
			truncate = true,
			text_size = theme.size.font_md,
			line_height = NAME_LINE,
			weight = function()
				return if row.unread or selected() then theme.weight.semibold else theme.weight.medium
			end,
			color = name_color,
		},
	}
	if row.group then
		table.insert(
			lines,
			ui.text {
				members_label(row.members),
				w_full = true,
				min_w = 0,
				truncate = true,
				text_size = theme.size.font_tiny,
				line_height = SUBTITLE_LINE,
				color = theme.color.text_muted,
			}
		)
	end
	return ui.column { w_full = true, ui.row {
		h = theme.size.member_row_height,
		mx = theme.size.space_8,
		px = theme.size.space_8,
		gap = theme.size.space_12,
		rounded = theme.size.radius_sm,
		items = "center",
		cursor = "pointer",
		bg = function()
			if selected() then
				return theme.color.modifier_selected
			end
			return if hovered:get() then theme.color.modifier_hover else nil
		end,
		on_hover = function(inside: boolean)
			hovered:set(inside)
		end,
		on_click = actions.select_dm(row.id),
		face,
		ui.column {
			grow = 1,
			min_w = 0,
			table.unpack(lines),
		},
		ui.show(
			row.mentions > 0,
			mention_badge {
				count = function()
					return row.mentions
				end,
			}
		),
		ui.popover {
			open = confirming,
			label = "Close conversation",
			p = theme.size.space_16,
			rounded = 4,
			bg = theme.color.background_floating,
			placement = { side = "bottom", align = "end", offset = 4 },
			anchor = ui.row {
				items = "center",
				opacity = function()
					return if hovered:get() or confirming:get() then 1 else 0
				end,
				icon_button {
					name = "close",
					icon_size = CLOSE_ICON,
					hit_size = CLOSE_HIT,
					tooltip = "Close DM",
					active = selected,
					on_click = ui.set(confirming, true),
				},
			},
			close_confirm(row, confirming),
		},
	} }
end)

local function requests_count(): number
	return state.dms.requests:get()
end

return function()
	return ui.column {
		w_full = true,
		grow = 1,
		min_h = 0,
		bg = theme.color.background_secondary,
		search(),
		ui.scroll {
			grow = 1,
			min_h = 0,
			ui.column {
				w_full = true,
				pt = theme.size.space_8,
				pb = theme.size.space_16,
				nav_row { page = "friends", icon = "friends", label = "Friends" },
				nav_row { page = "nitro", icon = "nitro", label = "Nitro" },
				nav_row { page = "shop", icon = "shop", label = "Shop" },
				ui.show(function()
					return requests_count() > 0
				end, nav_row {
					page = "requests",
					icon = "mail",
					label = "Message Requests",
					count = requests_count,
				}),
				section_header(),
				ui.each(state.dms.rows, function(row: state.DmRow)
					return dm_row { row = row }
				end, {
					key = by_key,
					w_full = true,
				}),
			},
		},
	}
end
