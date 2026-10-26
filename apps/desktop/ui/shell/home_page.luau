local theme = require("../theme")
local state = require("../state")
local panel = require("@components/panel")
local icon = require("@components/icon")
local icon_button = require("@components/icon_button")
local friends = require("./friends")

local TITLES = {
	friends = "Friends",
	nitro = "Nitro",
	shop = "Shop",
	requests = "Message Requests",
}

local ICONS = {
	friends = "friends",
	nitro = "nitro",
	shop = "shop",
	requests = "mail",
}

local icon_lg = theme.number(theme.size.icon_size_lg)
local icon_md = theme.number(theme.size.icon_size_md)
local button = theme.number(theme.size.button_size)

local function page(): string
	return state.home.page:get()
end

local function on_friends(): boolean
	return page() == "friends"
end

local function title(): string
	return TITLES[page()] or ""
end

local function plain_header()
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
			icon {
				name = function()
					return ICONS[page()] or "home"
				end,
				size = icon_lg(),
				color = theme.color.text_4,
			},
			ui.text {
				title,
				text_size = theme.size.font_size_md,
				line_height = theme.size.line_height_label,
				weight = theme.weight.semibold,
				color = theme.color.text_2,
			},
		},
		ui.row {
			shrink = 0,
			items = "center",
			gap = theme.size.space_8,
			icon_button { name = "inbox", tooltip = "Inbox", hit_size = button(), icon_size = icon_md() },
			ui.div { w = theme.size.window_controls_width, h_full = true, shrink = 0 },
		},
	}
end

local function placeholder()
	return ui.column {
		w_full = true,
		h_full = true,
		gap = theme.size.space_8,
		justify = "center",
		items = "center",
		ui.text {
			title,
			text_size = theme.size.font_size_lg,
			line_height = theme.size.line_height_lg,
			weight = theme.weight.semibold,
			color = theme.color.text_2,
		},
		ui.text {
			"Coming in a later version.",
			text_size = theme.size.font_size_sm,
			line_height = theme.size.line_height_sm,
			color = theme.color.text_4,
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
		gap = theme.size.gap,
		bg = theme.color.bg_3,
		panel {
			w_full = true,
			h = theme.size.channel_header_height,
			ui.show(on_friends, friends.header(), plain_header()),
		},
		ui.row {
			w_full = true,
			grow = 1,
			min_h = 0,
			gap = theme.size.gap,
			panel {
				grow = 1,
				shrink = 1,
				min_w = 0,
				h_full = true,
				ui.show(on_friends, friends.content(), placeholder()),
			},
			ui.show(on_friends, panel { w = theme.size.friends_active_width, h_full = true, friends.active_now() }),
		},
	}
end
