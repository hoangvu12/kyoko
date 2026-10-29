local theme = require("../theme")
local state = require("../state")
local icon = require("@components/icon")
local icon_button = require("@components/icon_button")
local avatar = require("@components/avatar")
local actions = require("../actions")

local NO_DM = { key = "", id = "", name = "", avatar = "", status = "offline", group = false, members = 0, unread = false, mentions = 0 }

local query = ui.state("query", "")

local icon_lg = theme.number(theme.size.icon_size_lg)
local icon_md = theme.number(theme.size.icon_size_md)
local icon_sm = theme.number(theme.size.icon_size_sm)
local button = theme.number(theme.size.button_size)

local function topic()
	return ui.show(function()
		return state.timeline.channel_topic:get() ~= ""
	end, ui.row {
		grow = 1,
		min_w = 0,
		items = "center",
		ui.div {
			w = theme.size.space_4,
			h = theme.size.space_4,
			shrink = 0,
			mx = theme.size.space_8,
			rounded = theme.size.space_4,
			bg = theme.color.text_5,
		},
		ui.text {
			function()
				return state.timeline.channel_topic:get()
			end,
			literal = true,
			grow = 1,
			min_w = 0,
			truncate = true,
			text_size = theme.size.font_size_sm,
			line_height = theme.size.line_height_sm,
			color = theme.color.text_5,
		},
	})
end

local function tool(props: { [string]: any })
	props.hit_size = button()
	props.icon_size = icon_md()
	return icon_button(props)
end

local function direct(): boolean
	return state.sidebar.guild_id:get() == ""
end

local function guild(): boolean
	return not direct()
end

local function open_dm(): state.DmRow
	local open = state.timeline.channel_id:get()
	if not direct() or open == "" then
		return NO_DM
	end
	for _, row in state.dms.rows:get() do
		if row.id == open then
			return row
		end
	end
	return NO_DM
end

local function group_open(): boolean
	return open_dm().group
end

local function single_open(): boolean
	local row = open_dm()
	return row.id ~= "" and not row.group
end

local function dm_open(): boolean
	return open_dm().id ~= ""
end

local function call_here(): boolean
	local call = state.voice.dm_call:get()
	return call.shown and call.channel_id == open_dm().id
end

local function dm_badge(row: state.DmRow)
	if row.group then
		return ui.row {
			w = theme.size.icon_size_lg,
			h = theme.size.icon_size_lg,
			shrink = 0,
			mr = theme.size.space_2,
			justify = "center",
			items = "center",
			rounded = theme.size.icon_size_lg,
			bg = theme.color.accent_4,
			icon { name = "group", size = icon_sm(), color = theme.color.text_0 },
		}
	end
	return ui.div {
		shrink = 0,
		mr = theme.size.space_2,
		avatar { src = row.avatar, size = icon_lg(), status = row.status, cutout = theme.color.bg_4 },
	}
end

local function channel_mark()
	return ui.show(
		function()
			return open_dm().id ~= ""
		end,
		ui.presence(open_dm, dm_badge),
		icon {
			name = function()
				return state.timeline.channel_icon:get()
			end,
			size = icon_lg(),
			color = theme.color.text_4,
		}
	)
end

local function search()
	return ui.row {
		w = theme.size.search_width,
		h = theme.size.button_size,
		shrink = 0,
		ml = theme.size.space_4,
		px = theme.size.space_8,
		gap = theme.size.space_8,
		items = "center",
		rounded = theme.size.radius_sm,
		bg = theme.color.bg_3,
		border = theme.size.border_thickness,
		border_color = theme.color.border_light,
		ui.input {
			grow = 1,
			min_w = 0,
			h_full = true,
			p = 0,
			bg = "#00000000",
			border = 0,
			focus = { border = 0 },
			text_size = theme.size.font_size_sm,
			color = theme.color.text_3,
			placeholder = "Search",
			placeholder_color = theme.color.text_5,
			value = query,
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
		icon { name = "search", size = icon_sm(), color = theme.color.text_4 },
	}
end

local function title()
	return ui.row {
		grow = 1,
		min_w = 0,
		items = "center",
		channel_mark(),
		ui.text {
			function()
				return state.timeline.channel_name:get()
			end,
			literal = true,
			shrink = 0,
			ml = theme.size.space_8,
			text_size = theme.size.font_size_md,
			line_height = theme.size.line_height_md,
			weight = theme.weight.semibold,
			color = theme.color.text_2,
		},
		topic(),
	}
end

local function toolbar()
	return ui.row {
		shrink = 0,
		items = "center",
		gap = theme.size.space_8,
		pr = theme.size.space_12,
		ui.show(guild, tool {
			name = "thread",
			id = "top_bar_threads",
			tooltip = "Threads",
			on_press = function(point: any)
				actions.open_threads(point.x, point.y)
			end,
		}),
		ui.show(dm_open, tool {
			name = "phone",
			id = "top_bar_call",
			tooltip = ui.text {
				function()
					return if call_here() then "Join Call" else "Start Voice Call"
				end,
			},
			on_click = function()
				actions.start_call(open_dm().id, false)
			end,
		}),
		ui.show(dm_open, tool {
			name = "video",
			id = "top_bar_video_call",
			tooltip = ui.text {
				function()
					return if call_here() then "Join Video Call" else "Start Video Call"
				end,
			},
			on_click = function()
				actions.start_call(open_dm().id, true)
			end,
		}),
		ui.show(guild, tool { name = "bell", tooltip = "Notification Settings" }),
		tool {
			name = "pin",
			id = "top_bar_pins",
			tooltip = "Pinned Messages",
			on_press = function(point: any)
				actions.open_pins(point.x, point.y)
			end,
		},
		ui.show(guild, tool {
			name = "members",
			tooltip = "Member List",
			active = function()
				return state.shell.members_visible:get()
			end,
			on_click = ui.toggle(state.shell.members_visible),
		}),
		ui.show(direct, tool { name = "user_plus", tooltip = "Add Friends to DM" }),
		ui.show(single_open, tool {
			name = "panel_right",
			id = "top_bar_dm_profile",
			tooltip = "Show User Profile",
			active = function()
				return state.shell.dm_profile_visible:get()
			end,
			on_click = ui.toggle(state.shell.dm_profile_visible),
		}),
		ui.show(group_open, tool {
			name = "members",
			id = "top_bar_dm_members",
			tooltip = "Show Member List",
			active = function()
				return state.shell.members_visible:get()
			end,
			on_click = ui.toggle(state.shell.members_visible),
		}),
		search(),
	}
end

return function()
	return ui.row {
		w_full = true,
		h_full = true,
		items = "center",
		pl = theme.size.space_8,
		ui.row {
			grow = 1,
			min_w = 0,
			items = "center",
			pl = theme.size.space_6,
			title(),
		},
		toolbar(),
		ui.row {
			shrink = 0,
			items = "center",
			gap = theme.size.space_8,
			tool {
				name = "inbox",
				id = "top_bar_inbox",
				tooltip = "Inbox",
				on_press = function(point: any)
					actions.open_inbox(point.x, point.y)
				end,
			},
			ui.div { w = theme.size.window_controls_width, h_full = true, shrink = 0 },
		},
	}
end
