local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")

local SLOT = "web_login"
local HOVER_BRIGHTNESS = 0.7
local PRESSED_SCALE = 0.9

local function field(name: string): () -> any
	return function()
		local web = state.login.web:get()
		return if web then (web :: any)[name] else nil
	end
end

local function text(name: string): () -> string
	local read = field(name)
	return function()
		return read() or ""
	end
end

local function title_bar()
	return ui.row {
		h = theme.size.button_size,
		shrink = 0,
		items = "center",
		gap = theme.size.space_8,
		pl = theme.size.space_12,
		pr = theme.size.space_4,
		color = theme.color.text_4,
		icon { name = "kyoko_mark", size = theme.size.icon_size_sm :: any },
		ui.text {
			"Sign in on discord.com",
			grow = 1,
			min_w = 0,
			text_size = theme.size.font_size_sm,
			line_height = theme.size.line_height_sm,
			weight = theme.weight.medium,
			color = theme.color.text_3,
		},
		ui.button {
			id = "web_login_close",
			tooltip = "Cancel",
			w = theme.size.window_control_hit,
			h = theme.size.button_size,
			shrink = 0,
			p = 0,
			border = 0,
			rounded = 0,
			bg = "#00000000",
			focus = { border = 0 },
			justify = "center",
			items = "center",
			cursor = "pointer",
			hover = { brightness = HOVER_BRIGHTNESS },
			active = { scale = PRESSED_SCALE },
			on_click = actions.login_web_cancel,
			ui.div {
				w = theme.size.window_control_size,
				h = theme.size.window_control_size,
				rounded = theme.size.window_control_size,
				bg = theme.color.dnd,
			},
		},
	}
end

local function address()
	return ui.row {
		h = theme.size.button_size,
		shrink = 0,
		mx = theme.size.space_8,
		mb = theme.size.space_8,
		px = theme.size.space_10,
		gap = theme.size.space_6,
		items = "center",
		rounded = theme.size.radius_sm,
		bg = theme.color.bg_3,
		color = theme.color.text_4,
		ui.show(field("secure"), icon { name = "lock", size = theme.size.icon_size_xs :: any }),
		ui.text {
			text("host"),
			literal = true,
			shrink = 0,
			text_size = theme.size.font_size_sm,
			line_height = theme.size.line_height_sm,
			weight = theme.weight.medium,
			color = theme.color.text_3,
		},
		ui.text {
			text("path"),
			literal = true,
			min_w = 0,
			shrink = 1,
			text_size = theme.size.font_size_sm,
			line_height = theme.size.line_height_sm,
			weight = theme.weight.regular,
			color = theme.color.text_5,
		},
	}
end

local function page()
	return ui.column {
		grow = 1,
		min_h = 0,
		mx = theme.size.space_8,
		rounded = theme.size.radius_sm,
		bg = theme.color.bg_3,
		items = "center",
		justify = "center",
		on_resize = function(_, element)
			actions.report_slot(SLOT, element)
		end,
		ui.text {
			function()
				return if field("signing_in")() then "Signing in..." else "Opening discord.com..."
			end,
			text_size = theme.size.font_size_sm,
			line_height = theme.size.line_height_sm,
			weight = theme.weight.medium,
			color = theme.color.text_4,
		},
	}
end

local cancel = ui.component(function()
	local hovered = ui.use_state(false)
	return ui.button {
		id = "web_login_cancel",
		h = theme.size.button_height_sm,
		shrink = 0,
		px = theme.size.space_16,
		items = "center",
		justify = "center",
		rounded = theme.size.radius_sm,
		bg = function()
			return if hovered:get() then theme.color.bg_1 else theme.color.bg_2
		end,
		border = theme.size.border_thickness,
		border_color = theme.color.border_light,
		focus = { border_color = theme.color.border_light },
		cursor = "pointer",
		on_hover = function(inside: boolean)
			hovered:set(inside)
		end,
		on_click = actions.login_web_cancel,
		ui.text {
			"Cancel",
			text_size = theme.size.font_size_sm,
			line_height = theme.size.line_height_label,
			weight = theme.weight.medium,
			color = theme.color.text_3,
		},
	}
end)

local function footer()
	return ui.row {
		shrink = 0,
		p = theme.size.space_12,
		gap = theme.size.space_12,
		items = "center",
		ui.text {
			"Browsing data from this window is cleared after you sign in.",
			grow = 1,
			min_w = 0,
			wrap = true,
			text_size = theme.size.font_size_xs,
			line_height = theme.size.line_height_xs,
			weight = theme.weight.regular,
			color = theme.color.text_4,
		},
		cancel(),
	}
end

local function window()
	return ui.column {
		id = "web_login_window",
		w = theme.size.web_login_width,
		h = theme.size.web_login_height,
		min_h = 0,
		shrink = 1,
		overflow = "hidden",
		rounded = theme.size.radius_sm,
		bg = theme.color.bg_4,
		border = theme.size.border_thickness,
		border_color = theme.color.border,
		title_bar(),
		address(),
		page(),
		footer(),
	}
end

return function()
	return ui.show(
		function()
			local web = state.login.web:get()
			return web ~= nil and web.open == true
		end,
		ui.column {
			absolute = true,
			left = 0,
			top = 0,
			right = 0,
			bottom = 0,
			p = theme.size.gap,
			items = "center",
			justify = "center",
			bg = theme.color.overlay,
			window(),
		}
	)
end
