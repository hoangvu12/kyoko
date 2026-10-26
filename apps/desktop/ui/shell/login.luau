local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")
local title_bar = require("./title_bar")
local qr_column = require("./login_qr")
local web_window = require("./login_web")

local NOTICE =
	"Kyoko is an unofficial client and is not affiliated with Discord. Using a third-party client with a user account can violate Discord's Terms of Service and can result in account termination. Use it at your own risk."
local PHI = 1.618
local FIELD_HEIGHT = 44
local BUTTON_HEIGHT = 32
local CHECKBOX = 16
local EYE = 14
local DISABLED_FILL = 0.15
local DISABLED_BORDER = 0.57
local DISABLED_TEXT = 0.35

local function locked(): boolean
	return not state.login.acknowledged:get()
end

local function leading(size: string): () -> number
	local points = theme.number(size)
	return function()
		return points() * PHI
	end
end

local function layer(props: { [string]: any })
	props.absolute = true
	props.left = 0
	props.top = 0
	props.right = 0
	props.bottom = 0
	props.pointer_events = "none"
	return ui.div(props)
end

local function label(text: any, size: string, color: any, extra: { [string]: any }?)
	local props: { [string]: any } = {
		text,
		text_size = size,
		line_height = leading(size),
		color = color,
	}
	for key, value in extra or {} do
		props[key] = value
	end
	return ui.text(props)
end

local function brand()
	return ui.column {
		gap = theme.size.space_8,
		ui.row {
			w = theme.size.login_logo,
			h = theme.size.login_logo,
			rounded = theme.size.login_logo,
			bg = theme.color.brand,
			items = "center",
			justify = "center",
			icon { name = "chat", size = theme.size.login_logo_icon :: any, color = theme.color.text_on_brand },
		},
		label("Welcome back", theme.size.font_xl, theme.color.header_primary, { weight = theme.weight.bold }),
		label("Paste your account token to continue.", theme.size.font_sm, theme.color.text_muted),
	}
end

local function token_field(token: any)
	local revealed = ui.state("login_token_revealed", false)
	local function field(secure: boolean)
		return ui.input {
			value = token,
			secure = secure,
			w_full = true,
			h = FIELD_HEIGHT,
			pl = theme.size.space_12,
			pr = 40,
			bg = "#00000000",
			border = 0,
			rounded = theme.size.radius_sm,
			focus = { border = 0 },
			text_size = theme.size.font_md,
			color = theme.color.text_normal,
			on_commit = function()
				actions.login_submit(token:get())
			end,
		}
	end
	return ui.row {
		relative = true,
		w_full = true,
		h = FIELD_HEIGHT,
		items = "center",
		layer { rounded = theme.size.radius_sm, bg = theme.color.interactive_muted, opacity = 0.3 },
		layer { rounded = theme.size.radius_sm, border = 1, border_color = theme.color.interactive_muted },
		ui.show(revealed, field(false), field(true)),
		ui.show(
			function()
				return token:get() == ""
			end,
			ui.text {
				"Account token",
				absolute = true,
				left = 13,
				top = 0,
				bottom = 0,
				pointer_events = "none",
				text_size = theme.size.font_md,
				line_height = FIELD_HEIGHT,
				color = theme.color.text_muted,
			}
		),
		ui.button {
			absolute = true,
			right = 12,
			top = (FIELD_HEIGHT - EYE) / 2,
			w = EYE,
			h = EYE,
			p = 0,
			bg = "#00000000",
			border = 0,
			focus = { border = 0 },
			cursor = "pointer",
			color = theme.color.text_normal,
			hover = { color = theme.color.header_primary },
			on_click = function()
				revealed:set(not revealed:get())
			end,
			icon { name = "eye", size = EYE },
		},
	}
end

local function status_line()
	return ui.show(
		function()
			return state.login.message:get() ~= ""
		end,
		label(function()
			return state.login.message:get()
		end, theme.size.font_sm, function()
			return if state.login.error:get() then theme.color.danger else theme.color.text_muted
		end, { literal = true, wrap = true })
	)
end

local button = ui.component(function(props: { text: any, primary: boolean, disabled: () -> boolean, on_click: any })
	local fill = if props.primary then theme.color.brand else theme.color.button_secondary
	local hovered = ui.use_state(false)
	return ui.button {
		relative = true,
		w_full = true,
		h = BUTTON_HEIGHT,
		p = 0,
		bg = "#00000000",
		border = 0,
		focus = { border = 0 },
		justify = "center",
		items = "center",
		cursor = function()
			return if props.disabled() then "default" else "pointer"
		end,
		on_hover = function(inside: boolean)
			hovered:set(inside)
		end,
		on_click = function()
			if not props.disabled() then
				props.on_click()
			end
		end,
		layer {
			rounded = theme.size.radius_sm,
			bg = function()
				if hovered:get() and not props.disabled() then
					return if props.primary then theme.color.brand_hover else theme.color.button_secondary_hover
				end
				return fill
			end,
			opacity = function()
				return if props.disabled() then DISABLED_FILL else 1
			end,
		},
		ui.show(
			function()
				return not props.primary and props.disabled()
			end,
			layer { rounded = theme.size.radius_sm, border = 1, border_color = fill, opacity = DISABLED_BORDER }
		),
		ui.text {
			props.text,
			text_size = theme.size.font_md,
			color = theme.color.text_on_brand,
			opacity = function()
				return if props.disabled() then DISABLED_TEXT else 1
			end,
		},
	}
end)

local function web_message(): string
	local web = state.login.web:get()
	return if web then web.message else ""
end

local function token_column(token: any)
	return ui.column {
		grow = 1,
		min_w = 0,
		gap = theme.size.space_16,
		brand(),
		ui.column {
			gap = theme.size.space_8,
			label("TOKEN", theme.size.font_tiny, theme.color.header_secondary, { weight = theme.weight.bold }),
			token_field(token),
			status_line(),
		},
		button {
			text = function()
				return if state.login.checking:get() then "Checking token..." else "Paste token"
			end,
			primary = true,
			disabled = function()
				return locked() or state.login.checking:get()
			end,
			on_click = function()
				actions.login_submit(token:get())
			end,
		},
		ui.show(
			state.login.unsaved,
			button {
				text = "Continue for this session only",
				primary = false,
				disabled = function()
					return false
				end,
				on_click = actions.login_continue,
			}
		),
		ui.show(
			state.login.adding,
			button {
				text = "Back to your account",
				primary = false,
				disabled = function()
					return state.login.checking:get()
				end,
				on_click = actions.login_cancel_add,
			}
		),
		ui.column {
			gap = theme.size.space_8,
			button {
				text = "Sign in on discord.com",
				primary = false,
				disabled = function()
					return locked() or state.login.checking:get()
				end,
				on_click = actions.login_web_open,
			},
			ui.show(
				function()
					return web_message() ~= ""
				end,
				label(web_message, theme.size.font_sm, function()
					local web = state.login.web:get()
					return if web and web.error then theme.color.red_1 else theme.color.text_4
				end, { literal = true, wrap = true })
			),
		},
	}
end

local function checkbox()
	return ui.button {
		direction = "row",
		items = "center",
		gap = theme.size.space_8,
		p = 0,
		bg = "#00000000",
		border = 0,
		focus = { border = 0 },
		cursor = function()
			return if locked() then "pointer" else "default"
		end,
		on_click = function()
			if locked() then
				actions.login_acknowledge()
			end
		end,
		ui.row {
			relative = true,
			w = CHECKBOX,
			h = CHECKBOX,
			items = "center",
			justify = "center",
			layer {
				rounded = theme.size.radius_sm,
				bg = function()
					return if locked() then theme.color.interactive_muted else theme.color.brand
				end,
				opacity = function()
					return if locked() then 0.3 else 0.5
				end,
			},
			layer {
				rounded = theme.size.radius_sm,
				border = 1,
				border_color = function()
					return if locked() then theme.color.interactive_muted else theme.color.brand
				end,
				opacity = function()
					return if locked() then 1 else 0.5
				end,
			},
			ui.show(
				state.login.acknowledged,
				ui.div { opacity = 0.5, icon { name = "check", size = 12, color = theme.color.text_on_brand } }
			),
		},
		label("I understand", theme.size.font_sm, function()
			return if locked() then theme.color.text_normal else theme.color.text_muted
		end),
	}
end

local function notice()
	return ui.column {
		relative = true,
		w = theme.size.login_card_width,
		p = theme.size.space_16,
		gap = theme.size.space_12,
		layer { rounded = theme.size.radius_md, bg = theme.color.warning, opacity = 0.1 },
		layer { rounded = theme.size.radius_md, border = 1, border_color = theme.color.warning, opacity = 0.5 },
		ui.row {
			items = "start",
			gap = theme.size.space_12,
			icon { name = "warning", size = theme.size.icon_lg :: any, color = theme.color.warning },
			label(NOTICE, theme.size.font_sm, theme.color.text_normal, { grow = 1, min_w = 0, wrap = true }),
		},
		checkbox(),
	}
end

return function()
	local token = ui.state("login_token", "")
	ui.effect(function()
		if state.login.screen:get() == "main" then
			token:set("")
		end
	end)
	return ui.column {
		relative = true,
		w_full = true,
		h_full = true,
		bg = theme.color.background_tertiary,
		color = theme.color.text_normal,
		text_size = theme.size.font_md,
		font = theme.font.ui,
		title_bar(),
		ui.column {
			grow = 1,
			min_h = 0,
			items = "center",
			justify = "center",
			gap = theme.size.space_16,
			ui.row {
				w = theme.size.login_card_width,
				p = theme.size.space_32,
				gap = theme.size.space_32,
				rounded = theme.size.radius_md,
				bg = theme.color.background_secondary,
				token_column(token),
				qr_column(),
			},
			notice(),
		},
		web_window(),
	}
end
