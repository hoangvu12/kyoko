local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")
local avatar = require("@components/avatar")
local mention_badge = require("@components/mention_badge")

local TAB_HEIGHT = 28
local DIVIDER_HEIGHT = 20
local ROW_AVATAR = 36
local ACTIVE_AVATAR = 40
local CIRCLE = 36
local CIRCLE_ICON = 20
local INPUT_HEIGHT = 44
local BUTTON_HEIGHT = 32
local EMPTY_ICON = 80
local MENU_WIDTH = theme.size.context_menu_width
local LINE_TINY = 19.4
local LINE_SM = 22.65
local LINE_MD = 25.9
local LINE_LG = 32.4

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

local tab_button = ui.component(function(props: { key: string, label: string })
	local hovered = ui.use_state(false)
	local add = props.key == "add"
	local function selected(): boolean
		return on_tab(props.key)
	end
	local children: { any } = {
		ui.text {
			props.label,
			color = function()
				if add then
					return if selected() then theme.color.brand else theme.color.text_on_brand
				end
				if selected() then
					return theme.color.interactive_active
				end
				return if hovered:get() then theme.color.interactive_hover else theme.color.interactive_normal
			end,
		},
	}
	if props.key == "pending" then
		local function count(): number
			return state.friends.pending:get()
		end
		table.insert(children, ui.show(function()
			return count() > 0
		end, mention_badge { count = count }))
	end
	return ui.row {
		id = `friends_tab_{props.key}`,
		h = TAB_HEIGHT,
		px = theme.size.space_8,
		gap = theme.size.space_8,
		shrink = 0,
		items = "center",
		rounded = theme.size.radius_sm,
		cursor = "pointer",
		text_size = theme.size.font_md,
		line_height = LINE_MD,
		weight = theme.weight.medium,
		bg = function()
			if add then
				return if selected() then nil else theme.color.brand
			end
			if selected() then
				return theme.color.modifier_selected
			end
			return if hovered:get() then theme.color.modifier_hover else nil
		end,
		on_hover = function(inside: boolean)
			hovered:set(inside)
		end,
		on_click = actions.friends_tab(props.key),
		table.unpack(children),
	}
end)

local function header()
	local tabs = {}
	for _, tab in TABS do
		table.insert(tabs, tab_button { key = tab.key, label = tab.label })
	end
	table.insert(tabs, tab_button { key = "add", label = "Add Friend" })
	return ui.row {
		w_full = true,
		h = theme.size.top_bar_height,
		shrink = 0,
		px = theme.size.space_16,
		gap = theme.size.space_16,
		items = "center",
		border_sides = { 0, 0, 1, 0 },
		border_color = theme.color.background_tertiary,
		ui.row {
			gap = theme.size.space_8,
			shrink = 0,
			items = "center",
			icon { name = "friends", size = 24, color = theme.color.text_muted },
			ui.text {
				"Friends",
				text_size = theme.size.font_md,
				line_height = LINE_MD,
				weight = theme.weight.semibold,
				color = theme.color.header_primary,
			},
		},
		ui.div { w = 1, h = DIVIDER_HEIGHT, shrink = 0, bg = theme.color.divider },
		ui.row { gap = theme.size.space_8, items = "center", table.unpack(tabs) },
	}
end

local circle = ui.component(function(props: { id: string, icon: string, hint: string, hover: string, on_click: any })
	local hovered = ui.use_state(false)
	return ui.button {
		id = props.id,
		w = CIRCLE,
		h = CIRCLE,
		shrink = 0,
		p = 0,
		border = 0,
		focus = { border = 0 },
		rounded = CIRCLE,
		justify = "center",
		items = "center",
		cursor = "pointer",
		bg = theme.color.background_secondary,
		hover = { bg = theme.color.background_secondary },
		tooltip = props.hint,
		on_hover = function(inside: boolean)
			hovered:set(inside)
		end,
		on_click = props.on_click,
		icon {
			name = props.icon,
			size = CIRCLE_ICON,
			color = function()
				return if hovered:get() then props.hover else theme.color.interactive_normal
			end,
		},
	}
end)

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

local more_button = ui.component(function(props: { id: string })
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
			hover = theme.color.interactive_hover,
			on_click = ui.toggle(open),
		},
		menu_entry(`friend_remove_{id}`, "Remove Friend", "trash", theme.color.danger, function()
			actions.friends_act("remove", id)
		end),
		menu_entry(`friend_block_{id}`, "Block", "shield", theme.color.danger, function()
			actions.friends_act("block", id)
		end),
		ui.div { h = 1, my = 4, mx = 4, bg = theme.color.modifier_hover },
		menu_entry(`friend_copy_{id}`, "Copy User ID", "copy", theme.color.text_normal, function()
			actions.friends_act("copy_id", id)
		end),
	}
end)

local function row_actions(row: state.FriendRow)
	local id = row.id
	local function act(name: string): () -> ()
		return function()
			actions.friends_act(name, id)
		end
	end
	if row.relation == "friend" then
		return {
			circle {
				id = `friend_message_{id}`,
				icon = "chat",
				hint = "Message",
				hover = theme.color.interactive_hover,
				on_click = act("message"),
			},
			more_button { id = id },
		}
	elseif row.relation == "incoming" then
		return {
			circle { id = `friend_accept_{id}`, icon = "check", hint = "Accept", hover = theme.color.success, on_click = act("accept") },
			circle { id = `friend_ignore_{id}`, icon = "close", hint = "Ignore", hover = theme.color.danger, on_click = act("remove") },
		}
	elseif row.relation == "outgoing" then
		return {
			circle { id = `friend_cancel_{id}`, icon = "close", hint = "Cancel", hover = theme.color.danger, on_click = act("remove") },
		}
	end
	return {
		circle { id = `friend_unblock_{id}`, icon = "close", hint = "Unblock", hover = theme.color.danger, on_click = act("remove") },
	}
end

local person = ui.component(function(props: { row: state.FriendRow })
	local row = props.row
	local hovered = ui.use_state(false)
	return ui.row {
		id = `friend_row_{row.id}`,
		w_full = true,
		h = theme.size.friends_row_height,
		px = theme.size.space_8,
		gap = theme.size.space_12,
		shrink = 0,
		items = "center",
		rounded = theme.size.radius_sm,
		border_sides = { 1, 0, 0, 0 },
		border_color = theme.color.divider,
		cursor = "pointer",
		bg = function()
			return if hovered:get() then theme.color.modifier_hover else nil
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
			avatar {
				src = row.avatar,
				size = ROW_AVATAR,
				status = if row.relation == "friend" then row.status else nil,
				cutout = theme.color.background_primary,
			},
			ui.column {
				grow = 1,
				min_w = 0,
				ui.row {
					w_full = true,
					min_w = 0,
					gap = theme.size.space_8,
					items = "baseline",
					ui.text {
						row.name,
						literal = true,
						min_w = 0,
						truncate = true,
						text_size = theme.size.font_md,
						line_height = LINE_MD,
						weight = theme.weight.semibold,
						color = theme.color.header_primary,
					},
					ui.text {
						row.username,
						literal = true,
						min_w = 0,
						truncate = true,
						text_size = theme.size.font_sm,
						line_height = LINE_SM,
						color = theme.color.header_secondary,
						opacity = function()
							return if hovered:get() then 1 else 0
						end,
					},
				},
				ui.text {
					row.subtitle,
					literal = true,
					w_full = true,
					min_w = 0,
					truncate = true,
					text_size = theme.size.font_sm,
					line_height = LINE_SM,
					color = theme.color.text_muted,
				},
			},
		},
		ui.row { gap = theme.size.space_8, shrink = 0, items = "center", table.unpack(row_actions(row)) },
	}
end)

local function section_title(row: state.FriendRow)
	return ui.div {
		w_full = true,
		px = theme.size.space_8,
		pt = theme.size.space_16,
		pb = theme.size.space_8,
		ui.text {
			row.title,
			text_size = theme.size.font_tiny,
			line_height = LINE_TINY,
			weight = theme.weight.bold,
			color = theme.color.header_secondary,
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
		icon { name = "friends", size = EMPTY_ICON, color = theme.color.interactive_muted },
		ui.text {
			function()
				return state.friends.empty:get()
			end,
			text_size = theme.size.font_md,
			line_height = LINE_MD,
			color = theme.color.text_muted,
		},
	}
end

local function field(id: string, value: any, placeholder: string, on_key: ((event: any) -> boolean?)?)
	return ui.input {
		id = id,
		w_full = true,
		h = INPUT_HEIGHT,
		px = theme.size.space_12,
		rounded = theme.size.radius_sm,
		border = 1,
		border_color = theme.color.interactive_muted,
		bg = theme.color.input_background,
		focus = { border_color = theme.color.brand },
		text_size = theme.size.font_md,
		line_height = LINE_MD,
		color = theme.color.text_normal,
		placeholder = placeholder,
		placeholder_color = theme.color.text_muted,
		value = value,
		on_key = on_key,
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
		ui.div { w_full = true, shrink = 0, p = theme.size.space_16, field("friends_search", query, "Search") },
		ui.column {
			grow = 1,
			min_h = 0,
			w_full = true,
			px = theme.size.space_16,
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
		},
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
	return ui.column {
		w_full = true,
		gap = theme.size.space_16,
		p = theme.size.space_24,
		ui.column {
			w_full = true,
			gap = theme.size.space_8,
			ui.text {
				"ADD FRIEND",
				text_size = theme.size.font_md,
				line_height = LINE_MD,
				weight = theme.weight.bold,
				color = theme.color.header_primary,
			},
			ui.text {
				"You can add friends with their Discord username.",
				text_size = theme.size.font_sm,
				line_height = LINE_SM,
				color = theme.color.header_secondary,
			},
		},
		ui.row {
			w_full = true,
			gap = theme.size.space_12,
			items = "center",
			ui.div {
				grow = 1,
				min_w = 0,
				field("friends_add_input", name, "Enter a username", function(event: any): boolean?
					if event.key == "Enter" then
						send()
						return true
					end
					return nil
				end),
			},
			ui.button {
				"Send Friend Request",
				id = "friends_add_send",
				h = BUTTON_HEIGHT,
				px = theme.size.space_12,
				shrink = 0,
				border = 0,
				focus = { border = 0 },
				rounded = theme.size.radius_sm,
				cursor = "pointer",
				bg = theme.color.brand,
				hover = { bg = theme.color.brand_hover },
				color = theme.color.text_on_brand,
				text_size = theme.size.font_md,
				line_height = LINE_MD,
				weight = theme.weight.medium,
				opacity = function()
					return if phase() == "sending" then 0.6 else 1
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
			text_size = theme.size.font_sm,
			line_height = LINE_SM,
			color = function()
				return if phase() == "failed" then theme.color.danger else theme.color.success
			end,
		}),
	}
end)

local function active_friend(row: state.FriendRow)
	return ui.row {
		id = `active_friend_{row.id}`,
		w_full = true,
		p = theme.size.space_8,
		gap = theme.size.space_12,
		items = "center",
		rounded = theme.size.radius_sm,
		cursor = "pointer",
		hover = { bg = theme.color.modifier_hover },
		on_press = function(point: any)
			actions.open_friend_profile(row.id, point.x, point.y)
		end,
		avatar { src = row.avatar, size = ACTIVE_AVATAR, status = row.status, cutout = theme.color.background_primary },
		ui.column {
			min_w = 0,
			ui.text {
				row.name,
				literal = true,
				min_w = 0,
				truncate = true,
				text_size = theme.size.font_md,
				line_height = LINE_MD,
				weight = theme.weight.semibold,
				color = theme.color.header_primary,
			},
			ui.text {
				row.subtitle,
				literal = true,
				min_w = 0,
				truncate = true,
				text_size = theme.size.font_sm,
				line_height = LINE_SM,
				color = theme.color.text_muted,
			},
		},
	}
end

local function active_now()
	local function quiet(): boolean
		return #state.friends.active:get() == 0
	end
	return ui.column {
		w = theme.size.friends_active_width,
		h_full = true,
		shrink = 0,
		p = theme.size.space_16,
		gap = theme.size.space_16,
		border_sides = { 0, 0, 0, 1 },
		border_color = theme.color.background_tertiary,
		ui.text {
			"Active Now",
			text_size = theme.size.font_lg,
			line_height = LINE_LG,
			weight = theme.weight.bold,
			color = theme.color.header_primary,
		},
		ui.scroll {
			grow = 1,
			min_h = 0,
			w_full = true,
			ui.show(quiet, ui.column {
				w_full = true,
				gap = theme.size.space_4,
				p = theme.size.space_12,
				ui.text {
					"It's quiet for now...",
					text_size = theme.size.font_md,
					line_height = LINE_MD,
					weight = theme.weight.semibold,
					color = theme.color.header_primary,
				},
				ui.text {
					"When a friend starts an activity, like playing a game or hanging out in voice, we'll show it here.",
					w_full = true,
					wrap = true,
					text_size = theme.size.font_sm,
					line_height = LINE_SM,
					color = theme.color.text_muted,
				},
			}),
			ui.show(function()
				return not quiet()
			end, ui.each(state.friends.active, active_friend, {
				key = by_key,
				w_full = true,
				gap = theme.size.space_4,
			})),
		},
	}
end

return function()
	return ui.column {
		absolute = true,
		left = 0,
		top = 0,
		right = 0,
		bottom = 0,
		bg = theme.color.background_primary,
		header(),
		ui.row {
			grow = 1,
			min_h = 0,
			w_full = true,
			ui.column {
				grow = 1,
				min_w = 0,
				h_full = true,
				ui.show(function()
					return not on_tab("add")
				end, list()),
				ui.show(function()
					return on_tab("add")
				end, add_form()),
			},
			active_now(),
		},
	}
end
