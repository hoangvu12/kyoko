local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local avatar = require("@components/avatar")
local icon_button = require("@components/icon_button")
local plugin_slot = require("@components/plugin_slot")

local AVATAR = 32
local AREA_HEIGHT = 40
local BUTTON = 32
local NAME_LINE = 18
local SUBTITLE_LINE = 13
local EMOJI = 14

local muted = ui.state("user.muted", false)
local deafened = ui.state("user.deafened", false)

local function avatar_src(): string
	return if state.user.avatar_ready:get() then state.user.avatar:get() else ""
end

local function status_emoji(panel: any)
	if panel.emoji_image ~= "" then
		return ui.image { src = panel.emoji_image, w = EMOJI, h = EMOJI, shrink = 0, fit = "contain" }
	elseif panel.emoji ~= "" then
		return ui.text {
			panel.emoji,
			literal = true,
			shrink = 0,
			text_size = EMOJI - 2,
			line_height = EMOJI,
		}
	end
	return nil
end

local function subtitle(panel: any)
	return ui.row {
		min_w = 0,
		items = "center",
		gap = theme.size.space_4,
		status_emoji(panel),
		if panel.subtitle ~= ""
			then ui.text {
				panel.subtitle,
				literal = true,
				min_w = 0,
				truncate = true,
				text_size = theme.size.font_tiny,
				line_height = SUBTITLE_LINE,
				color = theme.color.text_muted,
			}
			else nil,
	}
end

local function identity(panel: any)
	return ui.column {
		min_w = 0,
		ui.text {
			panel.name,
			literal = true,
			min_w = 0,
			truncate = true,
			text_size = theme.size.font_sm,
			line_height = NAME_LINE,
			weight = theme.weight.semibold,
			color = theme.color.header_primary,
		},
		subtitle(panel),
	}
end

local function user_area()
	return ui.row {
		grow = 1,
		min_w = 0,
		h = AREA_HEIGHT,
		px = theme.size.space_4,
		gap = theme.size.space_8,
		items = "center",
		rounded = theme.size.radius_sm,
		cursor = "pointer",
		hover = { bg = theme.color.modifier_hover },
		tooltip = ui.text { function()
			return state.user.panel:get().latency
		end },
		on_press = function(point: any)
			actions.open_status_menu(point.x, point.y)
		end,
		ui.presence(function()
			return { status = state.user.panel:get().status, src = avatar_src() }
		end, function(shown: any)
			return avatar {
				src = shown.src,
				size = AVATAR,
				status = shown.status,
				cutout = theme.color.user_panel_background,
			}
		end),
		ui.presence(function()
			return state.user.panel:get()
		end, identity),
	}
end

local function toggle(signal: any)
	return function()
		signal:set(not signal:get())
	end
end

return function()
	return ui.row {
		w_full = true,
		h = theme.size.user_panel_height,
		shrink = 0,
		px = theme.size.space_8,
		gap = theme.size.space_4,
		items = "center",
		bg = theme.color.user_panel_background,
		user_area(),
		plugin_slot { name = "panel.buttons", context = { area = "panel" }, gap = theme.size.space_4 },
		icon_button {
			name = function()
				return if muted:get() or deafened:get() then "mic_off" else "mic"
			end,
			tooltip = ui.text { function()
				return if muted:get() then "Unmute" else "Mute"
			end },
			hit_size = BUTTON,
			filled_hover = true,
			danger = function()
				return muted:get() or deafened:get()
			end,
			on_click = toggle(muted),
		},
		icon_button {
			name = function()
				return if deafened:get() then "headphones_off" else "headphones"
			end,
			tooltip = ui.text { function()
				return if deafened:get() then "Undeafen" else "Deafen"
			end },
			hit_size = BUTTON,
			filled_hover = true,
			danger = function()
				return deafened:get()
			end,
			on_click = toggle(deafened),
		},
		icon_button {
			name = "settings",
			tooltip = "User Settings",
			hit_size = BUTTON,
			filled_hover = true,
			on_click = function()
				actions.open_settings("my-account")
			end,
		},
	}
end
