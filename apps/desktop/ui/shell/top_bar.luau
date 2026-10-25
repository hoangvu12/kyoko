local theme = require("../theme")
local state = require("../state")
local icon = require("@components/icon")
local icon_button = require("@components/icon_button")
local status_dot = require("@components/status_dot")
local actions = require("../actions")

local SEARCH_WIDTH = 144
local SEARCH_FOCUSED_WIDTH = 240
local SEARCH_SLIDE = 180
local SEARCH_HEIGHT = 24
local TITLE_ICON = 24
local SEARCH_ICON = 16
local STATUS_DOT = 10
local STATUSES = { "online", "idle", "dnd", "offline" }

local query = ui.state("query", "")
local searching = ui.state("top_bar.search_focused", false)

local function topic()
	return ui.show(function()
		return state.timeline.channel_topic:get() ~= ""
	end, ui.row {
		grow = 1,
		min_w = 0,
		items = "center",
		gap = theme.size.space_8,
		ui.div { w = 1, h = 24, shrink = 0, bg = theme.color.divider },
		ui.text {
			function()
				return state.timeline.channel_topic:get()
			end,
			literal = true,
			grow = 1,
			min_w = 0,
			truncate = true,
			text_size = theme.size.font_sm,
			color = theme.color.text_muted,
		},
	})
end

local function direct(): boolean
	return state.sidebar.guild_id:get() == ""
end

local function guild(): boolean
	return not direct()
end

local function direct_status(): string
	local open = state.timeline.channel_id:get()
	if not direct() or open == "" then
		return ""
	end
	for _, row in state.dms.rows:get() do
		if row.id == open then
			return if row.group then "offline" else row.status
		end
	end
	return ""
end

local function status()
	local dots = {}
	for _, name in STATUSES do
		table.insert(
			dots,
			ui.show(function()
				return direct_status() == name
			end, status_dot { status = name, size = STATUS_DOT, cutout = theme.color.background_primary })
		)
	end
	return ui.row { shrink = 0, items = "center", table.unpack(dots) }
end

local function search()
	return ui.row {
		w = ui.tween(searching, { from = SEARCH_WIDTH, to = SEARCH_FOCUSED_WIDTH, duration = SEARCH_SLIDE, ease = "outQuint" }),
		h = SEARCH_HEIGHT,
		shrink = 0,
		items = "center",
		pl = theme.size.space_4,
		pr = theme.size.space_8,
		rounded = theme.size.radius_sm,
		bg = theme.color.background_tertiary,
		ui.input {
			grow = 1,
			min_w = 0,
			h = SEARCH_HEIGHT,
			px = 8,
			bg = "#00000000",
			border = 0,
			focus = { border = 0 },
			text_size = theme.size.font_sm,
			color = theme.color.text_normal,
			placeholder = "Search",
			placeholder_color = theme.color.text_muted,
			value = query,
			on_focus = function()
				searching:set(true)
			end,
			on_blur = function()
				searching:set(false)
			end,
			on_key = function(event: any): boolean?
				if event.key == "Enter" then
					local text = query:get()
					if string.match(text, "%S") then
						actions.run_search(text)
					end
					return true
				end
				return nil
			end,
		},
		icon { name = "search", size = SEARCH_ICON, color = theme.color.text_muted },
	}
end

return function()
	return ui.row {
		w_full = true,
		h = theme.size.top_bar_height,
		shrink = 0,
		items = "center",
		gap = theme.size.space_8,
		px = theme.size.space_16,
		bg = theme.color.background_primary,
		border_sides = { 0, 0, 1, 0 },
		border_color = theme.color.background_tertiary,
		ui.row {
			grow = 1,
			min_w = 0,
			items = "center",
			gap = theme.size.space_8,
			ui.show(
				direct,
				icon { name = "at_sign", size = TITLE_ICON, color = theme.color.text_muted },
				icon { name = "hash", size = TITLE_ICON, color = theme.color.text_muted }
			),
			ui.text {
				function()
					return state.timeline.channel_name:get()
				end,
				literal = true,
				shrink = 0,
				text_size = theme.size.font_md,
				weight = theme.weight.semibold,
				color = theme.color.header_primary,
			},
			status(),
			topic(),
		},
		ui.row {
			shrink = 0,
			items = "center",
			gap = theme.size.space_16,
			ui.show(guild, icon_button { name = "thread", tooltip = "Threads" }),
			ui.show(direct, icon_button { name = "phone", tooltip = "Start Voice Call" }),
			ui.show(direct, icon_button { name = "video", tooltip = "Start Video Call" }),
			ui.show(guild, icon_button { name = "bell", tooltip = "Notification Settings" }),
			icon_button { name = "pin", tooltip = "Pinned Messages" },
			ui.show(guild, icon_button {
				name = "members",
				tooltip = "Member List",
				active = function()
					return state.shell.members_visible:get()
				end,
				on_click = ui.toggle(state.shell.members_visible),
			}),
			search(),
			icon_button { name = "inbox", tooltip = "Inbox" },
			icon_button { name = "help", tooltip = "Help" },
		},
	}
end
