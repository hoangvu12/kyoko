local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")
local kit = require("./kit")

local CLEAR = "#00000000"
local DENY = -1
local NEUTRAL = 0
local ALLOW = 1

local draft = ui.state("overwrite_draft", {} :: { [string]: number })
local rights_query = ui.state("overwrite_rights_query", "")
local people_query = ui.state("overwrite_people_query", "")

local function n(token: string): number
	return theme.number(token)()
end

local function overwrites()
	return state.channel.overwrites:get()
end

local function editing(): state.ChannelOverwrite?
	return state.channel.overwrite:get()
end

local function same(left: { [string]: number }, right: { [string]: number }): boolean
	for key, value in left do
		if (right[key] or NEUTRAL) ~= value and value ~= NEUTRAL then
			return false
		end
	end
	for key, value in right do
		if (left[key] or NEUTRAL) ~= value and value ~= NEUTRAL then
			return false
		end
	end
	return true
end

local function dirty(): boolean
	local current = editing()
	return current ~= nil and current.editable and not same(draft:get(), current.choices)
end

local function reset()
	local current = editing()
	draft:set(if current then table.clone(current.choices) else {})
end

local function save()
	local current = editing()
	local groups = overwrites()
	if not current or not groups then
		return
	end
	local choices = {}
	for _, group in groups.groups do
		for _, right in group.rights do
			table.insert(choices, { index = right.index, value = draft:get()[tostring(right.index)] or NEUTRAL })
		end
	end
	actions.channel_action("overwrite_save", { choices = choices })
end

local function follow_editing()
	local seen = ""
	ui.effect(function()
		local current = editing()
		local now = if current then current.serial else ""
		if now ~= seen then
			seen = now
			reset()
		end
	end)
end

local function target_row(entry: state.ChannelOverwriteRow)
	local function selected(): boolean
		local value = overwrites()
		return value ~= nil and value.editing == entry.id
	end
	return ui.button {
		id = "overwrite_row_" .. entry.id,
		label = entry.name,
		w_full = true,
		h = theme.size.settings_item_height,
		px = theme.size.space_10,
		gap = theme.size.space_8,
		items = "center",
		rounded = theme.size.radius_sm,
		border = 0,
		focus = { border = theme.size.border_thickness, border_color = theme.color.accent_2 },
		cursor = "pointer",
		bg = function()
			return if selected() then theme.color.bg_2 else CLEAR
		end,
		hover = { bg = theme.color.bg_3 },
		on_click = function()
			actions.channel_action("overwrite_open", { id = entry.id, member = entry.member })
		end,
		icon {
			name = if entry.member then "profile" else "shield",
			size = theme.size.icon_size_sm :: any,
			color = if entry.color ~= "" then entry.color else theme.color.text_4,
		},
		kit.text(entry.name, theme.size.font_size_md, theme.size.line_height_label, function()
			return if selected() then theme.color.text_2 else theme.color.text_3
		end, {
			weight = theme.weight.medium,
			truncate = true,
			min_w = 0,
			grow = 1,
		}),
	}
end

local add_menu = ui.component(function()
	local open = ui.use_state(false)
	local function candidate_item(entry: state.ChannelCandidate)
		return ui.menu_item {
			entry.name,
			id = "overwrite_add_" .. entry.id,
			on_click = function()
				actions.channel_action("overwrite_add", { id = entry.id, member = entry.member })
			end,
		}
	end
	return ui.menu {
		open = open,
		label = "Add members or roles",
		w = theme.size.menu_width,
		px = theme.size.space_8,
		py = theme.size.space_6,
		gap = theme.size.space_2,
		rounded = theme.size.radius_sm,
		bg = theme.color.bg_3,
		border = theme.size.border_thickness,
		border_color = theme.color.border,
		placement = { side = "bottom", align = "end", offset = 4 },
		anchor = kit.round_button {
			id = "overwrite_add",
			name = "plus",
			tooltip = "Add members or roles",
			size = n(theme.size.button_size),
			bg = theme.color.bg_3,
			on_click = function()
				people_query:set("")
				open:set(not open:get())
			end,
		},
		ui.div {
			w_full = true,
			pb = theme.size.space_4,
			kit.input(people_query, "Search members or roles", {
				id = "overwrite_add_search",
				h = theme.size.button_height_md,
			}),
		},
		ui.each(ui.computed(function()
			return state.channel.candidates:get()
		end), candidate_item, {
			key = function(entry: state.ChannelCandidate)
				return entry.id
			end,
			w_full = true,
		}),
		ui.show(function()
			return #state.channel.candidates:get() == 0
		end, kit.hint("No members or roles left to add.")),
	}
end)

local function targets()
	return ui.column {
		w = theme.size.settings_dialog_nav_width,
		shrink = 0,
		gap = theme.size.space_4,
		ui.row {
			w_full = true,
			items = "center",
			justify = "between",
			pb = theme.size.space_8,
			kit.eyebrow("Roles/Members"),
			add_menu {},
		},
		ui.each(ui.computed(function()
			local value = overwrites()
			return if value then value.rows else {}
		end), target_row, {
			key = function(entry: state.ChannelOverwriteRow)
				return entry.key .. "|" .. entry.name .. "|" .. entry.color
			end,
			w_full = true,
		}),
	}
end

local function tri_button(index: number, value: number, glyph: string, tint: string, fill: string, label: string)
	local key = tostring(index)
	local function chosen(): boolean
		return (draft:get()[key] or NEUTRAL) == value
	end
	return ui.button {
		id = "overwrite_" .. key .. "_" .. tostring(value + 1),
		label = label,
		role = "radio",
		checked = chosen,
		w = theme.size.button_height_sm,
		h = theme.size.button_height_sm,
		p = 0,
		border = 0,
		focus = { border = theme.size.border_thickness, border_color = theme.color.accent_2 },
		justify = "center",
		items = "center",
		cursor = "pointer",
		bg = function()
			return if chosen() then fill else CLEAR
		end,
		on_click = function()
			local editable = editing()
			if not editable or not editable.editable then
				return
			end
			local next = table.clone(draft:get())
			next[key] = if value == NEUTRAL then nil else value
			draft:set(next)
		end,
		icon {
			name = glyph,
			size = theme.size.icon_size_sm :: any,
			color = function()
				return if chosen() then tint else theme.color.text_4
			end,
		},
	}
end

local function tri(index: number, title: string)
	return ui.row {
		shrink = 0,
		overflow = "hidden",
		rounded = theme.size.radius_xs,
		border = theme.size.border_thickness,
		border_color = theme.color.border,
		tri_button(index, DENY, "close", theme.color.text_0, theme.color.red_3, "Deny " .. title),
		tri_button(index, NEUTRAL, "slash", theme.color.text_2, theme.color.bg_2, "Inherit " .. title),
		tri_button(index, ALLOW, "check", theme.color.text_0, theme.color.green_3, "Allow " .. title),
	}
end

local function rights_list()
	local value = overwrites()
	local sections = {}
	for _, group in (value and value.groups) or {} do
		local lines = {}
		for _, right in group.rights do
			table.insert(
				lines,
				ui.show(function()
					local wanted = string.lower(rights_query:get())
					return wanted == "" or string.find(string.lower(right.name), wanted, 1, true) ~= nil
				end, ui.div {
					w_full = true,
					py = theme.size.space_12,
					border_sides = { 0, 0, 1, 0 },
					border_color = theme.color.border,
					kit.line(right.name, right.hint, tri(right.index, right.name)),
				})
			)
		end
		table.insert(
			sections,
			ui.column { w_full = true, gap = theme.size.space_4, kit.eyebrow(group.title), table.unpack(lines) }
		)
	end
	return ui.column { w_full = true, gap = theme.size.settings_row_gap, table.unpack(sections) }
end

local function editor()
	return ui.presence(function()
		local current = editing()
		return if current then current.id else nil
	end, function()
		local current = editing()
		if not current then
			return ui.div {}
		end
		return ui.column {
			id = "overwrite_editor",
			grow = 1,
			basis = 0,
			min_w = 0,
			gap = theme.size.space_16,
			ui.row {
				w_full = true,
				items = "center",
				gap = theme.size.space_12,
				kit.text(current.name, theme.size.font_size_lg, theme.size.line_height_lg, if current.color ~= "" then current.color else theme.color.text_2, {
					weight = theme.weight.semibold,
					truncate = true,
					min_w = 0,
					grow = 1,
				}),
				if not current.everyone and current.editable
					then kit.button {
						id = "overwrite_remove",
						text = "Remove",
						tone = "critical_secondary",
						on_click = function()
							actions.channel_action("overwrite_remove")
						end,
					}
					else nil,
			},
			if not current.editable
				then kit.notice("You can only change overwrites for roles below your highest role.", "lock")
				else nil,
			kit.search_input(rights_query, "Search permissions", { id = "overwrite_search" }),
			rights_list(),
		}
	end)
end

local function build()
	follow_editing()
	rights_query:set("")
	ui.effect(function()
		actions.channel_action("candidates", people_query:get())
	end)
	return ui.column {
		w_full = true,
		gap = theme.size.space_16,
		kit.title("Permissions"),
		kit.hint("Use advanced permissions to change what roles and members can do in this channel. Each permission can be allowed, denied, or left to follow the server's roles."),
		ui.row {
			w_full = true,
			items = "start",
			gap = theme.size.space_24,
			targets(),
			ui.show(function()
				return editing() ~= nil
			end, editor(), ui.column {
				grow = 1,
				basis = 0,
				min_w = 0,
				kit.empty("shield", "Pick a role or member to edit their permissions here."),
			}),
		},
	}
end

return {
	keywords = "channel permissions overwrites roles members allow deny",
	build = build,
	save_bar = function()
		return ui.show(dirty, kit.save_bar(reset, save))
	end,
}
