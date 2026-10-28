local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local motion = require("@components/motion")
local icon = require("@components/icon")
local rail_tile = require("@components/rail_tile")
local rail_gap = require("@components/rail_gap")
local plugin_slot = require("@components/plugin_slot")

local FOLDER_CELLS = 4
local HOME_SPIN = 400
local HOME_TURN = -360
local HOME_SHRINK = 0.8

local avatar_size = theme.number(theme.size.guildbar_avatar_size)
local folder_size = theme.number(theme.size.folder_size)
local cell_size = theme.number(theme.size.icon_size_sm)
local mark_size = theme.number(theme.size.icon_size_lg)
local glyph_size = theme.number(theme.size.icon_size_md)

local guild_style = {
	rest_bg = theme.color.hover,
	rest_fg = theme.color.text_3,
	active_bg = theme.color.active,
	active_fg = theme.color.text_1,
}

local home_style = {
	rest_bg = theme.color.hover,
	rest_fg = theme.color.text_4,
	active_bg = theme.color.active,
	active_fg = theme.color.text_1,
}

local action_style = guild_style

local folder_style = guild_style

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

local function glyph_face(name: string, size: () -> number)
	return function(foreground: () -> string)
		return icon { name = name, size = size(), color = foreground }
	end
end

local function home_face(foreground: () -> string, hovered: any)
	local spinning = motion.moving(hovered)
	return ui.div {
		id = "rail_home_mark",
		rotate = ui.tween(spinning, { from = 0, to = HOME_TURN, duration = HOME_SPIN, ease = "inOutSine" }),
		scale = ui.tween(spinning, { from = 1, to = HOME_SHRINK, duration = HOME_SPIN, ease = "inOutSine" }),
		icon { name = "kyoko", size = mark_size(), color = foreground },
	}
end

local function acronym_size(acronym: string): any
	local count = utf8.len(acronym) or #acronym
	if count <= 2 then
		return theme.size.font_size_md
	elseif count == 3 then
		return theme.size.font_size_sm
	end
	return theme.size.font_size_xs
end

local function guild_face(guild: state.RailGuild)
	return function(foreground: () -> string)
		if guild.icon ~= "" then
			return ui.image { src = guild.icon, w = avatar_size(), h = avatar_size(), fit = "cover" }
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
			then ui.image { src = guild.icon, w = avatar_size(), h = avatar_size(), fit = "cover" }
			else ui.text {
				guild.acronym,
				literal = true,
				text_size = theme.size.font_size_md,
				weight = theme.weight.semibold,
				color = theme.color.text_0,
			}
		return ui.row {
			w = theme.size.guildbar_avatar_size,
			h = theme.size.guildbar_avatar_size,
			rounded = theme.size.radius_md,
			overflow = "hidden",
			opacity = 0.85,
			bg = theme.color.accent_2,
			justify = "center",
			items = "center",
			face,
		}
	end
end

local function voice_avatar(person: state.RailVoice)
	return ui.div {
		w = theme.size.voice_row_avatar,
		h = theme.size.voice_row_avatar,
		shrink = 0,
		rounded = theme.size.voice_row_avatar,
		overflow = "hidden",
		bg = theme.color.bg_2,
		if person.avatar ~= ""
			then ui.image {
				src = person.avatar,
				w = theme.size.voice_row_avatar,
				h = theme.size.voice_row_avatar,
				fit = "cover",
			}
			else nil,
	}
end

local function voice_row(guild: state.RailGuild)
	local people = {}
	for _, person in guild.voice or {} do
		table.insert(people, voice_avatar(person))
	end
	local more = guild.voice_more or 0
	return ui.row {
		id = "rail_tooltip_voice",
		items = "center",
		gap = theme.number(theme.size.space_8),
		icon { name = "volume_fill", size = theme.number(theme.size.icon_size_md)(), color = theme.color.text_4 },
		ui.row {
			items = "center",
			gap = theme.number(theme.size.space_4),
			table.unpack(people),
		},
		if more > 0
			then ui.text {
				"+" .. more,
				text_size = theme.size.font_size_xs,
				weight = theme.weight.semibold,
				color = theme.color.text_4,
			}
			else nil,
	}
end

local function in_voice(guild: state.RailGuild): boolean
	return guild.voice ~= nil and #guild.voice > 0
end

local function card(name: string, voice: any)
	return ui.column {
		max_w = theme.size.reaction_tooltip_width,
		px = theme.size.space_12,
		py = theme.size.space_8,
		gap = theme.number(theme.size.space_8),
		bg = theme.color.bg_3,
		border = theme.size.border_thickness,
		border_color = theme.color.border,
		rounded = theme.size.radius_sm,
		ui.text {
			name,
			literal = true,
			text_size = theme.size.font_size_md,
			weight = theme.weight.semibold,
			color = theme.color.text_2,
		},
		voice,
	}
end

local function guild_card(guild: state.RailGuild)
	return card(guild.name, if in_voice(guild) then voice_row(guild) else nil)
end

local card_offset = theme.number(theme.size.space_8)

local function card_placement()
	return { side = "right", align = "center", offset = card_offset() }
end

local function slot(tile: any, before: string, target: string)
	return ui.div {
		relative = true,
		w_full = true,
		shrink = 0,
		tile,
		rail_gap { drop = before, target = target, on_drop = drop },
	}
end

local function guild_tile(guild: state.RailGuild)
	local tile = rail_tile {
		face = guild_face(guild),
		style = guild_style,
		tooltip = guild_card(guild),
		tooltip_placement = card_placement(),
		speaker = in_voice(guild),
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
		drag = function()
			return { guild = guild.id }
		end,
		preview = guild_preview(guild),
		onto = { drop = "onto_guild", target = guild.id, accepts = accepts_guild_except(guild.id) },
		on_drop = drop,
	}
	return slot(tile, "before_guild", guild.id)
end

local function mini(guild: state.RailGuild)
	local face = if guild.icon ~= ""
		then ui.image { src = guild.icon, w = cell_size(), h = cell_size(), fit = "cover" }
		else ui.text { guild.letter, literal = true }
	return ui.row {
		w = theme.size.icon_size_sm,
		h = theme.size.icon_size_sm,
		shrink = 0,
		rounded = theme.size.radius_xs,
		overflow = "hidden",
		justify = "center",
		items = "center",
		bg = theme.color.bg_2,
		color = theme.color.text_3,
		text_size = theme.size.font_size_tag,
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
		return ui.div {
			grid = true,
			cols = 2,
			gap = theme.number(theme.size.space_4),
			cells,
		}
	end
end

local function folder_preview(folder: state.RailFolder)
	return function()
		return ui.row {
			w = theme.size.folder_size,
			h = theme.size.folder_size,
			rounded = theme.size.radius_lg,
			opacity = 0.85,
			bg = theme.color.hover,
			justify = "center",
			items = "center",
			icon { name = "folder", size = mark_size(), color = folder.color },
		}
	end
end

local function folder_tile(folder: state.RailFolder, open: boolean)
	return rail_tile {
		face = if open then glyph_face("folder", mark_size) else folder_grid(folder),
		style = if open
			then {
				rest_bg = "#00000000",
				rest_fg = folder.color,
				active_bg = "#00000000",
				active_fg = folder.color,
			}
			else folder_style,
		size = if open then theme.size.guildbar_avatar_size else theme.size.folder_size,
		radius = theme.size.radius_lg,
		tooltip = card(folder.name, nil),
		tooltip_placement = card_placement(),
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
	local rows = { slot(folder_tile(folder, true), "before_folder", folder.id) }
	for _, guild in folder.guilds do
		table.insert(rows, guild_tile(guild))
	end
	return ui.column {
		relative = true,
		w_full = true,
		shrink = 0,
		py = theme.size.space_4,
		gap = theme.number(theme.size.space_8),
		ui.row {
			absolute = true,
			top = 0,
			left = 0,
			right = 0,
			bottom = 0,
			justify = "center",
			pointer_events = "none",
			ui.div {
				w = theme.size.folder_size,
				h_full = true,
				rounded = theme.size.radius_lg,
				bg = theme.color.hover,
			},
		},
		table.unpack(rows),
	}
end

local function folder(item: state.RailFolder)
	local function is_open(): boolean
		return table.find(state.rail.open_folders:get(), item.id) ~= nil
	end
	return ui.show(is_open, open_folder(item), slot(folder_tile(item, false), "before_folder", item.id))
end

local function entry(item: state.RailItem)
	if item.kind == "folder" then
		return folder(item :: any)
	end
	return guild_tile(item :: any)
end

local function separator()
	return ui.div {
		w = theme.size.guild_separator_width,
		h = theme.size.border_thickness,
		shrink = 0,
		bg = theme.color.border,
	}
end

return function()
	local home = rail_tile {
		id = "rail_home",
		face = home_face,
		style = home_style,
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
		face = glyph_face("plus", glyph_size),
		style = action_style,
		tooltip = "Add a Server",
		on_click = actions.add_server(),
	}
	local explore = rail_tile {
		face = glyph_face("compass", glyph_size),
		style = action_style,
		tooltip = "Discover",
		on_click = actions.discover(),
	}
	return ui.scroll {
		w_full = true,
		h_full = true,
		ui.column {
			w_full = true,
			items = "center",
			gap = theme.number(theme.size.space_8),
			py = theme.size.guild_list_padding,
			home,
			plugin_slot {
				name = "rail.buttons",
				context = { area = "rail" },
				direction = "column",
				gap = theme.number(theme.size.space_8),
			},
			separator(),
			ui.each(state.rail.items, entry, {
				key = by_key,
				w_full = true,
				gap = theme.number(theme.size.space_8),
			}),
			slot(add, "end", ""),
			explore,
		},
	}
end
