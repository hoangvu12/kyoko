local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local avatar = require("@components/avatar")
local widgets = require("./widgets")

local RING = 4

local function account(): state.SettingsAccount?
	return state.settings.account:get()
end

local function field(name: string): () -> string
	return function()
		local found = account()
		return if found then (found :: any)[name] or "" else ""
	end
end

local function open_web()
	actions.settings_action("open_web")
end

local function portrait()
	return ui.row {
		shrink = 0,
		p = RING,
		rounded = 999,
		bg = theme.color.background_secondary,
		ui.presence(function()
			local found = account()
			return if found then found.avatar .. "|" .. found.status else nil
		end, function()
			return avatar {
				src = field("avatar")(),
				size = theme.number(theme.size.avatar_lg)(),
				status = field("status")(),
			}
		end),
	}
end

local function card()
	return ui.column {
		w_full = true,
		rounded = theme.size.radius_md,
		bg = theme.color.background_secondary,
		overflow = "hidden",
		ui.div {
			w_full = true,
			h = theme.size.profile_banner_height_lg,
			bg = function()
				local accent = field("accent")()
				return if accent ~= "" then accent else theme.color.brand
			end,
		},
		ui.row {
			w_full = true,
			items = "end",
			gap = theme.size.space_16,
			px = theme.size.space_16,
			mt = function()
				return -theme.number(theme.size.avatar_lg)() / 2
			end,
			portrait(),
			ui.div {
				grow = 1,
				min_w = 0,
				pb = theme.size.space_8,
				widgets.label(field("name"), theme.size.font_xl, theme.color.header_primary, {
					weight = theme.weight.bold,
					literal = true,
					truncate = true,
				}),
			},
			ui.div {
				pb = theme.size.space_8,
				widgets.button {
					text = "Edit User Profile",
					tone = "primary",
					on_click = function()
						actions.open_settings("profiles")
					end,
				},
			},
		},
		ui.column {
			m = theme.size.space_16,
			px = theme.size.space_16,
			rounded = theme.size.radius_md,
			bg = theme.color.background_tertiary,
			widgets.row("Display Name", field("name")),
			widgets.row("Username", field("username")),
			ui.show(function()
				return field("email")() ~= ""
			end, widgets.row("Email", field("email"))),
			ui.show(function()
				return field("phone")() ~= ""
			end, widgets.row("Phone Number", field("phone"))),
		},
	}
end

return function()
	return ui.column {
		w_full = true,
		gap = theme.size.space_24,
		card(),
		ui.column {
			w_full = true,
			widgets.heading("Password and Authentication"),
			widgets.row(
				"Password",
				"Change these in Discord's web app.",
				widgets.button { text = "Open Discord", small = true, on_click = open_web }
			),
			widgets.row(
				"Two-Factor Authentication",
				"Change these in Discord's web app.",
				widgets.button { text = "Open Discord", small = true, on_click = open_web }
			),
		},
		ui.column {
			gap = theme.size.space_8,
			widgets.heading("Sign Out"),
			widgets.button { text = "Log Out", tone = "danger", on_click = actions.sign_out },
		},
	}
end
