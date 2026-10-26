local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")
local icon_button = require("@components/icon_button")
local mention_badge = require("@components/mention_badge")
local motion = require("@components/motion")
local dm_sidebar = require("./dm_sidebar")

local MENU_WIDTH = theme.size.context_menu_width
local CATEGORY_PLUS = 16
local CATEGORY_PLUS_HIT = 18
local LOCK_BADGE = 11
local LOCK_ICON = 9
local LOCK_OFFSET = -3
local SPINE_LEFT = 32
local SPINE_WIDTH = 16
local SPINE_HALF = 19
local SPINE_LINE = 2
local SPINE_CORNER = 8
local THREAD_INDENT = 40
local VOICE_INDENT = 28
local SMALL_TEXT_LIFT = 2

local icon_xs = theme.number(theme.size.icon_size_xs)
local icon_sm = theme.number(theme.size.icon_size_sm)
local icon_md = theme.number(theme.size.icon_size_md)
local button_size = theme.number(theme.size.button_size)
local dot_size = theme.number(theme.size.unread_dot_size)

local menu_style = {
	w = MENU_WIDTH,
	px = theme.size.space_8,
	py = theme.size.space_6,
	gap = theme.size.space_2,
	rounded = theme.size.radius_sm,
	bg = theme.color.bg_3,
	border = theme.size.border_thickness,
	border_color = theme.color.border,
}

local function by_key(item: { key: string })
	return item.key
end

local function separator()
	return ui.div { h = theme.size.border_thickness, my = theme.size.space_4, mx = theme.size.space_4, bg = theme.color.border }
end

local function item(label: string, on_click: () -> (), disabled: boolean?, id: string?)
	return ui.menu_item { label, id = id, on_click = on_click, disabled = disabled == true }
end

local function later(name: string, id: string?): () -> ()
	return function()
		actions.sidebar_action(name, id)
	end
end

local MUTE_WINDOWS = {
	{ label = "For 15 Minutes", minutes = 15 },
	{ label = "For 1 Hour", minutes = 60 },
	{ label = "For 8 Hours", minutes = 480 },
	{ label = "For 24 Hours", minutes = 1440 },
	{ label = "Until I turn it back on", minutes = 0 },
}

local LEVELS = {
	{ label = "All Messages", level = "all" },
	{ label = "Only @mentions", level = "mentions" },
	{ label = "Nothing", level = "nothing" },
	{ label = "Use Category Default", level = "inherit" },
}

local function send(name: string, payload: any): () -> ()
	return function()
		actions.sidebar_send(name, payload)
	end
end

local function submenu(id: string, label: string, open: any, items: { any })
	return ui.submenu {
		open = open,
		trigger = ui.menu_item { label, id = id },
		label = label,
		w = MENU_WIDTH,
		px = theme.size.space_8,
		py = theme.size.space_6,
		gap = theme.size.space_2,
		rounded = theme.size.radius_sm,
		bg = theme.color.bg_3,
		border = theme.size.border_thickness,
		border_color = theme.color.border,
		table.unpack(items),
	}
end

local function mute_items(action: string, id: string): { any }
	local items = {}
	for _, window in MUTE_WINDOWS do
		table.insert(
			items,
			item(window.label, send(action, { id = id, minutes = window.minutes }), nil, `{action}_{window.minutes}`)
		)
	end
	return items
end

local function level_items(id: string): { any }
	local current = actions.channel_level(id)
	local items = {}
	for _, choice in LEVELS do
		table.insert(
			items,
			ui.menu_item {
				choice.label,
				id = `level_{choice.level}`,
				checked = current == choice.level,
				on_click = send("channel_notifications", { id = id, level = choice.level }),
			}
		)
	end
	return items
end

local function header_flag(name: string): () -> boolean
	return function()
		return (state.sidebar.header:get() :: any)[name] == true
	end
end

local function header_lacks(name: string): () -> boolean
	local flag = header_flag(name)
	return function()
		return not flag()
	end
end

local function header_menu(mute_open: any): { any }
	local can_invite = header_flag("can_invite")
	local can_settings = header_flag("can_open_settings")
	local can_manage = header_flag("can_manage_channels")
	return {
		ui.show(can_invite, item("Invite People", later("invite"), nil, "menu_invite")),
		ui.show(can_settings, item("Server Settings", later("server_settings"))),
		ui.show(can_manage, item("Create Channel", later("create_channel"))),
		ui.show(function()
			return can_invite() or can_settings() or can_manage()
		end, separator()),
		ui.menu_item { "Mark As Read", on_click = later("mark_guild_read"), disabled = header_lacks("unread") },
		ui.show(
			header_flag("muted"),
			item("Unmute Server", send("mute_guild", { unmute = true }), nil, "menu_unmute_guild"),
			submenu("menu_mute_guild", "Mute Server", mute_open, mute_items("mute_guild", ""))
		),
		item("Notification Settings", later("notification_settings"), nil, "menu_notifications"),
		ui.show(header_lacks("owner"), separator()),
		ui.show(header_lacks("owner"), item("Leave Server", later("leave_guild"), nil, "menu_leave")),
	}
end

local function channel_menu(channel: state.SidebarRow): { any }
	local header = state.sidebar.header:get()
	local items = {
		item("Mark As Read", later("mark_channel_read", channel.id), not channel.unread),
		separator(),
	}
	if not channel.thread and header.can_invite then
		table.insert(items, item("Invite to Channel", later("invite", channel.id), nil, "menu_invite_channel"))
	end
	if channel.muted then
		table.insert(
			items,
			item("Unmute Channel", send("mute_channel", { id = channel.id, unmute = true }), nil, "menu_unmute_channel")
		)
	else
		table.insert(
			items,
			submenu(
				"menu_mute_channel",
				"Mute Channel",
				ui.state("channel_mute_open", false),
				mute_items("mute_channel", channel.id)
			)
		)
	end
	if not channel.thread then
		table.insert(
			items,
			submenu(
				"menu_channel_levels",
				"Notification Settings",
				ui.state("channel_levels_open", false),
				level_items(channel.id)
			)
		)
	end
	if not channel.thread and header.can_manage_channels then
		table.insert(items, separator())
		table.insert(items, item("Edit Channel", later("edit_channel", channel.id)))
		table.insert(items, item("Delete Channel", later("delete_channel", channel.id)))
	end
	table.insert(items, separator())
	table.insert(items, item("Copy Link", later("copy_channel_link", channel.id)))
	return items
end

local header = ui.component(function()
	local open = ui.use_state(false)
	local hovered = ui.use_state(false)
	local mute_open = ui.use_state(false)
	return ui.row {
		w_full = true,
		h = theme.size.sidebar_header_height,
		shrink = 0,
		items = "center",
		gap = theme.size.space_4,
		pl = theme.size.space_8,
		pr = theme.size.space_8,
		border_sides = { 0, 0, 1, 0 },
		border_color = theme.color.border,
		ui.menu {
			open = open,
			label = "Server menu",
			w = MENU_WIDTH,
			px = theme.size.space_8,
			py = theme.size.space_6,
			gap = theme.size.space_2,
			rounded = theme.size.radius_sm,
			bg = theme.color.bg_3,
			border = theme.size.border_thickness,
			border_color = theme.color.border,
			placement = { side = "bottom", align = "center", offset = 4 },
			anchor = ui.button {
				id = "server_menu",
				grow = 1,
				min_w = 0,
				h = theme.size.button_size,
				p = 0,
				px = theme.size.space_8,
				border = 0,
				rounded = theme.size.radius_sm,
				focus = { border = 0 },
				justify = "start",
				items = "center",
				cursor = "pointer",
				bg = function()
					return if hovered:get() or open:get() then theme.color.hover else "#00000000"
				end,
				on_hover = function(inside: boolean)
					hovered:set(inside)
				end,
				on_click = ui.toggle(open),
				ui.text {
					function()
						return state.sidebar.guild_name:get()
					end,
					literal = true,
					min_w = 0,
					truncate = true,
					text_size = theme.size.font_size_md,
					line_height = theme.size.line_height_md,
					weight = theme.weight.semibold,
					color = theme.color.text_2,
				},
				ui.div {
					shrink = 0,
					ml = theme.size.space_4,
					icon { name = "chevron_down", size = icon_sm(), color = theme.color.text_2 },
				},
			},
			table.unpack(header_menu(mute_open)),
		},
		ui.show(
			header_flag("can_invite"),
			icon_button {
				name = "friend_requests",
				tooltip = "Invite People",
				hit_size = button_size(),
				icon_size = icon_md(),
				on_click = later("invite"),
			}
		),
	}
end)

local category = ui.component(function(props: { row: state.SidebarRow, first: boolean })
	local row = props.row
	local hovered = ui.use_state(false)
	local function tone()
		return if hovered:get() then theme.color.text_3 else theme.color.text_4
	end
	return ui.div {
		w_full = true,
		pt = if props.first then theme.size.space_10 else theme.size.space_16,
		ui.row {
			h = theme.size.category_row_height,
			mx = theme.size.space_8,
			px = theme.size.space_6,
			gap = theme.size.space_2,
			items = "center",
			cursor = "pointer",
			on_hover = function(inside: boolean)
				hovered:set(inside)
			end,
			on_click = actions.toggle_category(row.id),
			ui.text {
				row.name,
				literal = true,
				min_w = 0,
				shrink = 1,
				truncate = true,
				text_size = theme.size.font_size_sm,
				line_height = theme.size.line_height_sm,
				weight = theme.weight.medium,
				color = tone,
			},
			icon {
				name = if row.collapsed then "chevron_right" else "chevron_down",
				size = icon_xs(),
				color = tone,
			},
			ui.div { grow = 1 },
			ui.row {
				items = "center",
				opacity = function()
					return if hovered:get() then 1 else 0
				end,
				icon_button {
					name = "plus",
					icon_size = CATEGORY_PLUS,
					hit_size = CATEGORY_PLUS_HIT,
					tooltip = "Create Channel",
					on_click = later("create_channel", row.id),
				},
			},
		},
	}
end)

local function tones(row: state.SidebarRow, selected: boolean): (string, string)
	if selected then
		return theme.color.text_2, theme.color.text_3
	elseif row.muted or row.locked then
		return theme.color.text_5, theme.color.text_5
	elseif row.unread then
		return theme.color.text_3, theme.color.text_3
	end
	return theme.color.text_4, theme.color.text_3
end

local function glyph(row: state.SidebarRow, color: () -> string)
	return ui.div {
		relative = true,
		w = theme.size.icon_size_md,
		h = theme.size.icon_size_md,
		shrink = 0,
		icon { name = row.icon or "hash", size = icon_md(), color = color },
		ui.show(
			row.locked == true,
			ui.row {
				absolute = true,
				right = LOCK_OFFSET,
				bottom = LOCK_OFFSET,
				w = LOCK_BADGE,
				h = LOCK_BADGE,
				rounded = LOCK_BADGE,
				justify = "center",
				items = "center",
				bg = theme.color.bg_4,
				icon { name = "lock", size = LOCK_ICON, color = color },
			}
		),
	}
end
local function spine(last: boolean)
	return ui.div {
		absolute = true,
		left = SPINE_LEFT,
		top = 0,
		w = SPINE_WIDTH,
		h = theme.size.channel_item_pitch,
		ui.div {
			absolute = true,
			left = 0,
			top = 0,
			w = SPINE_WIDTH,
			h = SPINE_HALF,
			border_sides = { 0, 0, SPINE_LINE, SPINE_LINE },
			border_color = theme.color.text_5,
			rounded_bl = SPINE_CORNER,
		},
		ui.show(
			not last,
			ui.div {
				absolute = true,
				left = 0,
				top = SPINE_HALF,
				h = SPINE_HALF,
				w = SPINE_LINE,
				bg = theme.color.text_5,
			}
		),
	}
end

local function voice_state_icons(member: state.VoiceMember): { any }
	local icons = {}
	local size = theme.number(theme.size.voice_badge_icon)
	if member.video == "camera" then
		table.insert(icons, icon { name = "video", size = size(), color = theme.color.text_4 })
	elseif member.video == "screen" then
		table.insert(icons, icon { name = "screen_share", size = size(), color = theme.color.text_4 })
	end
	if member.deafened then
		table.insert(icons, icon { name = "headphones_off", size = size(), color = theme.color.red_3 })
	elseif member.muted then
		table.insert(icons, icon { name = "mic_off", size = size(), color = theme.color.red_3 })
	end
	return icons
end

local function voice_member(member: state.VoiceMember)
	return ui.row {
		h = theme.size.voice_row_height,
		ml = VOICE_INDENT,
		mr = theme.size.space_8,
		px = theme.size.space_8,
		pb = SMALL_TEXT_LIFT,
		gap = theme.size.space_8,
		rounded = theme.size.radius_sm,
		items = "center",
		hover = { bg = theme.color.hover },
		ui.div {
			w = theme.size.voice_row_avatar,
			h = theme.size.voice_row_avatar,
			shrink = 0,
			rounded = theme.size.voice_row_avatar,
			overflow = "hidden",
			bg = theme.color.bg_3,
			ui.show(
				member.avatar ~= "",
				ui.image {
					src = member.avatar,
					w = theme.size.voice_row_avatar,
					h = theme.size.voice_row_avatar,
					fit = "cover",
				}
			),
		},
		ui.text {
			member.name,
			literal = true,
			grow = 1,
			min_w = 0,
			truncate = true,
			text_size = theme.size.font_size_sm,
			color = theme.color.text_4,
		},
		ui.show(
			member.live,
			ui.text {
				"LIVE",
				shrink = 0,
				px = theme.size.space_4,
				rounded = theme.size.radius_sm,
				bg = theme.color.red_2,
				text_size = theme.size.font_size_xs,
				line_height = 16,
				weight = theme.weight.bold,
				color = theme.color.text_0,
			}
		),
		table.unpack(voice_state_icons(member)),
	}
end

local function unread_dot(hovered: any)
	return ui.row {
		absolute = true,
		top = 0,
		bottom = 0,
		left = motion.slide(hovered, 0, theme.size.unread_dot_size),
		w = 0,
		justify = "center",
		items = "center",
		pointer_events = "none",
		ui.div {
			w = theme.size.unread_dot_size,
			h = theme.size.unread_dot_size,
			shrink = 0,
			rounded = theme.size.unread_dot_size,
			bg = theme.color.text_3,
		},
	}
end

local channel = ui.component(function(props: { row: state.SidebarRow })
	local row = props.row
	local hovered = ui.use_state(false)
	local function selected(): boolean
		if row.hidden == true then
			local hidden = state.hidden.view:get()
			return hidden.open and hidden.id == row.id
		end
		return state.timeline.channel_id:get() == row.id
	end
	local function rest(): string
		local value = tones(row, selected())
		return value
	end
	local function lit(): string
		local value, hover = tones(row, selected())
		return if hovered:get() then hover else value
	end
	local selectable = row.selectable == true
	local on_click = if selectable
		then actions.select_channel(row.id)
		elseif row.hidden == true then actions.open_hidden_channel(row.id)
		else nil
	local members = {}
	for _, member in row.voice or {} do
		table.insert(members, voice_member(member))
	end
	return ui.column {
		relative = true,
		w_full = true,
		ui.show(row.thread == true, spine(row.last_thread == true)),
		ui.div {
			relative = true,
			w_full = true,
			py = theme.size.space_1,
			ui.show(function()
				return row.unread == true and not row.muted
			end, unread_dot(hovered)),
			ui.row {
				id = `channel_{row.id}`,
				relative = true,
				h = theme.size.channel_item_height,
				ml = if row.thread
					then motion.slide(hovered, theme.size.space_40, theme.size.list_item_hover_shift)
					else motion.slide(hovered, theme.size.space_8, theme.size.list_item_hover_shift),
				mr = theme.size.space_8,
				px = theme.size.space_8,
				gap = theme.size.space_6,
				rounded = theme.size.radius_sm,
				items = "center",
				text_size = theme.size.font_size_md,
				color = rest,
				cursor = if on_click ~= nil then "pointer" else "not-allowed",
				bg = function()
					if selected() then
						return theme.color.active
					end
					return if hovered:get() then theme.color.hover else nil
				end,
				on_hover = function(inside: boolean)
					hovered:set(inside)
				end,
				on_click = on_click,
				context_menu_below = true,
				context_menu_style = menu_style,
				context_menu = function()
					return channel_menu(row)
				end,
				glyph(row, lit),
				ui.text {
					row.name,
					literal = true,
					grow = 1,
					min_w = 0,
					truncate = true,
					line_height = theme.size.line_height_md,
					color = rest,
					weight = theme.weight.medium,
				},
				ui.show(
					(row.mentions or 0) > 0,
					mention_badge {
						count = function()
							return row.mentions or 0
						end,
					}
				),
			},
		},
		ui.column {
			w_full = true,
			table.unpack(members),
		},
	}
end)

local function sidebar_row(row: state.SidebarRow, index: number)
	if row.kind == "category" then
		return category { row = row, first = index == 1 }
	end
	return channel { row = row }
end

local function home(): boolean
	return state.sidebar.guild_id:get() == ""
end

return function()
	return ui.show(home, dm_sidebar(), ui.column {
		w_full = true,
		grow = 1,
		min_h = 0,
		header(),
		ui.scroll {
			grow = 1,
			min_h = 0,
			ui.each(state.sidebar.rows, sidebar_row, {
				key = by_key,
				w_full = true,
				pb = theme.size.space_16,
			}),
		},
	})
end