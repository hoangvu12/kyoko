local theme = require("../theme")
local state = require("../state")
local panel = require("@components/panel")
local title_bar = require("./title_bar")
local window_controls = require("./window_controls")
local top_bar = require("./top_bar")
local server_rail = require("./server_rail")
local channel_sidebar = require("./channel_sidebar")
local user_panel = require("./user_panel")
local voice_panel = require("./voice_panel")
local timeline = require("./timeline")
local composer = require("./composer")
local member_list = require("./member_list")
local login = require("./login")
local storage_notice = require("./storage_notice")
local plugin_notice = require("./plugin_notice")
local home_page = require("./home_page")
local dm_profile = require("./dm_profile")
local dm_members = require("./dm_members")
local settings = require("./settings")
local server_settings = require("./server_settings")
local channel_settings = require("./channel_settings")
local search_panel = require("./search_panel")
local forum = require("./forum")
local hidden_channel = require("./hidden_channel")
local thread_panel = require("./thread_panel")
local connection_banner = require("./connection_banner")

local SIDEBAR_MIN_WIDTH = 900
local MEMBER_LIST_MIN_WIDTH = 1200

local function sidebar_column()
	return ui.column {
		w = theme.size.sidebar_width,
		h_full = true,
		shrink = 0,
		gap = theme.size.gap,
		panel { w_full = true, grow = 1, min_h = 0, channel_sidebar() },
		ui.show(function()
			return state.voice.panel:get().open
		end, panel { w_full = true, shrink = 0, voice_panel() }),
		panel { w_full = true, h = theme.size.user_panel_box_height, user_panel() },
	}
end

local function chat()
	local column = ui.ref()
	return panel {
		grow = 1,
		shrink = 1,
		min_w = 0,
		h_full = true,
		ui.show(
			function()
				return state.forum.view:get().open
			end,
			forum(),
			ui.show(function()
				return state.hidden.view:get().open
			end, hidden_channel(), ui.column {
				ref = column,
				w_full = true,
				grow = 1,
				min_h = 0,
				connection_banner(),
				timeline(),
				composer {
					room = function()
						return column.height or 0
					end,
				},
			})
		),
	}
end

local function home_cover()
	return ui.show(function()
		return state.home.page:get() ~= ""
	end, home_page())
end

local function dm_side(members_room: () -> boolean): boolean
	return members_room()
		and state.sidebar.guild_id:get() == ""
		and state.timeline.channel_id:get() ~= ""
		and not state.search.panel:get().open
end

local function page(sidebar_shown: () -> boolean, members_room: () -> boolean)
	return ui.column {
		relative = true,
		grow = 1,
		min_w = 0,
		h_full = true,
		gap = theme.size.gap,
		panel { w_full = true, h = theme.size.channel_header_height, top_bar() },
		ui.row {
			w_full = true,
			grow = 1,
			min_h = 0,
			gap = theme.size.gap,
			chat(),
			ui.show(function()
				return state.threads.panel:get().open
			end, panel { h_full = true, thread_panel() }),
			ui.show(function()
				return state.search.panel:get().open and sidebar_shown()
			end, panel { h_full = true, search_panel() }, ui.show(function()
				return members_room()
					and state.shell.members_visible:get()
					and state.sidebar.guild_id:get() ~= ""
					and not state.threads.panel:get().open
					and not state.hidden.view:get().open
					and not state.forum.view:get().open
			end, panel { w = theme.size.member_list_panel_width, h_full = true, member_list() })),
			ui.show(function()
				return dm_side(members_room) and state.dms.profile:get().open and state.shell.dm_profile_visible:get()
			end, panel { h_full = true, dm_profile() }),
			ui.show(function()
				return dm_side(members_room) and #state.dms.members:get() > 0 and state.shell.members_visible:get()
			end, panel { h_full = true, dm_members() }),
		},
		home_cover(),
	}
end

return function()
	local root = ui.ref()
	local function wider(than: number): () -> boolean
		return function()
			return (root.width or than) >= than
		end
	end
	local sidebar_shown = wider(SIDEBAR_MIN_WIDTH)
	local members_room = wider(MEMBER_LIST_MIN_WIDTH)
	local shell = ui.column {
		relative = true,
		w_full = true,
		h_full = true,
		title_bar(),
		storage_notice(),
		plugin_notice(),
		ui.row {
			w_full = true,
			grow = 1,
			min_h = 0,
			gap = theme.size.gap,
			px = theme.size.gap,
			pb = theme.size.gap,
			panel { w = theme.size.guild_list_width, h_full = true, server_rail() },
			ui.show(sidebar_shown, sidebar_column()),
			page(sidebar_shown, members_room),
		},
		settings(),
		server_settings(),
		channel_settings(),
	}
	return ui.column {
		ref = root,
		relative = true,
		w_full = true,
		h_full = true,
		bg = theme.color.bg_3,
		color = theme.color.text_3,
		text_size = theme.size.font_size_md,
		font = theme.font.ui,
		ui.show(function()
			return state.login.screen:get() == "main"
		end, shell, login()),
		window_controls(),
	}
end
