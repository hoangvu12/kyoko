local theme = require("../theme")
local state = require("../state")
local title_bar = require("./title_bar")
local top_bar = require("./top_bar")
local server_rail = require("./server_rail")
local channel_sidebar = require("./channel_sidebar")
local user_panel = require("./user_panel")
local timeline = require("./timeline")
local composer = require("./composer")
local member_list = require("./member_list")
local login = require("./login")
local storage_notice = require("./storage_notice")
local plugin_notice = require("./plugin_notice")
local home_page = require("./home_page")
local settings = require("./settings")
local search_panel = require("./search_panel")
local forum = require("./forum")
local thread_panel = require("./thread_panel")

local SIDEBAR_MIN_WIDTH = 900
local MEMBER_LIST_MIN_WIDTH = 1200

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
		ref = root,
		relative = true,
		w_full = true,
		h_full = true,
		bg = theme.color.background_tertiary,
		color = theme.color.text_normal,
		text_size = theme.size.font_md,
		font = theme.font.ui,
		title_bar(),
		storage_notice(),
		plugin_notice(),
		ui.row {
			w_full = true,
			grow = 1,
			min_h = 0,
			server_rail(),
			ui.show(sidebar_shown, ui.column {
				w = theme.size.channel_sidebar_width,
				h_full = true,
				shrink = 0,
				rounded_tl = theme.number(theme.size.radius_md),
				overflow = "hidden",
				channel_sidebar(),
				user_panel(),
			}),
			ui.column {
				relative = true,
				grow = 1,
				min_w = 0,
				h_full = true,
				bg = theme.color.background_primary,
				top_bar(),
				ui.row {
					w_full = true,
					grow = 1,
					min_h = 0,
					ui.show(function()
						return state.forum.view:get().open
					end, forum(), ui.column {
						grow = 1,
						min_w = 0,
						h_full = true,
						timeline(),
						composer(),
					}),
					ui.show(function()
						return state.threads.panel:get().open
					end, thread_panel()),
					ui.show(function()
						return state.search.panel:get().open and sidebar_shown()
					end, search_panel(), ui.show(function()
						return members_room()
							and state.shell.members_visible:get()
							and state.sidebar.guild_id:get() ~= ""
							and not state.threads.panel:get().open
					end, member_list())),
				},
				home_page(),
			},
		},
		settings(),
	}
	return ui.show(function()
		return state.login.screen:get() == "main"
	end, shell, login())
end
