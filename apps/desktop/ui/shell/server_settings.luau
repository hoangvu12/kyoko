local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local kit = require("@settings/kit")
local dialog = require("@settings/dialog")

export type ServerPageModule = { keywords: string, build: () -> any, save_bar: (() -> any)? }

local PAGES: { [string]: ServerPageModule } = {
	overview = require("@settings/server_overview"),
	roles = require("@settings/server_roles"),
	members = require("@settings/server_members"),
	bans = require("@settings/server_bans"),
	["audit-log"] = require("@settings/server_audit"),
	emoji = require("@settings/server_emoji"),
	stickers = require("@settings/server_stickers"),
	invites = require("@settings/server_invites"),
}

local function n(token: string): number
	return theme.number(token)()
end

local function sections(): { dialog.Section }
	local guild = state.server.guild:get()
	local found = {}
	for index, group in (guild and guild.groups) or {} do
		local pages = {}
		for _, entry in group.pages do
			local module = PAGES[entry.key]
			if module then
				table.insert(pages, {
					key = entry.key,
					title = entry.title,
					icon = entry.icon,
					keywords = module.keywords,
					build = module.build,
				})
			end
		end
		table.insert(found, { key = tostring(index), title = group.title, pages = pages })
	end
	return found
end

local function find(key: string): dialog.Page?
	for _, section in sections() do
		for _, page in section.pages do
			if page.key == key then
				return page
			end
		end
	end
	return nil
end

local function shape(): string
	local keys = {}
	for _, section in sections() do
		table.insert(keys, section.title or "")
		for _, page in section.pages do
			table.insert(keys, page.key)
		end
	end
	return table.concat(keys, "|")
end

local function guild_field(name: string): () -> string
	return function()
		local guild = state.server.guild:get()
		return if guild then (guild :: any)[name] or "" else ""
	end
end

local function server_card()
	local side = n(theme.size.avatar_size_md)
	return dialog.card(
		ui.presence(function()
			return guild_field("icon")() .. "|" .. guild_field("acronym")()
		end, function()
			local picture = guild_field("icon")()
			return ui.row {
				id = "server_card_icon",
				w = side,
				h = side,
				shrink = 0,
				justify = "center",
				items = "center",
				overflow = "hidden",
				rounded = theme.size.radius_md,
				bg = theme.color.text_1,
				if picture ~= ""
					then ui.image { src = picture, w = side, h = side, fit = "cover" }
					else kit.text(guild_field("acronym")(), theme.size.font_size_md, theme.size.line_height_label, theme.color.accent_5, {
						weight = theme.weight.bold,
					}),
			}
		end),
		guild_field("name"),
		kit.text("Server Settings", theme.size.font_size_sm, theme.size.line_height_sm, theme.color.text_4, {
			weight = theme.weight.medium,
		})
	)
end

return function()
	return dialog.frame {
		prefix = "server",
		label = "Server Settings",
		page_key = function()
			return state.server.page:get()
		end,
		sections = sections,
		shape = shape,
		find = find,
		build = function(page: dialog.Page)
			return if page.build then page.build() else ui.div {}
		end,
		select = function(key: string)
			actions.open_server_settings(key)
		end,
		close = actions.close_server_settings,
		card = server_card,
		overlay = function(key: string)
			local module = PAGES[key]
			return if module and module.save_bar then module.save_bar() else nil
		end,
	}
end
