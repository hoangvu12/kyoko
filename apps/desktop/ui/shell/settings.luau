local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")
local avatar = require("@components/avatar")
local pages = require("@settings/pages")
local kit = require("@settings/kit")
local dialog = require("@settings/dialog")

local CLEAR = "#00000000"

local function n(token: string): number
	return theme.number(token)()
end

local function account_field(name: string): () -> string
	return function()
		local found = state.settings.account:get()
		return if found then (found :: any)[name] or "" else ""
	end
end

local function profile_card()
	return dialog.card(
		ui.presence(function()
			return account_field("avatar")() .. "|" .. account_field("status")()
		end, function()
			return ui.div {
				w = theme.size.avatar_size_md,
				h = theme.size.avatar_size_md,
				shrink = 0,
				rounded = theme.size.avatar_size_md,
				bg = theme.color.bg_2,
				avatar {
					src = account_field("avatar")(),
					size = n(theme.size.avatar_size_md),
					status = account_field("status")(),
					cutout = theme.color.bg_4,
				},
			}
		end),
		account_field("name"),
		ui.row {
			id = "settings_edit_profiles",
			items = "center",
			gap = theme.size.space_4,
			cursor = "pointer",
			on_click = function()
				actions.open_settings("profiles")
			end,
			kit.text("Edit Profiles", theme.size.font_size_sm, theme.size.line_height_sm, theme.color.text_4, {
				weight = theme.weight.medium,
			}),
			icon { name = "edit", size = theme.size.icon_size_xs :: any, color = theme.color.text_4 },
		}
	)
end

local function build_info()
	return ui.column {
		w_full = true,
		mt = theme.size.space_12,
		px = theme.size.space_8,
		kit.text(function()
			return "Kyoko " .. state.settings.about:get().version
		end, theme.size.font_size_xs, theme.size.line_height_xs, theme.color.text_5),
		kit.text(function()
			return state.settings.about:get().platform or ""
		end, theme.size.font_size_xs, theme.size.line_height_xs, theme.color.text_5),
		ui.button {
			id = "settings_data_folder",
			p = 0,
			border = 0,
			focus = { border = 0 },
			bg = CLEAR,
			cursor = "pointer",
			on_click = function()
				actions.settings_action("open_data_folder")
			end,
			kit.text("Open data folder", theme.size.font_size_xs, theme.size.line_height_xs, theme.color.accent_1),
		},
	}
end

return function()
	return dialog.frame {
		prefix = "settings",
		label = "User Settings",
		page_key = function()
			return state.settings.page:get()
		end,
		sections = function()
			return pages.sections :: any
		end,
		shape = function()
			return "pages"
		end,
		find = pages.find :: any,
		build = pages.build :: any,
		select = function(key: string)
			actions.open_settings(key)
		end,
		close = actions.close_settings,
		card = profile_card,
		extras = {
			{
				key = "log-out",
				title = "Log Out",
				icon = "log_out",
				words = "log out logout sign out",
				danger = true,
				on_click = actions.sign_out(),
			},
		},
		footer = build_info,
	}
end
