local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")
local kit = require("@settings/kit")
local dialog = require("@settings/dialog")

export type ChannelPageModule = { keywords: string, build: () -> any, save_bar: (() -> any)? }

local PAGES: { [string]: ChannelPageModule } = {
	overview = require("@settings/channel_overview"),
	permissions = require("@settings/channel_permissions"),
	invites = require("@settings/channel_invites"),
}

local function info(): state.ChannelInfo?
	return state.channel.info:get()
end

local function sections(): { dialog.Section }
	local current = info()
	local pages = {}
	for _, entry in (current and current.pages) or {} do
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
	return { { key = "channel", pages = pages } }
end

local function find(key: string): dialog.Page?
	for _, page in sections()[1].pages do
		if page.key == key then
			return page
		end
	end
	return nil
end

local function shape(): string
	local keys = {}
	for _, page in sections()[1].pages do
		table.insert(keys, page.key)
	end
	return table.concat(keys, "|")
end

local function channel_card()
	return ui.row {
		id = "channel_settings_card",
		w_full = true,
		shrink = 0,
		items = "center",
		gap = theme.size.space_6,
		pt = theme.size.settings_profile_top,
		px = theme.size.space_24,
		ui.presence(function()
			local current = info()
			return if current then current.icon else nil
		end, function(name: string)
			return icon { name = name, size = theme.size.icon_size_sm :: any, color = theme.color.text_4 }
		end),
		kit.text(function()
			local current = info()
			return if current then current.name else ""
		end, theme.size.font_size_sm, theme.size.line_height_sm, theme.color.text_4, {
			weight = theme.weight.medium,
			truncate = true,
			min_w = 0,
			shrink = 1,
		}),
		kit.text(function()
			local current = info()
			return if current then current.label else ""
		end, theme.size.font_size_xs, theme.size.line_height_xs, theme.color.text_2, {
			weight = theme.weight.bold,
			shrink = 0,
		}),
	}
end

return function()
	return dialog.frame {
		prefix = "channel_settings",
		label = "Channel Settings",
		searchable = false,
		page_key = function()
			return state.channel.page:get()
		end,
		sections = sections,
		shape = shape,
		find = find,
		build = function(page: dialog.Page)
			return if page.build then page.build() else ui.div {}
		end,
		select = function(key: string)
			actions.open_channel_page(key)
		end,
		close = actions.close_channel_settings,
		card = channel_card,
		extras = {
			{
				key = "delete",
				title = "Delete Channel",
				icon = "trash",
				words = "delete channel remove",
				danger = true,
				on_click = function()
					actions.channel_action("delete")
				end,
			},
		},
		overlay = function(key: string)
			local module = PAGES[key]
			return if module and module.save_bar then module.save_bar() else nil
		end,
	}
end
