local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local kit = require("./kit")

local function field(name: string): () -> string
	return function()
		local found = state.settings.account:get()
		return if found then (found :: any)[name] or "" else ""
	end
end

local function open_web()
	actions.settings_action("open_web")
end

local function shown(name: string, edit: boolean)
	return function()
		return ui.row {
			items = "center",
			gap = theme.size.space_12,
			kit.shown_value(field(name)),
			if edit then kit.button { text = "Edit", on_click = open_web } else nil,
		}
	end
end

local function account_row(title: string, name: string, keywords: string, optional: boolean)
	return {
		kind = "block",
		title = title,
		keywords = keywords,
		visible = if optional
			then function()
				return field(name)() ~= ""
			end
			else nil,
		build = function()
			return kit.line(title, nil, shown(name, true)())
		end,
	}
end

return {
	headings = {
		{
			key = "account-info",
			title = "Account Info",
			rows = {
				account_row("Display Name", "name", "name nickname global", false),
				account_row("Username", "username", "handle tag", false),
				account_row("Email", "email", "mail address", true),
				account_row("Phone Number", "phone", "mobile sms", true),
			},
		},
		{
			key = "security",
			title = "Password & Security",
			rows = {
				{
					kind = "custom",
					title = "Password",
					note = "Change your password in Discord's web app.",
					control = function()
						return kit.button { text = "Edit", on_click = open_web }
					end,
				},
				{
					kind = "custom",
					title = "Multi-Factor Authentication",
					note = "Set up an authenticator app or SMS backup in Discord's web app.",
					keywords = "2fa two-factor mfa authenticator",
					control = function()
						return kit.button { text = "Open Discord", on_click = open_web }
					end,
				},
			},
		},
		{
			key = "sign-out",
			title = "Sign Out",
			rows = {
				{
					kind = "custom",
					title = "Log Out",
					note = "Sign Kyoko out of this account. Your settings stay on this computer.",
					keywords = "logout sign out account",
					control = function()
						return kit.button { id = "settings_log_out", text = "Log Out", tone = "critical", on_click = actions.sign_out() }
					end,
				},
			},
		},
	},
}
