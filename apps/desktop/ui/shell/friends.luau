local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")
local icon_button = require("@components/icon_button")
local avatar = require("@components/avatar")

local EMPTY_ICON = 80
local MENU_WIDTH = theme.size.context_menu_width

local icon_lg = theme.number(theme.size.icon_size_lg)
local icon_md = theme.number(theme.size.icon_size_md)
local avatar_sm = theme.number(theme.size.avatar_size_sm)
local button = theme.number(theme.size.button_size)
local round = theme.number(theme.size.friends_button_size)

local TABS = {
	{ key = "online", label = "Online" },
	{ key = "all", label = "All" },
	{ key = "pending", label = "Pending" },
	{ key = "blocked", label = "Blocked" },
}

local function by_key(item: { key: string })
	return item.key
end

local function on_tab(key: string): boolean
	return state.friends.tab:get() == key
end

local function pending_badge()
	local function count(): number
		return state.friends.pending:get()
	end
	return ui.show(function()
		return count() > 0
	end, ui.row {
		min_w = theme.size.badge_size,
		h = theme.size.badge_size,
		px = theme.size.space_4,
		shrink = 0,
		justify = "center",
		items = "center",
		rounded = theme.size.badge_size,
		bg = theme.color.accent_new,
		ui.text {
			function()
				return tostring(count())
			end,
			text_size = theme.size.font_size_xs,
			line_height = theme.size.badge_size,
			weight = theme.weight.bold,
			color = theme.color.text_0,
		},
	})
end

local tab_button = ui.component(function(props: { key: string, label: string })
	local hovered = ui.use_state(false)
	local function selected(): boolean
		return on_tab(props.key)
	end
	return ui.row {
		id = `friends_tab_{props.key}`,
		h = theme.size.button_height_sm,
		px = theme.size.space_12,
		gap = theme.size.space_6,
		shrink = 0,
		items = "center",
		rounded = theme.size.radius_sm,
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
		on_click = actions.friends_tab(props.key),
		ui.text {
			props.label,
			text_size = theme.size.font_size_md,
			line_height = theme.size.line_height_label,
			weight = theme.weight.medium,
			color = function()
				if selected() then
					return theme.color.text_2
				end
				return if hovered:get() then theme.color.text_3 else theme.color.text_4
			end,
		},
		if props.key == "pending" then pending_badge() else nil,
	}
end)

local add_tab = ui.component(function()
	local hovered = ui.use_state(false)
	return ui.row {
		id = "friends_tab_add",
		h = theme.size.button_height_sm,
		px = theme.size.space_12,
		ml = theme.size.space_8,
		shrink = 0,
		items = "center",
		rounded = theme.size.radius_sm,
		border = theme.size.border_thickness,
		border_color = theme.color.button_border,
		cursor = "pointer",
		bg = function()
			if on_tab("add") then
				return theme.color.accent_5
			end
			return if hovered:get() then theme.color.green_4 else theme.color.green_2
		end,
		on_hover = function(inside: boolean)
			hovered:set(inside)
		end,
		on_click = actions.friends_tab("add"),
		ui.text {
			"Add Friend",
			text_size = theme.size.font_size_sm,
			line_height = theme.size.line_height_sm,
			weight = theme.weight.medium,
			color = theme.color.text_0,
		},
	}
end)

local function header()
	local tabs = {}
	for _, tab in TABS do
		table.insert(tabs, tab_button { key = tab.key, label = tab.label })
	end
	return ui.row {
		w_full = true,
		h_full = true,
		items = "center",
		pl = theme.size.space_16,
		ui.row {
			grow = 1,
			min_w = 0,
			gap = theme.size.space_8,
			items = "center",
			icon { name = "friends", size = icon_lg(), color = theme.color.text_4 },
			ui.text {
				"Friends",
				shrink = 0,
				text_size = theme.size.font_size_md,
				line_height = theme.size.line_height_label,
				weight = theme.weight.semibold,
				color = theme.color.text_2,
			},
			ui.div {
				w = theme.size.space_4,
				h = theme.size.space_4,
				mx = theme.size.space_4,
				shrink = 0,
				rounded = theme.size.space_4,
				bg = theme.color.text_5,
			},
			ui.row { shrink = 0, items = "center", table.unpack(tabs) },
			add_tab {},
		},
		ui.row {
			shrink = 0,
			items = "center",
			pr = theme.size.space_12,
			icon_button {
				name = "chat_plus",
				id = "friends_new_group",
				tooltip = "New Group DM",
				hit_size = button(),
				icon_size = icon_md(),
				on_click = actions.dm_action("create_group"),
			},
		},
		ui.row {
			shrink = 0,
			items = "center",
			gap = theme.size.space_8,
			icon_button {
				name = "inbox",
				id = "friends_inbox",
				tooltip = "Inbox",
				hit_size = button(),
				icon_size = icon_md(),
				on_press = function(point: any)
					actions.open_inbox(point.x, point.y)
				end,
			},
			ui.div { w = theme.size.window_controls_width, h_full = true, shrink = 0 },
		},
	}
end

local circle = ui.component(
	function(props: { id: string, icon: string, hint: string, tint: string?, lifted: () -> boolean, on_click: any })
		return ui.button {
			id = props.id,
			w = round(),
			h = round(),
			shrink = 0,
			p = 0,
			border = 0,
			focus = { border = 0 },
			rounded = round(),
			justify = "center",
			items = "center",
			cursor = "pointer",
			bg = function()
				return if props.lifted() then theme.color.bg_4 else theme.color.bg_3
			end,
			hover = { bg = theme.color.bg_4 },
			tooltip = props.hint,
			on_click = props.on_click,
			icon {
				name = props.icon,
				size = icon_md(),
				color = function()
					if props.tint then
						return props.tint
					end
					return if props.lifted() then theme.color.text_3 else theme.color.text_4
				end,
			},
		}
	end
)

local function menu_entry(id: string, label: string, glyph: string, color: string, on_click: () -> ())
	return ui.menu_item {
		id = id,
		color = color,
		on_click = on_click,
		ui.row {
			w_full = true,
			items = "center",
			gap = theme.size.space_8,
			ui.text { label, grow = 1, min_w = 0, truncate = true, color = color },
			icon { name = glyph, size = 16, color = color },
		},
	}
end

local more_button = ui.component(function(props: { id: string, lifted: () -> boolean })
	local open = ui.use_state(false)
	local id = props.id
	return ui.menu {
		open = open,
		label = "More",
		w = MENU_WIDTH,
		px = theme.size.space_8,
		py = theme.size.space_6,
		gap = theme.size.space_2,
		rounded = theme.size.radius_sm,
		bg = theme.color.bg_3,
		border = theme.size.border_thickness,
		border_color = theme.color.border,
		placement = { side = "bottom", align = "end", offset = 4 },
		anchor = circle {
			id = `friend_more_{id}`,
			icon = "more",
			hint = "More",
			lifted = props.lifted,
			on_click = ui.toggle(open),
		},
		menu_entry(`friend_remove_{id}`, "Remove Friend", "trash", theme.color.danger, function()
			actions.friends_act("ask_remove", id)
		end),
		menu_entry(`friend_block_{id}`, "Block", "shield", theme.color.danger, function()
			actions.friends_act("ask_block", id)
		end),
		ui.div { h = 1, my = 4, mx = 4, bg = theme.color.modifier_hover },
		menu_entry(`friend_copy_{id}`, "Copy User ID", "copy", theme.color.text_normal, function()
			actions.friends_act("copy_id", id)
		end),
	}
end)

local function row_actions(row: state.FriendRow, lifted: () -> boolean)
	local id = row.id
	local function act(name: string): () -> ()
		return function()
			actions.friends_act(name, id)
		end
	end
	if row.relation == "friend" then
		return {
			circle { id = `friend_message_{id}`, icon = "chat", hint = "Message", lifted = lifted, on_click = act("message") },
			more_button { id = id, lifted = lifted },
		}
	elseif row.relation == "incoming" then
		return {
			circle {
				id = `friend_accept_{id}`,
				icon = "check",
				hint = "Accept",
				tint = theme.color.green_2,
				lifted = lifted,
				on_click = act("accept"),
			},
			circle {
				id = `friend_ignore_{id}`,
				icon = "close",
				hint = "Ignore",
				tint = theme.color.red_2,
				lifted = lifted,
				on_click = act("remove"),
			},
		}
	elseif row.relation == "outgoing" then
		return {
			circle {
				id = `friend_cancel_{id}`,
				icon = "close",
				hint = "Cancel",
				tint = theme.color.red_2,
				lifted = lifted,
				on_click = act("remove"),
			},
		}
	end
	return {
		circle {
			id = `friend_unblock_{id}`,
			icon = "user_x",
			hint = "Unblock",
			tint = theme.color.red_2,
			lifted = lifted,
			on_click = act("remove"),
		},
	}
end

local person = ui.component(function(props: { row: state.FriendRow })
	local row = props.row
	local hovered = ui.use_state(false)
	local function lifted(): boolean
		return hovered:get()
	end
	local friend = row.relation == "friend"
	return ui.row {
		id = `friend_row_{row.id}`,
		w_full = true,
		h = theme.size.friends_row_height,
		px = theme.size.space_10,
		gap = theme.size.space_12,
		shrink = 0,
		items = "center",
		rounded = theme.size.radius_sm,
		border_sides = { 1, 0, 0, 0 },
		border_color = function()
			return if hovered:get() then "#00000000" else theme.color.border
		end,
		cursor = "pointer",
		bg = function()
			return if hovered:get() then theme.color.hover else nil
		end,
		on_hover = function(inside: boolean)
			hovered:set(inside)
		end,
		ui.row {
			id = `friend_open_{row.id}`,
			grow = 1,
			min_w = 0,
			h_full = true,
			gap = theme.size.space_12,
			items = "center",
			on_press = function(point: any)
				actions.open_friend_profile(row.id, point.x, point.y)
			end,
			avatar { src = row.avatar, size = avatar_sm(), status = row.status, cutout = theme.color.bg_4 },
			ui.column {
				grow = 1,
				min_w = 0,
				ui.row {
					w_full = true,
					min_w = 0,
					gap = theme.size.space_6,
					items = "baseline",
					ui.text {
						row.name,
						literal = true,
						min_w = 0,
						truncate = true,
						text_size = theme.size.font_size_md,
						line_height = theme.size.line_height_label,
						weight = theme.weight.semibold,
						color = theme.color.text_2,
					},
					ui.text {
						row.username,
						literal = true,
						min_w = 0,
						truncate = true,
						text_size = theme.size.font_size_sm,
						line_height = theme.size.line_height_sm,
						color = theme.color.text_4,
						opacity = function()
							return if not friend or hovered:get() then 1 else 0
						end,
					},
				},
				ui.text {
					row.subtitle,
					literal = true,
					w_full = true,
					min_w = 0,
					truncate = true,
					text_size = theme.size.font_size_sm,
					line_height = theme.size.line_height_sm,
					color = theme.color.text_4,
				},
			},
		},
		ui.row { gap = theme.size.space_10, shrink = 0, items = "center", table.unpack(row_actions(row, lifted)) },
	}
end)

local function section_title(row: state.FriendRow)
	return ui.div {
		w_full = true,
		px = theme.size.space_10,
		pt = theme.size.space_24,
		pb = theme.size.space_12,
		ui.text {
			row.title,
			literal = true,
			text_size = theme.size.font_size_sm,
			line_height = theme.size.line_height_sm,
			weight = theme.weight.medium,
			color = theme.color.text_4,
		},
	}
end

local function empty()
	return ui.column {
		grow = 1,
		w_full = true,
		gap = theme.size.space_16,
		items = "center",
		justify = "center",
		icon { name = "friends", size = EMPTY_ICON, color = theme.color.text_5 },
		ui.text {
			function()
				return state.friends.empty:get()
			end,
			text_size = theme.size.font_size_md,
			line_height = theme.size.line_height_md,
			color = theme.color.text_4,
		},
	}
end

local function search_field(query: any)
	return ui.row {
		w_full = true,
		h = theme.size.input_height,
		shrink = 0,
		px = theme.size.space_12,
		gap = theme.size.space_8,
		items = "center",
		rounded = theme.size.radius_sm,
		bg = theme.color.bg_3,
		border = theme.size.border_thickness,
		border_color = theme.color.border_light,
		ui.input {
			id = "friends_search",
			grow = 1,
			min_w = 0,
			h_full = true,
			p = 0,
			bg = "#00000000",
			border = 0,
			focus = { border = 0 },
			text_size = theme.size.font_size_md,
			line_height = theme.size.line_height_label,
			color = theme.color.text_3,
			placeholder = "Search",
			placeholder_color = theme.color.text_5,
			value = query,
		},
		icon { name = "search", size = icon_md(), color = theme.color.text_4 },
	}
end

local list = ui.component(function()
	local query = ui.use_state("")
	ui.effect(function()
		actions.friends_filter(query:get())
	end)
	return ui.column {
		grow = 1,
		min_h = 0,
		w_full = true,
		pt = theme.size.space_16,
		px = theme.size.space_10,
		ui.div { w_full = true, shrink = 0, px = theme.size.space_10, search_field(query) },
		ui.show(function()
			return state.friends.empty:get() ~= ""
		end, empty()),
		ui.show(function()
			return state.friends.empty:get() == ""
		end, ui.scroll {
			grow = 1,
			min_h = 0,
			w_full = true,
			ui.each(state.friends.rows, function(row: state.FriendRow)
				if row.kind == "title" then
					return section_title(row)
				end
				return person { row = row }
			end, { key = by_key, w_full = true }),
		}),
	}
end)

local add_form = ui.component(function()
	local name = ui.use_state("")
	local function send()
		local text = name:get()
		if string.match(text, "%S") then
			actions.add_friend(text)
		end
	end
	local function phase(): string
		return (state.friends.add:get() :: any).phase
	end
	local focused = ui.use_state(false)
	local function typed(): boolean
		return string.match(name:get(), "%S") ~= nil
	end
	return ui.column {
		w_full = true,
		pt = theme.size.space_16,
		px = theme.size.space_20,
		ui.column {
			w_full = true,
			gap = theme.size.space_8,
			pb = theme.size.space_24,
			border_sides = { 0, 0, 1, 0 },
			border_color = theme.color.border,
			ui.text {
				"Add Friend",
				text_size = theme.size.font_size_lg,
				line_height = theme.size.line_height_lg,
				weight = theme.weight.semibold,
				color = theme.color.text_2,
			},
			ui.text {
				"You can add friends with their Discord username.",
				text_size = theme.size.font_size_sm,
				line_height = theme.size.line_height_sm,
				color = theme.color.text_4,
			},
			ui.row {
				w_full = true,
				h = theme.size.add_friend_input_height,
				mt = theme.size.space_8,
				pl = theme.size.space_12,
				pr = theme.size.space_8,
				gap = theme.size.space_8,
				items = "center",
				rounded = theme.size.radius_sm,
				bg = theme.color.bg_3,
				border = theme.size.border_thickness,
				border_color = function()
					return if focused:get() then theme.color.accent_2 else theme.color.border_light
				end,
				ui.input {
					id = "friends_add_input",
					grow = 1,
					min_w = 0,
					h_full = true,
					p = 0,
					bg = "#00000000",
					border = 0,
					focus = { border = 0 },
					text_size = theme.size.font_size_md,
					line_height = theme.size.line_height_label,
					color = theme.color.text_2,
					placeholder = "Enter a username",
					placeholder_color = theme.color.text_5,
					value = name,
					on_focus = function()
						focused:set(true)
					end,
					on_blur = function()
						focused:set(false)
					end,
					on_key = function(event: any): boolean?
						if event.key == "Enter" then
							send()
							return true
						end
						return nil
					end,
				},
				ui.button {
					"Send Friend Request",
					id = "friends_add_send",
					h = theme.size.button_height_sm,
					px = theme.size.space_16,
					shrink = 0,
					border = theme.size.border_thickness,
					border_color = theme.color.button_border,
					focus = { border_color = theme.color.button_border },
					rounded = theme.size.radius_sm,
					cursor = "pointer",
					bg = theme.color.accent_3,
					hover = { bg = theme.color.accent_4 },
					color = theme.color.text_0,
					text_size = theme.size.font_size_sm,
					line_height = theme.size.line_height_label,
					weight = theme.weight.medium,
					opacity = function()
						return if typed() and phase() ~= "sending" then 1 else theme.number(theme.size.disabled_opacity_control)()
					end,
					on_click = send,
				},
			},
			ui.show(function()
				return phase() == "sent" or phase() == "failed"
			end, ui.text {
				function()
					return (state.friends.add:get() :: any).text
				end,
				literal = true,
				w_full = true,
				wrap = true,
				text_size = theme.size.font_size_sm,
				line_height = theme.size.line_height_sm,
				weight = theme.weight.medium,
				color = function()
					return if phase() == "failed" then theme.color.red_2 else theme.color.green_1
				end,
			}),
		},
		ui.text {
			"Other places to make friends",
			mt = theme.size.space_24,
			text_size = theme.size.font_size_lg,
			line_height = theme.size.line_height_lg,
			weight = theme.weight.semibold,
			color = theme.color.text_2,
		},
		ui.row {
			id = "friends_explore",
			w_full = true,
			mt = theme.size.space_12,
			px = theme.size.space_16,
			py = theme.size.space_12,
			gap = theme.size.space_12,
			items = "center",
			rounded = theme.size.radius_sm,
			bg = theme.color.bg_3,
			border = theme.size.border_thickness,
			border_color = theme.color.border,
			cursor = "pointer",
			hover = { bg = theme.color.hover },
			on_click = actions.discover(),
			ui.row {
				w = theme.size.avatar_size_md,
				h = theme.size.avatar_size_md,
				shrink = 0,
				justify = "center",
				items = "center",
				rounded = theme.size.radius_sm,
				bg = theme.color.green_3,
				icon { name = "compass", size = icon_lg(), color = theme.color.text_0 },
			},
			ui.text {
				"Explore Discoverable Servers",
				grow = 1,
				min_w = 0,
				truncate = true,
				text_size = theme.size.font_size_md,
				line_height = theme.size.line_height_label,
				weight = theme.weight.semibold,
				color = theme.color.text_2,
			},
			icon { name = "chevron_right", size = icon_md(), color = theme.color.text_4 },
		},
	}
end)

local function activity_box(row: state.FriendRow)
	local details = row.details or ""
	local status = row.state or ""
	if details == "" and status == "" then
		return nil
	end
	return ui.column {
		w_full = true,
		p = theme.size.space_12,
		rounded = theme.size.radius_sm,
		bg = theme.color.bg_4,
		if details ~= ""
			then ui.text {
				details,
				literal = true,
				w_full = true,
				truncate = true,
				text_size = theme.size.font_size_sm,
				line_height = theme.size.line_height_sm,
				color = theme.color.text_3,
			}
			else nil,
		if status ~= ""
			then ui.text {
				status,
				literal = true,
				w_full = true,
				truncate = true,
				text_size = theme.size.font_size_sm,
				line_height = theme.size.line_height_sm,
				color = theme.color.text_4,
			}
			else nil,
	}
end

local function active_card(row: state.FriendRow)
	return ui.column {
		id = `active_friend_{row.id}`,
		w_full = true,
		p = theme.size.space_16,
		gap = theme.size.space_12,
		rounded = theme.size.radius_sm,
		bg = theme.color.bg_3,
		border = theme.size.border_thickness,
		border_color = theme.color.border,
		cursor = "pointer",
		on_press = function(point: any)
			actions.open_friend_profile(row.id, point.x, point.y)
		end,
		ui.row {
			w_full = true,
			gap = theme.size.space_12,
			items = "center",
			avatar { src = row.avatar, size = avatar_sm(), status = row.status, cutout = theme.color.bg_3 },
			ui.column {
				grow = 1,
				min_w = 0,
				ui.text {
					row.name,
					literal = true,
					min_w = 0,
					truncate = true,
					text_size = theme.size.font_size_md,
					line_height = theme.size.line_height_label,
					weight = theme.weight.semibold,
					color = theme.color.text_2,
				},
				ui.text {
					row.subtitle,
					literal = true,
					min_w = 0,
					truncate = true,
					text_size = theme.size.font_size_sm,
					line_height = theme.size.line_height_sm,
					color = theme.color.text_4,
				},
			},
		},
		activity_box(row),
	}
end

local function active_now()
	local function quiet(): boolean
		return #state.friends.active:get() == 0
	end
	return ui.column {
		w_full = true,
		h_full = true,
		p = theme.size.space_16,
		gap = theme.size.space_16,
		ui.text {
			"Active Now",
			text_size = theme.size.font_size_lg,
			line_height = theme.size.line_height_lg,
			weight = theme.weight.bold,
			color = theme.color.text_2,
		},
		ui.scroll {
			grow = 1,
			min_h = 0,
			w_full = true,
			ui.show(quiet, ui.column {
				w_full = true,
				gap = theme.size.space_4,
				items = "center",
				pt = theme.size.space_16,
				ui.text {
					"It's quiet for now...",
					text_size = theme.size.font_size_md,
					line_height = theme.size.line_height_label,
					weight = theme.weight.semibold,
					color = theme.color.text_2,
				},
				ui.text {
					"When a friend starts an activity, like playing a game or hanging out in voice, we'll show it here.",
					w_full = true,
					wrap = true,
					align = "center",
					text_size = theme.size.font_size_sm,
					line_height = theme.size.line_height_sm,
					color = theme.color.text_4,
				},
			}),
			ui.show(function()
				return not quiet()
			end, ui.each(state.friends.active, active_card, {
				key = by_key,
				w_full = true,
				gap = theme.size.space_16,
			})),
		},
	}
end

local function content()
	return ui.column {
		w_full = true,
		h_full = true,
		ui.show(function()
			return not on_tab("add")
		end, list()),
		ui.show(function()
			return on_tab("add")
		end, add_form()),
	}
end

return {
	header = header,
	content = content,
	active_now = active_now,
}
