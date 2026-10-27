local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")
local title_bar = require("./title_bar")
local qr_column = require("./login_qr")
local web_window = require("./login_web")

local NOTICE =
	"Kyoko is an unofficial client and is not affiliated with Discord. Using a third-party client with a user account can violate Discord's Terms of Service and can result in account termination. Use it at your own risk."
local STORAGE_NOTE = "Kyoko keeps your token in Windows Credential Manager."
local WEB_NOTE = "You'll sign in on Discord's own page, in a Kyoko window."
local WELCOME = "We're so excited to see you again!"
local MFA_STAGES = { "mfa", "texting", "verifying" }
local OTHER_METHODS = {
	totp = "Use your authenticator app instead",
	backup = "Use a backup code instead",
	sms = "Receive auth code from SMS",
}

local function locked(): boolean
	return not state.login.acknowledged:get()
end

local function password(): any
	return state.login.password:get()
end

local function password_field(name: string): () -> any
	return function()
		local view = password()
		return if view then view[name] else nil
	end
end

local function password_stage(): string
	return password_field("stage")() or "form"
end

local function problem(): string
	return password_field("problem")() or ""
end

local function busy(): boolean
	local stage = password_stage()
	return stage == "logging_in" or stage == "texting" or stage == "verifying" or stage == "signing_in"
end

local function view(): string
	if state.login.unsaved:get() then
		return "unsaved"
	end
	local stage = password_stage()
	if table.find(MFA_STAGES, stage) then
		return "mfa"
	elseif stage == "handover" then
		return "handover"
	end
	local mode = state.login.mode:get()
	return if mode == "token" or mode == "email" then mode else "discord"
end

local function narrow(): boolean
	local current = view()
	return current == "mfa" or current == "unsaved"
end

local function dimmed()
	local fade = theme.number(theme.size.disabled_opacity_control)
	return function()
		return if locked() then fade() else 1
	end
end

local function line(text: any, size: string, height: string, weight: number, color: any, extra: { [string]: any }?)
	local props: { [string]: any } = {
		text,
		text_size = size,
		line_height = height,
		weight = weight,
		color = color,
	}
	for key, value in extra or {} do
		props[key] = value
	end
	return ui.text(props)
end

local function heading(subtitle: string)
	return ui.column {
		w_full = true,
		items = "center",
		mb = theme.size.space_20,
		line("Welcome back!", theme.size.font_size_xl, theme.size.line_height_xl, theme.weight.semibold, theme.color.text_2),
		line(subtitle, theme.size.font_size_md, theme.size.line_height_label, theme.weight.regular, theme.color.text_4, {
			mt = theme.size.space_8,
			align = "center",
			wrap = true,
			w_full = true,
		}),
	}
end

local function label(text: string)
	return ui.row {
		mb = theme.size.space_8,
		line(text, theme.size.font_size_xs, theme.size.line_height_xs, theme.weight.bold, theme.color.text_4),
		line(" *", theme.size.font_size_xs, theme.size.line_height_xs, theme.weight.bold, theme.color.red_2),
	}
end

local function text_field(props: { value: any, secure: boolean?, error: () -> boolean, on_commit: () -> (), reveal: boolean? })
	return ui.input {
		value = props.value,
		secure = props.secure or false,
		w_full = true,
		h = theme.size.input_height,
		pl = theme.size.space_12,
		pr = if props.reveal then theme.size.space_40 else theme.size.space_12,
		rounded = theme.size.radius_sm,
		bg = function()
			return if props.error() then theme.color.input_background_error else theme.color.bg_3
		end,
		border = theme.size.border_thickness,
		border_color = function()
			return if props.error() then theme.color.red_2 else theme.color.border_light
		end,
		hover = { border_color = theme.color.border },
		focus = { border_color = theme.color.accent_2 },
		text_size = theme.size.font_size_md,
		color = theme.color.text_3,
		on_commit = props.on_commit,
	}
end

local function error_line(message: () -> string, color: (() -> any)?)
	return ui.show(
		function()
			return message() ~= ""
		end,
		ui.row {
			w_full = true,
			mt = theme.size.space_8,
			gap = theme.size.space_6,
			items = "start",
			color = color or theme.color.red_1,
			ui.div { pt = theme.size.space_1, icon { name = "circle_alert", size = theme.size.icon_size_sm :: any } },
			ui.text {
				message,
				literal = true,
				wrap = true,
				grow = 1,
				shrink = 1,
				min_w = 0,
				text_size = theme.size.font_size_sm,
				line_height = theme.size.line_height_sm,
				weight = theme.weight.medium,
			},
		}
	)
end

type PrimaryProps = { text: any, glyph: string?, id: string?, disabled: () -> boolean, on_click: () -> () }

local primary_button = ui.component(function(props: PrimaryProps)
	local hovered = ui.use_state(false)
	return ui.button {
		id = props.id,
		direction = "row",
		w_full = true,
		h = theme.size.button_height_lg,
		px = theme.size.space_16,
		gap = theme.size.space_8,
		items = "center",
		justify = "center",
		rounded = theme.size.radius_sm,
		bg = function()
			return if hovered:get() and not props.disabled() then theme.color.accent_4 else theme.color.accent_3
		end,
		border = theme.size.border_thickness,
		border_color = theme.color.button_border,
		focus = { border_color = theme.color.button_border },
		color = theme.color.text_0,
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
		if props.glyph then icon { name = props.glyph, size = theme.size.icon_size_sm :: any } else nil,
		ui.text {
			props.text,
			text_size = theme.size.font_size_md,
			line_height = theme.size.line_height_label,
			weight = theme.weight.medium,
			color = theme.color.text_0,
		},
	}
end)

local function primary(props: PrimaryProps)
	local fade = theme.number(theme.size.disabled_opacity_control)
	return ui.div {
		w_full = true,
		opacity = function()
			return if props.disabled() then fade() else 1
		end,
		primary_button(props),
	}
end

local secondary = ui.component(function(props: { text: string, glyph: string, disabled: () -> boolean, on_click: () -> () })
	local hovered = ui.use_state(false)
	return ui.button {
		direction = "row",
		grow = 1,
		basis = 0,
		min_w = 0,
		h = theme.size.button_height_md,
		px = theme.size.space_16,
		gap = theme.size.space_8,
		items = "center",
		justify = "center",
		rounded = theme.size.radius_sm,
		bg = function()
			return if hovered:get() and not props.disabled() then theme.color.bg_1 else theme.color.bg_2
		end,
		border = theme.size.border_thickness,
		border_color = theme.color.border_light,
		focus = { border_color = theme.color.border_light },
		color = theme.color.text_3,
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
		icon { name = props.glyph, size = theme.size.icon_size_sm :: any },
		ui.text {
			props.text,
			text_size = theme.size.font_size_sm,
			line_height = theme.size.line_height_label,
			weight = theme.weight.medium,
			color = theme.color.text_3,
		},
	}
end)

local function link(text: any, on_click: () -> (), extra: { [string]: any }?)
	local props: { [string]: any } = {
		p = 0,
		bg = "#00000000",
		border = 0,
		focus = { border = 0 },
		cursor = "pointer",
		on_click = on_click,
		line(text, theme.size.font_size_sm, theme.size.line_height_sm, theme.weight.medium, theme.color.accent_1, {
			literal = true,
		}),
	}
	for key, value in extra or {} do
		props[key] = value
	end
	return ui.button(props)
end

local function centered_link(text: string, on_click: () -> (), top: string, id: string?)
	return ui.row { w_full = true, mt = top, justify = "center", link(text, on_click, { id = id }) }
end

local function note()
	return line(STORAGE_NOTE, theme.size.font_size_xs, theme.size.line_height_xs, theme.weight.regular, theme.color.text_5, {
		mt = theme.size.space_16,
		opacity = dimmed(),
	})
end

local function web_message(): string
	local web = state.login.web:get()
	return if web then web.message else ""
end

local function web_line()
	return error_line(web_message, function()
		local web = state.login.web:get()
		return if web and web.error then theme.color.red_1 else theme.color.text_4
	end)
end

local function back_to_account()
	return ui.show(
		state.login.adding,
		centered_link("Back to your account", function()
			if not state.login.checking:get() and not busy() then
				actions.login_cancel_add()
			end
		end, theme.size.space_12)
	)
end

local function flow_message(): string
	return if state.login.checking:get() then "" else state.login.message:get()
end

local function flow_line()
	return error_line(flow_message, function()
		return if state.login.error:get() then theme.color.red_1 else theme.color.text_4
	end)
end

local function choose(mode: string): () -> ()
	return function()
		if not locked() and not busy() then
			actions.login_mode(mode)
		end
	end
end

local function discord_column()
	return ui.column {
		grow = 1,
		min_w = 0,
		heading(WELCOME),
		ui.column {
			w_full = true,
			opacity = dimmed(),
			pointer_events = function()
				return not locked()
			end,
			primary {
				id = "login_discord",
				text = "Sign in on discord.com",
				glyph = "globe",
				disabled = function()
					return state.login.checking:get()
				end,
				on_click = function()
					if not locked() then
						actions.login_web_open()
					end
				end,
			},
			line(WEB_NOTE, theme.size.font_size_sm, theme.size.line_height_sm, theme.weight.regular, theme.color.text_4, {
				mt = theme.size.space_8,
				align = "center",
				wrap = true,
				w_full = true,
			}),
			web_line(),
			flow_line(),
			ui.column {
				w_full = true,
				mt = theme.size.space_20,
				gap = theme.size.space_8,
				items = "center",
				link("Use email and password instead", choose("email"), { id = "login_email_link" }),
				link("Sign in with a token", choose("token"), { id = "login_token_link" }),
			},
		},
		back_to_account(),
		note(),
	}
end

local function email_problem(): string
	local own = problem()
	if own ~= "" or state.login.checking:get() then
		return own
	end
	return state.login.message:get()
end

local function email_form()
	local login = ui.state("login_email", "")
	local secret = ui.state("login_password", "")
	local function submit()
		if not locked() and not busy() then
			actions.login_password_submit(login:get(), secret:get())
		end
	end
	local function failed(): boolean
		return problem() ~= ""
	end
	return ui.column {
		grow = 1,
		min_w = 0,
		heading(WELCOME),
		ui.column {
			gap = theme.size.space_20,
			opacity = dimmed(),
			pointer_events = function()
				return not locked()
			end,
			ui.column {
				label("EMAIL OR PHONE NUMBER"),
				text_field {
					value = login,
					error = function()
						return false
					end,
					on_commit = submit,
				},
			},
			ui.column {
				label("PASSWORD"),
				text_field { value = secret, secure = true, error = failed, on_commit = submit },
				error_line(email_problem, function()
					return if problem() ~= "" or state.login.error:get() then theme.color.red_1 else theme.color.text_4
				end),
				ui.row {
					w_full = true,
					mt = theme.size.space_8,
					justify = "between",
					link("Forgot your password?", actions.login_web_open),
					link("Back to sign-in options", choose("discord"), { id = "login_email_back" }),
				},
			},
		},
		ui.div {
			mt = theme.size.space_20,
			opacity = dimmed(),
			primary {
				text = function()
					return if busy() then "Logging in..." else "Log In"
				end,
				disabled = function()
					return busy() or state.login.checking:get()
				end,
				on_click = submit,
			},
		},
		ui.row {
			mt = theme.size.space_12,
			gap = theme.size.space_8,
			opacity = dimmed(),
			secondary {
				text = "Sign in with a token",
				glyph = "key",
				disabled = function()
					return locked() or busy()
				end,
				on_click = function()
					actions.login_mode("token")
				end,
			},
			secondary {
				text = "Sign in on discord.com",
				glyph = "globe",
				disabled = function()
					return locked() or busy() or state.login.checking:get()
				end,
				on_click = actions.login_web_open,
			},
		},
		web_line(),
		back_to_account(),
		note(),
	}
end

local function token_form()
	local token = ui.state("login_token", "")
	local revealed = ui.state("login_token_revealed", false)
	local function submit()
		if not locked() and not state.login.checking:get() then
			actions.login_submit(token:get())
		end
	end
	local function failed(): boolean
		return state.login.error:get() and state.login.message:get() ~= ""
	end
	local function field(secure: boolean)
		return text_field { value = token, secure = secure, error = failed, on_commit = submit, reveal = true }
	end
	return ui.column {
		grow = 1,
		min_w = 0,
		heading("Sign in with your account token."),
		ui.column {
			opacity = dimmed(),
			label("TOKEN"),
			ui.div {
				relative = true,
				w_full = true,
				ui.show(revealed, field(false), field(true)),
				ui.button {
					absolute = true,
					right = theme.number(theme.size.space_4),
					top = theme.number(theme.size.space_4),
					w = theme.size.button_size,
					h = theme.size.button_size,
					p = 0,
					rounded = theme.size.radius_sm,
					bg = "#00000000",
					border = 0,
					focus = { border = 0 },
					justify = "center",
					items = "center",
					cursor = "pointer",
					color = theme.color.text_4,
					hover = { color = theme.color.text_3 },
					on_click = function()
						revealed:set(not revealed:get())
					end,
					icon { name = "eye", size = theme.size.icon_size_md :: any },
				},
			},
			flow_line(),
		},
		ui.div {
			mt = theme.size.space_20,
			opacity = dimmed(),
			primary {
				text = function()
					return if state.login.checking:get() then "Checking token..." else "Paste token"
				end,
				disabled = function()
					return state.login.checking:get()
				end,
				on_click = submit,
			},
		},
		centered_link("Back to sign-in options", function()
			actions.login_mode("discord")
		end, theme.size.space_12, "login_token_back"),
		back_to_account(),
		note(),
	}
end

local function handover_step()
	return ui.column {
		grow = 1,
		min_w = 0,
		heading(WELCOME),
		ui.row {
			gap = theme.size.space_12,
			p = theme.size.space_16,
			rounded = theme.size.radius_sm,
			bg = theme.color.background_feedback_info,
			border = theme.size.border_thickness,
			border_color = theme.color.blue_2,
			ui.div { color = theme.color.blue_1, icon { name = "shield_alert", size = theme.size.icon_size_md :: any } },
			ui.column {
				grow = 1,
				min_w = 0,
				line(
					password_field("title"),
					theme.size.font_size_md,
					theme.size.line_height_label,
					theme.weight.semibold,
					theme.color.text_2,
					{ literal = true }
				),
				line(
					password_field("message"),
					theme.size.font_size_sm,
					theme.size.line_height_sm,
					theme.weight.regular,
					theme.color.text_3,
					{ literal = true, wrap = true, mt = theme.size.space_4 }
				),
			},
		},
		ui.div {
			mt = theme.size.space_20,
			primary {
				text = "Continue on discord.com",
				glyph = "globe",
				disabled = function()
					return state.login.checking:get()
				end,
				on_click = actions.login_web_open,
			},
		},
		web_line(),
		centered_link("Go back to login", actions.login_password_back, theme.size.space_12),
		note(),
	}
end

local function badge(glyph: string, color: any)
	return ui.row {
		w = theme.size.login_badge_size,
		h = theme.size.login_badge_size,
		rounded = theme.size.login_badge_size,
		bg = theme.color.bg_3,
		color = color,
		items = "center",
		justify = "center",
		icon { name = glyph, size = theme.size.space_40 :: any },
	}
end

local function step_heading(glyph: string, color: any, title: any, subtitle: any)
	return ui.column {
		w_full = true,
		items = "center",
		badge(glyph, color),
		line(title, theme.size.font_size_xl, theme.size.line_height_xl, theme.weight.semibold, theme.color.text_2, {
			mt = theme.size.space_16,
			align = "center",
		}),
		line(subtitle, theme.size.font_size_md, theme.size.line_height_label, theme.weight.regular, theme.color.text_4, {
			mt = theme.size.space_8,
			align = "center",
			wrap = true,
			w_full = true,
			literal = true,
		}),
	}
end

local function mfa_subtitle(): string
	local method = password_field("method")() or "totp"
	if method == "backup" then
		return "Enter one of your 8-character backup codes."
	elseif method == "sms" then
		if password_field("awaits_text")() then
			return "Discord will text a code to the phone number on your account."
		end
		local phone = password_field("phone")() or ""
		return if phone ~= "" then `Enter the 6-digit code sent to {phone}.` else "Enter the 6-digit code sent to your phone."
	end
	return "Enter the 6-digit code from your authenticator app."
end

local function mfa_step()
	local code = ui.state("login_code", "")
	ui.effect(function()
		if not table.find(MFA_STAGES, password_stage()) then
			code:set("")
		end
	end)
	local function awaits_text(): boolean
		return password_field("awaits_text")() == true
	end
	local function act()
		if busy() then
			return
		end
		if awaits_text() then
			actions.login_mfa_method("sms")
		else
			actions.login_mfa_verify(code:get())
		end
	end
	local function others(): { string }
		return password_field("others")() or {}
	end
	local function other(method: string)
		return ui.show(function()
			return table.find(others(), method) ~= nil
		end, link(OTHER_METHODS[method], function()
			if not busy() then
				actions.login_mfa_method(method)
			end
		end))
	end
	return ui.column {
		grow = 1,
		min_w = 0,
		step_heading("lock", theme.color.text_3, "Multi-Factor Authentication", mfa_subtitle),
		ui.column {
			w_full = true,
			mt = theme.size.space_20,
			ui.show(
				function()
					return not awaits_text()
				end,
				ui.column {
					w_full = true,
					ui.row {
						mb = theme.size.space_8,
						line(function()
							return if password_field("method")() == "backup" then "ENTER A BACKUP CODE" else "ENTER DISCORD AUTH CODE"
						end, theme.size.font_size_xs, theme.size.line_height_xs, theme.weight.bold, theme.color.text_4),
						line(" *", theme.size.font_size_xs, theme.size.line_height_xs, theme.weight.bold, theme.color.red_2),
					},
					text_field {
						value = code,
						error = function()
							return problem() ~= ""
						end,
						on_commit = act,
					},
				}
			),
			error_line(problem),
			ui.div {
				mt = theme.size.space_20,
				primary {
					text = function()
						local stage = password_stage()
						if stage == "texting" then
							return "Sending code..."
						elseif stage == "verifying" then
							return "Logging in..."
						end
						return if awaits_text() then "Send code" else "Log In"
					end,
					disabled = busy,
					on_click = act,
				},
			},
			ui.column {
				mt = theme.size.space_16,
				gap = theme.size.space_8,
				items = "start",
				other("backup"),
				other("totp"),
				other("sms"),
				link("Go back to login", function()
					if not busy() then
						actions.login_password_back()
					end
				end),
			},
		},
	}
end

local function unsaved_step()
	return ui.column {
		grow = 1,
		min_w = 0,
		step_heading("warning_fill", theme.color.yellow_1, "Your token couldn't be saved", function()
			return `You're signed in as {state.login.unsaved_name:get()}, but Windows Credential Manager refused to store your token.`
		end),
		ui.column {
			w_full = true,
			mt = theme.size.space_20,
			line(
				"If you continue, Kyoko keeps the token in memory only. You'll need to sign in again the next time you open Kyoko.",
				theme.size.font_size_sm,
				theme.size.line_height_sm,
				theme.weight.regular,
				theme.color.text_3,
				{ wrap = true, w_full = true }
			),
			ui.div {
				mt = theme.size.space_20,
				primary {
					text = "Continue for this session only",
					disabled = function()
						return false
					end,
					on_click = actions.login_continue,
				},
			},
			centered_link("Back to login", actions.login_discard_unsaved, theme.size.space_16),
		},
	}
end

local function checkbox()
	return ui.button {
		direction = "row",
		items = "center",
		gap = theme.size.space_10,
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
			w = theme.size.checkbox_size,
			h = theme.size.checkbox_size,
			rounded = theme.size.radius_xs,
			bg = function()
				return if locked() then theme.color.bg_2 else theme.color.accent_2
			end,
			border = theme.size.border_thickness,
			border_color = function()
				return if locked() then theme.color.border else theme.color.button_border
			end,
			items = "center",
			justify = "center",
			color = theme.color.text_0,
			ui.show(state.login.acknowledged, icon { name = "check", size = theme.size.icon_size_xs :: any }),
		},
		line("I understand", theme.size.font_size_md, theme.size.line_height_label, theme.weight.medium, theme.color.text_2),
	}
end

local function notice()
	return ui.column {
		w = theme.size.login_card_width,
		gap = theme.size.space_12,
		px = theme.size.space_20,
		py = theme.size.space_16,
		rounded = theme.size.radius_lg,
		bg = theme.color.background_feedback_warning,
		border = theme.size.border_thickness,
		border_color = theme.color.border_feedback_warning,
		ui.row {
			gap = theme.size.space_12,
			ui.div { color = theme.color.yellow_1, icon { name = "warning_fill", size = theme.size.icon_size_md :: any } },
			line(NOTICE, theme.size.font_size_sm, theme.size.line_height_sm, theme.weight.regular, theme.color.text_3, {
				grow = 1,
				min_w = 0,
				wrap = true,
			}),
		},
		ui.row {
			gap = theme.size.space_12,
			ui.div { w = theme.size.icon_size_md, shrink = 0 },
			checkbox(),
		},
	}
end

local function card()
	return ui.row {
		w = function()
			return if narrow() then theme.size.login_card_narrow else theme.size.login_card_width
		end,
		items = "center",
		gap = theme.size.login_card_gap,
		p = theme.size.space_32,
		rounded = theme.size.radius_lg,
		bg = theme.color.bg_4,
		border = theme.size.border_thickness,
		border_color = theme.color.border,
		ui.switch(view, {
			discord = discord_column(),
			email = email_form(),
			token = token_form(),
			handover = handover_step(),
			mfa = mfa_step(),
			unsaved = unsaved_step(),
		}, ui.div {}),
		ui.show(function()
			return not narrow()
		end, qr_column()),
	}
end

return function()
	local secrets = { ui.state("login_token", ""), ui.state("login_password", ""), ui.state("login_code", "") }
	ui.effect(function()
		if state.login.screen:get() == "main" then
			for _, secret in secrets do
				secret:set("")
			end
		end
	end)
	return ui.column {
		relative = true,
		w_full = true,
		h_full = true,
		bg = theme.color.bg_3,
		color = theme.color.text_3,
		text_size = theme.size.font_size_md,
		font = theme.font.ui,
		title_bar(),
		ui.column {
			grow = 1,
			min_h = 0,
			items = "center",
			justify = "center",
			gap = theme.size.gap,
			p = theme.size.gap,
			card(),
			ui.show(function()
				return view() ~= "unsaved"
			end, notice()),
		},
		web_window(),
	}
end
