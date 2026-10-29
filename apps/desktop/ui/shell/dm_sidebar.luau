local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local motion = require("@components/motion")
local icon = require("@components/icon")
local icon_button = require("@components/icon_button")
local mention_badge = require("@components/mention_badge")
local avatar = require("@components/avatar")

local HEADER_PLUS = 16
local HEADER_PLUS_HIT = 18
local CLOSE_ICON = 16
local CLOSE_HIT = 20
local CONFIRM_WIDTH = 224

local icon_sm = theme.number(theme.size.icon_size_sm)
local icon_lg = theme.number(theme.size.icon_size_lg)
local avatar_sm = theme.number(theme.size.avatar_size_sm)

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
		h = theme.size.sidebar_header_height,
		shrink = 0,
		px = theme.size.space_10,
		items = "center",
		border_sides = { 0, 0, 1, 0 },
		border_color = theme.color.border,
		ui.button {
			grow = 1,
			min_w = 0,
			h = theme.size.button_height_sm,
			px = theme.size.space_8,
			border = 0,
			focus = { border = 0 },
			rounded = theme.size.radius_sm,
			justify = "start",
			items = "center",
			cursor = "pointer",
			bg = theme.color.bg_3,
			on_click = actions.dm_action("search"),
			ui.text {
				"Find or start a conversation",
				min_w = 0,
				truncate = true,
				text_size = theme.size.font_size_sm,
				line_height = theme.size.line_height_sm,
				weight = theme.weight.medium,
				color = theme.color.text_4,
			},
		},
	}
end

local nav_row = ui.component(function(props: { page: string, icon: string, label: string, count: (() -> number)? })
	local hovered = ui.use_state(false)
	local function selected(): boolean
		return state.home.page:get() == props.page
	end
	local function tone(): string
		if selected() then
			return theme.color.text_2
		end
		return if hovered:get() then theme.color.text_3 else theme.color.text_4
	end
	return ui.row {
		h = theme.size.dm_row_height,
		ml = motion.slide(hovered, theme.size.space_8, theme.size.list_item_hover_shift),
		mr = theme.size.space_8,
		px = theme.size.space_8,
		gap = theme.size.space_12,
		rounded = theme.size.radius_sm,
		items = "center",
		cursor = "pointer",
		text_size = theme.size.font_size_md,
		weight = theme.weight.medium,
		color = tone,
		bg = function()
			if selected() then
				return theme.color.active
			end
			return if hovered:get() then theme.color.hover else nil
		end,
		on_hover = function(inside: boolean)
			hovered:set(inside)
		end,
		on_click = actions.show_home_page(props.page),
		icon { name = props.icon, size = icon_lg(), color = tone },
		ui.text { props.label, grow = 1, min_w = 0, truncate = true, line_height = theme.size.line_height_label, color = tone },
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
		return if hovered:get() then theme.color.text_3 else theme.color.text_4
	end
	return ui.row {
		mt = theme.size.space_16,
		mr = theme.size.space_8,
		mb = theme.size.space_4,
		ml = theme.size.space_16,
		pr = theme.size.space_8,
		items = "center",
		on_hover = function(inside: boolean)
			hovered:set(inside)
		end,
		ui.text {
			"Direct Messages",
			grow = 1,
			min_w = 0,
			truncate = true,
			text_size = theme.size.font_size_sm,
			line_height = theme.size.line_height_sm,
			weight = theme.weight.medium,
			color = tone,
		},
		icon_button {
			name = "plus",
			icon_size = HEADER_PLUS,
			hit_size = HEADER_PLUS_HIT,
			tooltip = "Create DM",
			on_click = actions.dm_action("create_group"),
		},
	}
end)

local function group_face()
	return ui.row {
		w = theme.size.avatar_size_sm,
		h = theme.size.avatar_size_sm,
		shrink = 0,
		rounded = theme.size.avatar_size_sm,
		justify = "center",
		items = "center",
		bg = theme.color.accent_2,
		icon { name = "group", size = icon_sm(), color = theme.color.text_0 },
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
		if selected() or row.unread then
			return theme.color.text_2
		end
		return if hovered:get() then theme.color.text_3 else theme.color.text_4
	end
	local face = if row.group
		then group_face()
		else avatar {
			src = row.avatar,
			size = avatar_sm(),
			status = row.status,
			cutout = theme.color.bg_4,
		}
	local lines = {
		ui.text {
			row.name,
			literal = true,
			w_full = true,
			min_w = 0,
			truncate = true,
			text_size = theme.size.font_size_md,
			line_height = theme.size.line_height_label,
			weight = function()
				return if row.unread then theme.weight.semibold else theme.weight.medium
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
				text_size = theme.size.font_size_xs,
				line_height = theme.size.line_height_xs,
				color = theme.color.text_4,
			}
		)
	end
	return ui.column { w_full = true, ui.row {
		h = theme.size.dm_row_height,
		ml = motion.slide(hovered, theme.size.space_8, theme.size.member_hover_shift),
		mr = theme.size.space_8,
		px = theme.size.space_8,
		gap = theme.size.space_12,
		rounded = theme.size.radius_sm,
		items = "center",
		cursor = "pointer",
		bg = function()
			if selected() then
				return theme.color.active
			end
			return if hovered:get() then theme.color.hover else nil
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
			row.in_call == true,
			ui.div {
				id = "dm_in_call_" .. row.id,
				shrink = 0,
				icon { name = "volume", size = icon_sm(), color = theme.color.green_2 },
			}
		),
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
			rounded = theme.size.radius_sm,
			bg = theme.color.bg_3,
			border = theme.size.border_thickness,
			border_color = theme.color.border,
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
		search(),
		ui.scroll {
			grow = 1,
			min_h = 0,
			ui.column {
				w_full = true,
				pt = theme.size.space_8,
				pb = theme.size.space_16,
				gap = theme.size.space_2,
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
					gap = theme.number(theme.size.space_2),
				}),
			},
		},
	}
end