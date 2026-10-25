local notes = require("./notes")

export type Page = { key: string, title: string, icon: string, build: () -> any }
export type Section = { key: string, title: string?, pages: { Page } }

local M = {}

M.sections = {
	{
		key = "user",
		pages = {
			{ key = "my-account", title = "My Account", icon = "account", build = require("./my_account") },
			{
				key = "profiles",
				title = "Profiles",
				icon = "profile",
				build = notes.page("profile", "Your profile is edited in Discord's web app for now.", true),
			},
			{ key = "privacy", title = "Privacy & Safety", icon = "shield", build = require("./privacy") },
			{
				key = "devices",
				title = "Devices",
				icon = "devices",
				build = notes.page("devices", "Devices are managed in Discord's web app.", true),
			},
			{
				key = "connections",
				title = "Connections",
				icon = "connections",
				build = notes.page("connections", "Connections are managed in Discord's web app."),
			},
			{
				key = "clips",
				title = "Clips",
				icon = "clips",
				build = notes.page("clips", "Clips are not supported in Kyoko."),
			},
			{
				key = "friend-requests",
				title = "Friend Requests",
				icon = "friend_requests",
				build = notes.page("friend_requests", "Who can send you friend requests is managed in Discord's web app.", true),
			},
		},
	},
	{
		key = "app",
		title = "App Settings",
		pages = {
			{ key = "appearance", title = "Appearance", icon = "appearance", build = require("./appearance") },
			{ key = "accessibility", title = "Accessibility", icon = "accessibility", build = require("./accessibility") },
			{ key = "chat", title = "Chat", icon = "chat", build = require("./chat") },
			{ key = "notifications", title = "Notifications", icon = "bell", build = require("./notifications") },
			{ key = "keybinds", title = "Keybinds", icon = "keybinds", build = require("./keybinds") },
			{ key = "language", title = "Language", icon = "language", build = require("./language") },
			{ key = "streamer-mode", title = "Streamer Mode", icon = "streamer", build = require("./streamer") },
			{ key = "advanced", title = "Advanced", icon = "advanced", build = require("./advanced") },
		},
	},
} :: { Section }

function M.find(key: string): Page?
	for _, section in M.sections do
		for _, page in section.pages do
			if page.key == key then
				return page
			end
		end
	end
	return nil
end

function M.add_section(section: Section)
	table.insert(M.sections, section)
end

function M.add_page(section_key: string, page: Page)
	for _, section in M.sections do
		if section.key == section_key then
			table.insert(section.pages, page)
			return
		end
	end
	M.add_section({ key = section_key, pages = { page } })
end

return M
