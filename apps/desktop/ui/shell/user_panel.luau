local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local avatar = require("@components/avatar")
local icon = require("@components/icon")
local plugin_slot = require("@components/plugin_slot")

local EMOJI = 14

local avatar_sm = theme.number(theme.size.avatar_size_sm)
local icon_md = theme.number(theme.size.icon_size_md)

local muted = ui.state("user.muted", false)
local deafened = ui.state("user.deafened", false)

local function avatar_src(): string
	return if state.user.avatar_ready:get() then state.user.avatar:get() else ""
end

local function status_emoji(panel: any)
	if panel.emoji_image ~= "" then
		return ui.image { src = panel.emoji_image, w = EMOJI, h = EMOJI, shrink = 0, fit = "contain" }
	elseif panel.emoji_svg and panel.emoji_svg ~= "" then
		return ui.svg { code = panel.emoji_svg, w = EMOJI, h = EMOJI, shrink = 0 }
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
				text_size = theme.size.font_size_xs,
				line_height = theme.size.line_height_xs,
				color = theme.color.text_4,
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
			text_size = theme.size.font_size_sm,
			line_height = theme.size.line_height_sm,
			weight = theme.weight.semibold,
			color = theme.color.text_2,
		},
		subtitle(panel),
	}
end

local function user_area()
	return ui.row {
		grow = 1,
		min_w = 0,
		h = theme.size.input_height,
		gap = theme.size.space_8,
		items = "center",
		rounded = theme.size.radius_sm,
		cursor = "pointer",
		hover = { bg = theme.color.hover },
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
				size = avatar_sm(),
				status = shown.status,
				cutout = theme.color.bg_4,
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

local panel_button = ui.component(function(props: {
	name: () -> string,
	tooltip: any,
	on: () -> boolean,
	on_click: any,
})
	local hovered = ui.use_state(false)
	return ui.button {
		w = theme.size.button_size,
		h = theme.size.button_size,
		shrink = 0,
		p = 0,
		border = 0,
		focus = { border = 0 },
		rounded = theme.size.radius_sm,
		justify = "center",
		items = "center",
		cursor = "pointer",
		tooltip = props.tooltip,
		bg = function()
			if props.on() then
				return if hovered:get()
					then theme.color.user_panel_button_active_hover
					else theme.color.user_panel_button_active
			end
			return if hovered:get() then theme.color.hover else "#00000000"
		end,
		on_hover = function(inside: boolean)
			hovered:set(inside)
		end,
		on_click = props.on_click,
		icon {
			name = props.name,
			size = icon_md(),
			color = function()
				if props.on() then
					return theme.color.accent_new
				end
				return if hovered:get() then theme.color.text_3 else theme.color.text_4
			end,
		},
	}
end)

local function never(): boolean
	return false
end

return function()
	return ui.row {
		w_full = true,
		h_full = true,
		px = theme.size.space_8,
		gap = theme.size.space_8,
		items = "center",
		user_area(),
		plugin_slot { name = "panel.buttons", context = { area = "panel" }, gap = theme.size.space_4 },
		ui.row {
			shrink = 0,
			items = "center",
			gap = theme.size.space_4,
			panel_button {
				name = function()
					return if muted:get() or deafened:get() then "mic_off" else "mic"
				end,
				tooltip = ui.text { function()
					return if muted:get() then "Unmute" else "Mute"
				end },
				on = function()
					return muted:get() or deafened:get()
				end,
				on_click = toggle(muted),
			},
			panel_button {
				name = function()
					return if deafened:get() then "headphones_off" else "headphones"
				end,
				tooltip = ui.text { function()
					return if deafened:get() then "Undeafen" else "Deafen"
				end },
				on = function()
					return deafened:get()
				end,
				on_click = toggle(deafened),
			},
			panel_button {
				name = function()
					return "settings"
				end,
				tooltip = "User Settings",
				on = never,
				on_click = function()
					actions.open_settings("my-account")
				end,
			},
		},
	}
end