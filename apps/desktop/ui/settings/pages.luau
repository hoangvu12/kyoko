local state = require("../state")
local kit = require("./kit")
local notes = require("./notes")

export type Page = {
	key: string,
	title: string,
	icon: string,
	build: (() -> any)?,
	headings: { kit.Heading }?,
	keywords: (string | () -> string)?,
	greyed: boolean?,
}
export type Section = { key: string, title: string?, pages: { Page } }

local M = {}

local function page(key: string, title: string, icon: string, spec: any, extra: { [string]: any }?): Page
	local made: { [string]: any } = { key = key, title = title, icon = icon, headings = spec.headings }
	for name, value in extra or {} do
		made[name] = value
	end
	return made :: Page
end

local function plugin_words(): string
	local words = {}
	for _, plugin in state.plugins.list:get() do
		table.insert(words, plugin.name)
		table.insert(words, plugin.description)
	end
	return "extensions addons " .. table.concat(words, " ")
end

local function theme_words(): string
	local words = {}
	for _, entry in state.themes.list:get() do
		table.insert(words, entry.name)
	end
	return "css colours colors links " .. table.concat(words, " ")
end

M.sections = {
	{
		key = "user",
		pages = {
			page("my-account", "My Account", "account", require("./my_account")),
			page(
				"profiles",
				"Profiles",
				"profile",
				notes.page("profiles", "Profiles", "Your profile is edited in Discord's web app for now.", "avatar banner bio pronouns about me")
			),
			page("privacy", "Data & Privacy", "shield", require("./privacy"), { keywords = "privacy safety" }),
			page("notifications", "Notifications", "bell", require("./notifications")),
			page(
				"devices",
				"Devices",
				"devices",
				notes.page("devices", "Devices", "Devices are managed in Discord's web app.", "sessions logged in log out")
			),
			page(
				"connections",
				"Connections",
				"connections",
				notes.page("connections", "Connections", "Connections are managed in Discord's web app.", "accounts spotify github steam")
			),
			page(
				"friend-requests",
				"Friend Requests",
				"friend_requests",
				notes.page(
					"friend-requests",
					"Friend Requests",
					"Who can send you friend requests is managed in Discord's web app.",
					"friends add"
				)
			),
		},
	},
	{
		key = "kyoko",
		title = "Kyoko",
		pages = {
			{ key = "plugins", title = "Plugins", icon = "plugins", build = require("./plugins"), keywords = plugin_words },
			{ key = "themes", title = "Themes", icon = "themes", build = require("./themes"), keywords = theme_words },
			page("safety", "Safety & Privacy", "shield_alert", require("./safety")),
			page("storage", "Storage", "folder", require("./storage")),
		},
	},
	{
		key = "experience",
		title = "Experience",
		pages = {
			page("voice", "Voice & Video", "headphones", require("./voice"), { greyed = true }),
			page("appearance", "Appearance", "appearance", require("./appearance")),
			page("accessibility", "Accessibility", "accessibility", require("./accessibility")),
			page("chat", "Chat", "chat", require("./chat")),
			{
				key = "keybinds",
				title = "Keybinds",
				icon = "keybinds",
				headings = {
					{
						key = "keybinds",
						title = "Keybinds",
						rows = {
							{
								kind = "block",
								title = "Show or Hide Kyoko",
								keywords = "shortcut hotkey keyboard",
								build = require("./keybinds"),
							},
						},
					},
				},
			},
			page("system", "System", "app_window", require("./system")),
			page("language", "Language & Time", "language", require("./language")),
			page("streamer-mode", "Streamer Mode", "streamer", require("./streamer")),
		},
	},
	{
		key = "developer",
		pages = {
			page("advanced", "Developer", "developer", require("./advanced")),
		},
	},
} :: { Section }

function M.find(key: string): Page?
	for _, section in M.sections do
		for _, found in section.pages do
			if found.key == key then
				return found
			end
		end
	end
	return nil
end

function M.build(found: Page): any
	if found.build then
		return found.build()
	end
	return kit.page({ headings = found.headings or {} })
end

function M.add_section(section: Section)
	table.insert(M.sections, section)
end

function M.add_page(section_key: string, added: Page)
	for _, section in M.sections do
		if section.key == section_key then
			table.insert(section.pages, added)
			return
		end
	end
	M.add_section({ key = section_key, pages = { added } })
end

return M
