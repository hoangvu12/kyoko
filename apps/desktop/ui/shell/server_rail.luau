local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")
local rail_tile = require("@components/rail_tile")
local rail_gap = require("@components/rail_gap")
local plugin_slot = require("@components/plugin_slot")

local SIZE = 48
local RAIL = 72
local EDGE = (RAIL - SIZE) / 2
local NESTED_PILL = (SIZE - RAIL) / 2
local FOLDER_CELLS = 4

local brand_style = {
	rest_bg = theme.color.background_primary,
	rest_fg = theme.color.text_normal,
	active_bg = theme.color.brand,
	active_fg = theme.color.text_on_brand,
}

local action_style = {
	rest_bg = theme.color.background_primary,
	rest_fg = theme.color.status_online,
	active_bg = theme.color.status_online,
	active_fg = theme.color.text_on_brand,
}

local function by_key(item: state.RailItem)
	return item.key
end

local function drop(payload: any, kind: string, target: string)
	actions.rail_drop(payload, kind, target)
end

local function accepts_guild_except(id: string): (payload: any) -> boolean
	return function(payload: any)
		return type(payload) == "table" and payload.guild ~= nil and payload.guild ~= id
	end
end

local function accepts_guild(payload: any): boolean
	return type(payload) == "table" and payload.guild ~= nil
end

local function icon_face(name: string, color: string?)
	return function(foreground: () -> string)
		return icon { name = name, size = 26, color = color or foreground }
	end
end

local function acronym_size(acronym: string): any
	local count = utf8.len(acronym) or #acronym
	if count <= 2 then
		local large = theme.number(theme.size.font_lg)
		return function()
			return large() * 0.9
		end
	elseif count == 3 then
		return theme.size.font_md
	end
	return theme.size.font_sm
end

local function guild_face(guild: state.RailGuild)
	return function(foreground: () -> string)
		if guild.icon ~= "" then
			return ui.image { src = guild.icon, w = SIZE, h = SIZE, fit = "cover" }
		end
		return ui.text {
			guild.acronym,
			literal = true,
			text_size = acronym_size(guild.acronym),
			weight = theme.weight.semibold,
			color = foreground,
		}
	end
end

local function guild_preview(guild: state.RailGuild)
	return function()
		local face = if guild.icon ~= ""
			then ui.image { src = guild.icon, w = SIZE, h = SIZE, fit = "cover" }
			else ui.text {
				guild.acronym,
				literal = true,
				text_size = theme.size.font_md,
				weight = theme.weight.semibold,
				color = theme.color.text_on_brand,
			}
		return ui.row {
			w = SIZE,
			h = SIZE,
			rounded = theme.size.radius_lg,
			overflow = "hidden",
			opacity = 0.85,
			bg = theme.color.brand,
			justify = "center",
			items = "center",
			face,
		}
	end
end

local function slot(tile: any, before: string, target: string, inset: number)
	return ui.div {
		relative = true,
		w_full = true,
		shrink = 0,
		tile,
		rail_gap { drop = before, target = target, inset = inset, on_drop = drop },
	}
end

local function guild_tile(guild: state.RailGuild, nested: boolean)
	local tile = rail_tile {
		face = guild_face(guild),
		style = brand_style,
		tooltip = guild.name,
		on_click = actions.select_guild(guild.id),
		selected = function()
			return state.sidebar.guild_id:get() == guild.id
		end,
		unread = function()
			return guild.unread
		end,
		mentions = function()
			return guild.mentions
		end,
		pill_inset = if nested then NESTED_PILL else 0,
		drag = function()
			return { guild = guild.id }
		end,
		preview = guild_preview(guild),
		onto = { drop = "onto_guild", target = guild.id, accepts = accepts_guild_except(guild.id) },
		on_drop = drop,
	}
	return slot(tile, "before_guild", guild.id, if nested then 0 else EDGE)
end

local function mini(guild: state.RailGuild)
	local face = if guild.icon ~= ""
		then ui.image { src = guild.icon, w = theme.size.space_16, h = theme.size.space_16, fit = "cover" }
		else ui.text { guild.letter, literal = true }
	return ui.row {
		w = theme.size.space_16,
		h = theme.size.space_16,
		shrink = 0,
		rounded = 8,
		overflow = "hidden",
		justify = "center",
		items = "center",
		bg = theme.color.background_primary,
		color = theme.color.text_normal,
		text_size = 9,
		face,
	}
end

local function folder_grid(folder: state.RailFolder)
	return function()
		local cells = {}
		for index, guild in folder.guilds do
			if index > FOLDER_CELLS then
				break
			end
			table.insert(cells, mini(guild))
		end
		return ui.row {
			w_full = true,
			h_full = true,
			flex_wrap = true,
			p = 6,
			gap = theme.number(theme.size.space_4),
			content = "start",
			cells,
		}
	end
end

local function folder_preview(folder: state.RailFolder)
	return function()
		return ui.row {
			w = SIZE,
			h = SIZE,
			rounded = theme.size.radius_lg,
			opacity = 0.85,
			bg = folder.tint,
			justify = "center",
			items = "center",
			icon { name = "folder", size = 26, color = folder.color },
		}
	end
end

local function folder_tile(folder: state.RailFolder, open: boolean)
	return rail_tile {
		face = if open then icon_face("folder", folder.color) else folder_grid(folder),
		style = {
			rest_bg = folder.tint,
			rest_fg = theme.color.text_normal,
			active_bg = folder.tint,
			active_fg = theme.color.text_normal,
		},
		tooltip = folder.name,
		on_click = actions.toggle_folder(folder.id),
		unread = if open
			then nil
			else function()
				return folder.unread
			end,
		mentions = if open
			then nil
			else function()
				return folder.mentions
			end,
		drag = function()
			return { folder = folder.id }
		end,
		preview = folder_preview(folder),
		onto = { drop = "onto_folder", target = folder.id, accepts = accepts_guild },
		on_drop = drop,
	}
end

local function open_folder(folder: state.RailFolder)
	local rows = { slot(folder_tile(folder, true), "before_folder", folder.id, 0) }
	for _, guild in folder.guilds do
		table.insert(rows, guild_tile(guild, true))
	end
	return ui.row {
		w_full = true,
		shrink = 0,
		justify = "center",
		ui.column {
			w = SIZE,
			gap = theme.number(theme.size.space_8),
			rounded = theme.size.radius_lg,
			bg = folder.tint,
			rows,
		},
	}
end

local function folder(item: state.RailFolder)
	local function is_open(): boolean
		return table.find(state.rail.open_folders:get(), item.id) ~= nil
	end
	return ui.show(is_open, open_folder(item), slot(folder_tile(item, false), "before_folder", item.id, EDGE))
end

local function entry(item: state.RailItem)
	if item.kind == "folder" then
		return folder(item :: any)
	end
	return guild_tile(item :: any, false)
end

local function divider()
	return ui.div {
		w = theme.size.space_32,
		h = 2,
		shrink = 0,
		bg = theme.color.background_primary,
	}
end

return function()
	local home = rail_tile {
		face = icon_face("home"),
		style = brand_style,
		tooltip = "Direct Messages",
		on_click = actions.select_home(),
		selected = function()
			return state.sidebar.guild_id:get() == ""
		end,
		mentions = function()
			return state.rail.home_mentions:get()
		end,
	}
	local add = rail_tile {
		face = icon_face("plus"),
		style = action_style,
		tooltip = "Add a Server",
		on_click = actions.add_server(),
	}
	local explore = rail_tile {
		face = icon_face("compass"),
		style = action_style,
		tooltip = "Discover",
		on_click = actions.discover(),
	}
	return ui.scroll {
		w = theme.size.server_rail_width,
		h_full = true,
		shrink = 0,
		bg = theme.color.background_tertiary,
		ui.column {
			w_full = true,
			items = "center",
			gap = theme.number(theme.size.space_8),
			py = theme.size.space_12,
			home,
			plugin_slot {
				name = "rail.buttons",
				context = { area = "rail" },
				direction = "column",
				gap = theme.number(theme.size.space_8),
			},
			divider(),
			ui.each(state.rail.items, entry, {
				key = by_key,
				w_full = true,
				gap = theme.number(theme.size.space_8),
			}),
			slot(add, "end", "", EDGE),
			explore,
		},
	}
end
