local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")
local kyoko_mark = require("@components/kyoko_mark")

local EXPIRED_CODE_OPACITY = 0.15

local function stage(): string
	if not state.login.acknowledged:get() then
		return "locked"
	end
	local qr = state.login.qr:get()
	return if qr then qr.stage else "waiting"
end

local function field(name: string): () -> any
	return function()
		local qr = state.login.qr:get()
		return if qr then (qr :: any)[name] or "" else ""
	end
end

local function is(...: string): () -> boolean
	local wanted = { ... }
	return function()
		return table.find(wanted, stage()) ~= nil
	end
end

local function line(text: any, size: string, height: string, weight: number, color: any, extra: { [string]: any }?)
	local props: { [string]: any } = {
		text,
		text_size = size,
		line_height = height,
		weight = weight,
		color = color,
		align = "center",
	}
	for key, value in extra or {} do
		props[key] = value
	end
	return ui.text(props)
end

local function placeholder(glyph: string?, text: any)
	return ui.column {
		w = theme.size.qr_size,
		h = theme.size.qr_size,
		p = theme.size.space_16,
		gap = theme.size.space_12,
		rounded = theme.size.radius_sm,
		bg = theme.color.bg_3,
		border = theme.size.border_thickness,
		border_color = theme.color.border,
		items = "center",
		justify = "center",
		color = theme.color.text_4,
		if glyph then icon { name = glyph, size = theme.size.icon_size_lg :: any } else nil,
		line(text, theme.size.font_size_sm, theme.size.line_height_sm, theme.weight.medium, theme.color.text_4, {
			wrap = true,
			w_full = true,
		}),
	}
end

local function mark()
	return ui.row {
		absolute = true,
		left = 0,
		top = 0,
		right = 0,
		bottom = 0,
		items = "center",
		justify = "center",
		ui.row {
			w = theme.size.space_40,
			h = theme.size.space_40,
			rounded = theme.size.radius_sm,
			bg = theme.color.text_0,
			items = "center",
			justify = "center",
			kyoko_mark { size = theme.number(theme.size.space_40)(), rounded = theme.size.radius_sm },
		},
	}
end

local refresh = ui.component(function()
	local hovered = ui.use_state(false)
	return ui.button {
		direction = "row",
		h = theme.size.button_height_sm,
		px = theme.size.space_16,
		gap = theme.size.space_8,
		items = "center",
		justify = "center",
		rounded = theme.size.radius_sm,
		bg = function()
			return if hovered:get() then theme.color.accent_4 else theme.color.accent_3
		end,
		border = theme.size.border_thickness,
		border_color = theme.color.button_border,
		focus = { border_color = theme.color.button_border },
		color = theme.color.text_0,
		cursor = "pointer",
		on_hover = function(inside: boolean)
			hovered:set(inside)
		end,
		on_click = actions.login_qr_refresh,
		icon { name = "refresh", size = theme.size.icon_size_sm :: any },
		ui.text {
			"Refresh",
			text_size = theme.size.font_size_sm,
			line_height = theme.size.line_height_label,
			weight = theme.weight.medium,
			color = theme.color.text_0,
		},
	}
end)

local function code()
	local expired = is("ended")
	return ui.div {
		relative = true,
		w = theme.size.qr_size,
		h = theme.size.qr_size,
		p = theme.size.space_8,
		rounded = theme.size.radius_sm,
		bg = theme.color.text_1,
		ui.show(
			function()
				return field("image")() ~= ""
			end,
			ui.image {
				src = field("image"),
				w_full = true,
				h_full = true,
				fit = "contain",
				opacity = function()
					return if expired() then EXPIRED_CODE_OPACITY else 1
				end,
			}
		),
		ui.show(is("code"), mark()),
		ui.show(
			expired,
			ui.column {
				absolute = true,
				left = 0,
				top = 0,
				right = 0,
				bottom = 0,
				gap = theme.size.space_12,
				items = "center",
				justify = "center",
				line(
					"This code has expired",
					theme.size.font_size_sm,
					theme.size.line_height_sm,
					theme.weight.semibold,
					theme.color.text_0
				),
				refresh(),
			}
		),
	}
end

local function scanner()
	return ui.column {
		w_full = true,
		items = "center",
		ui.row {
			w = theme.size.qr_size,
			h = theme.size.qr_size,
			items = "center",
			justify = "center",
			ui.div {
				w = theme.size.avatar_size_xl,
				h = theme.size.avatar_size_xl,
				rounded = theme.size.avatar_size_xl,
				bg = theme.color.bg_2,
				ui.show(
					function()
						return field("avatar")() ~= ""
					end,
					ui.image {
						src = field("avatar"),
						w = theme.size.avatar_size_xl,
						h = theme.size.avatar_size_xl,
						rounded = theme.size.avatar_size_xl,
						fit = "cover",
					}
				),
			},
		},
		line(
			field("username"),
			theme.size.font_size_md,
			theme.size.line_height_label,
			theme.weight.semibold,
			theme.color.text_2,
			{ literal = true, mt = theme.size.space_8 }
		),
		line(
			"Check your phone!",
			theme.size.font_size_xl,
			theme.size.line_height_xl,
			theme.weight.semibold,
			theme.color.text_2,
			{ mt = theme.size.space_16 }
		),
		line(
			"Log in on your phone to continue",
			theme.size.font_size_md,
			theme.size.line_height_label,
			theme.weight.regular,
			theme.color.text_4,
			{ mt = theme.size.space_8 }
		),
		ui.button {
			mt = theme.size.space_12,
			p = 0,
			bg = "#00000000",
			border = 0,
			focus = { border = 0 },
			cursor = "pointer",
			on_click = actions.login_qr_refresh,
			line(
				"Not me, start over",
				theme.size.font_size_sm,
				theme.size.line_height_sm,
				theme.weight.medium,
				theme.color.accent_1
			),
		},
	}
end

local function subtitle(): string
	local current = stage()
	if current == "locked" then
		return "Available once you accept the notice below."
	elseif current == "ended" then
		return "Refresh to get a new code."
	end
	return "Scan this with the Discord mobile app to log in instantly."
end

local function problem()
	return ui.show(
		function()
			return stage() == "ended" and field("message")() ~= ""
		end,
		ui.row {
			mt = theme.size.space_8,
			gap = theme.size.space_6,
			items = "start",
			justify = "center",
			color = function()
				local qr = state.login.qr:get()
				return if qr and qr.error then theme.color.red_1 else theme.color.text_4
			end,
			ui.div { pt = theme.size.space_1, icon { name = "circle_alert", size = theme.size.icon_size_sm :: any } },
			ui.text {
				field("message"),
				literal = true,
				wrap = true,
				shrink = 1,
				min_w = 0,
				text_size = theme.size.font_size_sm,
				line_height = theme.size.line_height_sm,
				weight = theme.weight.medium,
			},
		}
	)
end

local function continue_on_web()
	return ui.show(
		function()
			local qr = state.login.qr:get()
			return stage() == "ended" and qr ~= nil and qr.web == true
		end,
		ui.button {
			id = "login_qr_web",
			mt = theme.size.space_8,
			p = 0,
			bg = "#00000000",
			border = 0,
			focus = { border = 0 },
			cursor = "pointer",
			on_click = actions.login_web_open,
			line(
				"Continue on discord.com",
				theme.size.font_size_sm,
				theme.size.line_height_sm,
				theme.weight.medium,
				theme.color.accent_1
			),
		}
	)
end

local function box()
	return ui.switch(stage, {
		locked = placeholder("lock", "Accept the notice below to continue"),
		signing_in = placeholder(nil, "Signing in..."),
		code = code(),
		ended = code(),
	}, placeholder(nil, "Getting a code..."))
end

return function()
	return ui.column {
		w = function()
			return theme.number(theme.size.qr_size)() + theme.number(theme.size.space_56)()
		end,
		shrink = 0,
		items = "center",
		ui.show(
			is("scanned"),
			scanner(),
			ui.column {
				w_full = true,
				items = "center",
				box(),
				line(
					"Log in with QR Code",
					theme.size.font_size_xl,
					theme.size.line_height_xl,
					theme.weight.semibold,
					theme.color.text_2,
					{ mt = theme.size.space_32 }
				),
				line(subtitle, theme.size.font_size_md, theme.size.line_height_label, theme.weight.regular, theme.color.text_4, {
					mt = theme.size.space_8,
					wrap = true,
					w_full = true,
				}),
				problem(),
				continue_on_web(),
			}
		),
	}
end
