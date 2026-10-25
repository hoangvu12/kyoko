local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")
local icon_button = require("@components/icon_button")
local mention_badge = require("@components/mention_badge")
local dm_sidebar = require("./dm_sidebar")

local MENU_WIDTH = 224
local CHEVRON = 18
local CATEGORY_HEIGHT = 24
local CATEGORY_CHEVRON = 12
local CATEGORY_PLUS = 16
local CATEGORY_PLUS_HIT = 18
local ROW_GAP = 6
local GLYPH = 20
local LOCK_BADGE = 11
local LOCK_ICON = 9
local LOCK_OFFSET = -3
local PILL_LEFT = -8
local PILL_TOP = 12
local PILL_WIDTH = 4
local PILL_HEIGHT = 8
local SPINE_LEFT = 32
local SPINE_WIDTH = 16
local SPINE_HALF = 16
local SPINE_LINE = 2
local SPINE_CORNER = 8
local THREAD_INDENT = 40
local VOICE_INDENT = 28
local SMALL_TEXT_LIFT = 2

local menu_style = {
	w = MENU_WIDTH,
	p = 6,
	rounded = 4,
	bg = theme.color.background_floating,
}

local function by_key(item: { key: string })
	return item.key
end

local function separator()
	return ui.div { h = 1, my = 4, mx = 4, bg = theme.color.modifier_hover }
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
		p = 6,
		rounded = 4,
		bg = theme.color.background_floating,
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
	return ui.div {
		w_full = true,
		shrink = 0,
		border_sides = { 0, 0, 1, 0 },
		border_color = theme.color.background_tertiary,
		ui.menu {
			open = open,
			label = "Server menu",
			w = MENU_WIDTH,
			p = 6,
			rounded = 4,
			bg = theme.color.background_floating,
			placement = { side = "bottom", align = "center", offset = 4 },
			anchor = ui.button {
				id = "server_menu",
				w_full = true,
				h = theme.size.guild_header_height,
				p = 0,
				border = 0,
				rounded = 0,
				focus = { border = 0 },
				justify = "center",
				items = "center",
				cursor = "pointer",
				bg = function()
					return if hovered:get() or open:get() then theme.color.modifier_hover else "#00000000"
				end,
				on_hover = function(inside: boolean)
					hovered:set(inside)
				end,
				on_click = ui.toggle(open),
				ui.row {
					min_w = 0,
					px = theme.size.space_16,
					items = "center",
					ui.text {
						function()
							return state.sidebar.guild_name:get()
						end,
						literal = true,
						min_w = 0,
						truncate = true,
						text_size = theme.size.font_md,
						weight = theme.weight.semibold,
						color = theme.color.header_primary,
					},
					icon { name = "chevron_down", size = CHEVRON, color = theme.color.header_primary },
				},
			},
			table.unpack(header_menu(mute_open)),
		},
	}
end)

local category = ui.component(function(props: { row: state.SidebarRow })
	local row = props.row
	local hovered = ui.use_state(false)
	local function tone()
		return if hovered:get() then theme.color.interactive_hover else theme.color.header_secondary
	end
	return ui.div {
		w_full = true,
		pt = theme.size.space_16,
		ui.row {
			w_full = true,
			h = CATEGORY_HEIGHT,
			px = theme.size.space_8,
			pb = SMALL_TEXT_LIFT,
			gap = theme.size.space_2,
			items = "center",
			cursor = "pointer",
			on_hover = function(inside: boolean)
				hovered:set(inside)
			end,
			on_click = actions.toggle_category(row.id),
			icon {
				name = if row.collapsed then "chevron_right" else "chevron_down",
				size = CATEGORY_CHEVRON,
				color = tone,
			},
			ui.text {
				string.upper(row.name),
				literal = true,
				grow = 1,
				min_w = 0,
				truncate = true,
				text_size = theme.size.font_tiny,
				weight = theme.weight.semibold,
				color = tone,
			},
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
		return theme.color.interactive_active, theme.color.interactive_active
	elseif row.unread then
		return theme.color.header_primary, theme.color.header_primary
	elseif row.muted or row.locked then
		return theme.color.interactive_muted, theme.color.interactive_normal
	end
	return theme.color.text_muted, theme.color.interactive_hover
end

local function glyph(row: state.SidebarRow, color: () -> string)
	return ui.div {
		relative = true,
		w = GLYPH,
		h = GLYPH,
		shrink = 0,
		icon { name = row.icon or "hash", size = GLYPH, color = color },
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
				bg = theme.color.background_secondary,
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
		h = theme.size.channel_row_height,
		ui.div {
			absolute = true,
			left = 0,
			top = 0,
			w = SPINE_WIDTH,
			h = SPINE_HALF,
			border_sides = { 0, 0, SPINE_LINE, SPINE_LINE },
			border_color = theme.color.interactive_muted,
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
				bg = theme.color.interactive_muted,
			}
		),
	}
end

local function voice_state_icons(member: state.VoiceMember): { any }
	local icons = {}
	local size = theme.number(theme.size.voice_badge_icon)
	if member.video == "camera" then
		table.insert(icons, icon { name = "video", size = size(), color = theme.color.text_muted })
	elseif member.video == "screen" then
		table.insert(icons, icon { name = "screen_share", size = size(), color = theme.color.text_muted })
	end
	if member.deafened then
		table.insert(icons, icon { name = "headphones_off", size = size(), color = theme.color.danger })
	elseif member.muted then
		table.insert(icons, icon { name = "mic_off", size = size(), color = theme.color.danger })
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
		hover = { bg = theme.color.modifier_hover },
		ui.div {
			w = theme.size.voice_row_avatar,
			h = theme.size.voice_row_avatar,
			shrink = 0,
			rounded = theme.size.voice_row_avatar,
			overflow = "hidden",
			bg = theme.color.background_tertiary,
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
			text_size = theme.size.font_sm,
			color = theme.color.text_muted,
		},
		ui.show(
			member.live,
			ui.text {
				"LIVE",
				shrink = 0,
				px = theme.size.space_4,
				rounded = theme.size.radius_sm,
				bg = theme.color.live_badge,
				text_size = theme.size.font_tiny,
				line_height = 16,
				weight = theme.weight.bold,
				color = theme.color.text_on_brand,
			}
		),
		table.unpack(voice_state_icons(member)),
	}
end

local channel = ui.component(function(props: { row: state.SidebarRow })
	local row = props.row
	local hovered = ui.use_state(false)
	local function selected(): boolean
		return state.timeline.channel_id:get() == row.id
	end
	local function rest(): string
		local value = tones(row, selected())
		return value
	end
	local function glyph_color(): string
		local value, hover = tones(row, selected())
		return if hovered:get() then hover else value
	end
	local selectable = row.selectable == true
	local members = {}
	for _, member in row.voice or {} do
		table.insert(members, voice_member(member))
	end
	return ui.column {
		relative = true,
		w_full = true,
		ui.show(row.thread == true, spine(row.last_thread == true)),
		ui.row {
			id = `channel_{row.id}`,
			relative = true,
			h = theme.size.channel_row_height,
			ml = if row.thread then THREAD_INDENT else theme.size.space_8,
			mr = theme.size.space_8,
			px = theme.size.space_8,
			gap = ROW_GAP,
			rounded = theme.size.radius_sm,
			items = "center",
			text_size = theme.size.font_md,
			color = rest,
			cursor = if selectable then "pointer" else "not-allowed",
			bg = function()
				if selected() then
					return theme.color.modifier_selected
				end
				return if hovered:get() then theme.color.modifier_hover else nil
			end,
			on_hover = function(inside: boolean)
				hovered:set(inside)
			end,
			on_click = if selectable then actions.select_channel(row.id) else nil,
			context_menu_below = true,
			context_menu_style = menu_style,
			context_menu = function()
				return channel_menu(row)
			end,
			ui.show(function()
				return row.unread == true and not selected()
			end, ui.div {
				absolute = true,
				left = PILL_LEFT,
				top = PILL_TOP,
				w = PILL_WIDTH,
				h = PILL_HEIGHT,
				rounded_tr = PILL_WIDTH,
				rounded_br = PILL_WIDTH,
				bg = theme.color.unread_pill,
			}),
			glyph(row, glyph_color),
			ui.text {
				row.name,
				literal = true,
				grow = 1,
				min_w = 0,
				truncate = true,
				color = rest,
				weight = theme.weight.semibold,
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
		ui.column {
			w_full = true,
			table.unpack(members),
		},
	}
end)

local function sidebar_row(row: state.SidebarRow)
	if row.kind == "category" then
		return category { row = row }
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
		bg = theme.color.background_secondary,
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
