local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")
local kit = require("./kit")

local PALETTE = {
	"#1abc9c",
	"#2ecc71",
	"#3498db",
	"#9b59b6",
	"#e91e63",
	"#f1c40f",
	"#e67e22",
	"#e74c3c",
	"#95a5a6",
	"#607d8b",
	"#11806a",
	"#1f8b4c",
	"#206694",
	"#71368a",
	"#ad1457",
	"#c27c0e",
	"#a84300",
	"#992d22",
	"#979c9f",
	"#546e7a",
}
local TABS = { "Display", "Permissions" }
local CLEAR = "#00000000"

local draft_name = ui.state("role_name", "")
local draft_color = ui.state("role_color", "")
local draft_hoist = ui.state("role_hoist", false)
local draft_mentionable = ui.state("role_mentionable", false)
local draft_granted = ui.state("role_granted", {} :: { [string]: boolean })
local tab = ui.state("role_tab", 1)
local rights_query = ui.state("rights_query", "")
local roles_query = ui.state("roles_query", "")

local function n(token: string): number
	return theme.number(token)()
end

local function role(): state.ServerRole?
	return state.server.role:get()
end

local function rows(): { state.ServerRoleRow }
	local value = state.server.roles:get()
	return if value then value.rows else {}
end

local function granted_set(list: { number }): { [string]: boolean }
	local set = {}
	for _, index in list do
		set[tostring(index)] = true
	end
	return set
end

local function load(current: state.ServerRole)
	draft_name:set(current.name)
	draft_color:set(current.color)
	draft_hoist:set(current.hoist)
	draft_mentionable:set(current.mentionable)
	draft_granted:set(granted_set(current.granted))
end

local function same_grants(current: state.ServerRole): boolean
	local wanted = draft_granted:get()
	local held = granted_set(current.granted)
	for index in wanted do
		if not held[index] then
			return false
		end
	end
	for index in held do
		if not wanted[index] then
			return false
		end
	end
	return true
end

local function dirty(): boolean
	local current = role()
	if not current then
		return false
	end
	local renamed = not current.everyone and draft_name:get() ~= current.name and draft_name:get() ~= ""
	return renamed
		or string.lower(draft_color:get()) ~= current.color
		or draft_hoist:get() ~= current.hoist
		or draft_mentionable:get() ~= current.mentionable
		or not same_grants(current)
end

local function save()
	local current = role()
	if not current then
		return
	end
	local granted = {}
	for index in draft_granted:get() do
		table.insert(granted, tonumber(index))
	end
	actions.server_action("role_save", {
		id = current.id,
		name = draft_name:get(),
		color = draft_color:get(),
		hoist = draft_hoist:get(),
		mentionable = draft_mentionable:get(),
		granted = granted,
	})
end

local function matches(entry: state.ServerRoleRow): boolean
	local wanted = string.lower(roles_query:get())
	return wanted == "" or string.find(string.lower(entry.name), wanted, 1, true) ~= nil
end

local function listed(): { state.ServerRoleRow }
	local found = {}
	for _, entry in rows() do
		if not entry.everyone and matches(entry) then
			table.insert(found, entry)
		end
	end
	return found
end

local function neighbour(entry: state.ServerRoleRow, step: number): state.ServerRoleRow?
	local all = rows()
	for index, found in all do
		if found.id == entry.id then
			local other = all[index + step]
			return if other and other.movable then other else nil
		end
	end
	return nil
end

local function accepts(entry: state.ServerRoleRow): (any) -> boolean
	return function(payload: any): boolean
		return entry.movable and type(payload) == "table" and payload.role ~= nil and payload.role ~= entry.id
	end
end

local function more_menu(entry: state.ServerRoleRow)
	local open = ui.use_state(false)
	local items = {}
	local above = neighbour(entry, -1)
	local below = neighbour(entry, 1)
	if above then
		table.insert(
			items,
			ui.menu_item {
				"Move Up",
				id = "role_up_" .. entry.id,
				on_click = function()
					actions.server_action("role_drop", { id = entry.id, onto = above.id })
				end,
			}
		)
	end
	if below then
		table.insert(
			items,
			ui.menu_item {
				"Move Down",
				id = "role_down_" .. entry.id,
				on_click = function()
					actions.server_action("role_drop", { id = entry.id, onto = below.id })
				end,
			}
		)
	end
	table.insert(
		items,
		ui.menu_item {
			"Edit Role",
			id = "role_open_" .. entry.id,
			on_click = function()
				actions.server_action("role_open", entry.id)
			end,
		}
	)
	return ui.menu {
		open = open,
		label = "Role options",
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
			id = "role_more_" .. entry.id,
			name = "more",
			tooltip = "More",
			size = n(theme.size.button_size) + n(theme.size.space_4),
			bg = theme.color.bg_3,
			tint = theme.color.text_4,
			on_click = ui.toggle(open),
		},
		table.unpack(items),
	}
end

local MEMBERS_COLUMN = function()
	return n(theme.size.space_40) * 2
end
local ACTIONS_COLUMN = function()
	return n(theme.size.button_size) * 2 + n(theme.size.space_16)
end

local role_row = ui.component(function(props: { entry: state.ServerRoleRow })
	local entry = props.entry
	local hovered = ui.use_state(false)
	local over = ui.use_state(false)
	return ui.row {
		id = "role_row_" .. entry.id,
		w_full = true,
		h = theme.size.space_56,
		px = theme.size.space_8,
		gap = theme.size.space_12,
		items = "center",
		rounded = theme.size.radius_sm,
		border_sides = { 0, 0, 1, 0 },
		border_color = function()
			if over:get() then
				return theme.color.accent_2
			end
			return if hovered:get() then CLEAR else theme.color.border
		end,
		bg = function()
			if over:get() then
				return theme.color.active
			end
			return if hovered:get() then theme.color.hover else CLEAR
		end,
		cursor = "pointer",
		on_hover = function(inside: boolean)
			hovered:set(inside)
		end,
		on_click = function()
			actions.server_action("role_open", entry.id)
		end,
		on_drag = if entry.movable
			then function()
				return { role = entry.id }
			end
			else nil,
		drop_when = accepts(entry),
		on_drag_over = function()
			over:set(true)
		end,
		on_drag_leave = function()
			over:set(false)
		end,
		on_drop = function(payload: any)
			over:set(false)
			actions.server_action("role_drop", { id = payload.role, onto = entry.id })
		end,
		icon {
			name = "grip",
			size = theme.size.icon_size_sm :: any,
			color = if entry.movable then theme.color.text_5 else CLEAR,
		},
		icon {
			name = "shield",
			size = theme.size.icon_size_md :: any,
			color = if entry.color ~= "" then entry.color else theme.color.text_4,
		},
		kit.text(entry.name, theme.size.font_size_md, theme.size.line_height_label, theme.color.text_2, {
			weight = theme.weight.medium,
			truncate = true,
			grow = 1,
			min_w = 0,
		}),
		ui.row {
			w = MEMBERS_COLUMN(),
			shrink = 0,
			items = "center",
			gap = theme.size.space_6,
			kit.text(tostring(entry.members), theme.size.font_size_sm, theme.size.line_height_sm, theme.color.text_4, {
				weight = theme.weight.medium,
			}),
			icon { name = "profile", size = theme.size.icon_size_xs :: any, color = theme.color.text_4 },
		},
		ui.row {
			w = ACTIONS_COLUMN(),
			shrink = 0,
			justify = "end",
			gap = theme.size.space_8,
			kit.round_button {
				id = "role_edit_" .. entry.id,
				name = "edit",
				tooltip = "Edit",
				size = n(theme.size.button_size) + n(theme.size.space_4),
				bg = theme.color.bg_3,
				tint = theme.color.text_4,
				on_click = function()
					actions.server_action("role_open", entry.id)
				end,
			},
			more_menu(entry),
		},
	}
end)

local function everyone_card()
	return ui.presence(function()
		for _, entry in rows() do
			if entry.everyone then
				return entry.id
			end
		end
		return nil
	end, function(id: string)
		return ui.button {
			id = "role_everyone",
			label = "Default Permissions",
			w_full = true,
			gap = theme.size.space_16,
			px = theme.size.space_16,
			py = theme.size.space_12,
			items = "center",
			rounded = theme.size.radius_sm,
			border = 0,
			focus = { border = theme.size.border_thickness, border_color = theme.color.accent_2 },
			bg = theme.color.bg_3,
			hover = { bg = theme.color.bg_2 },
			cursor = "pointer",
			on_click = function()
				actions.server_action("role_open", id)
			end,
			ui.row {
				w = theme.size.avatar_size_md,
				h = theme.size.avatar_size_md,
				shrink = 0,
				rounded = theme.size.avatar_size_md,
				justify = "center",
				items = "center",
				bg = theme.color.bg_2,
				icon { name = "members", size = theme.size.icon_size_md :: any, color = theme.color.text_3 },
			},
			kit.label("Default Permissions", "@everyone • applies to all server members"),
			icon { name = "chevron_right", size = theme.size.icon_size_md :: any, color = theme.color.text_4 },
		}
	end)
end

local function list()
	return ui.column {
		w_full = true,
		gap = theme.size.space_16,
		kit.title("Roles"),
		ui.div {
			w_full = true,
			mt = -n(theme.size.space_8),
			kit.hint("Use roles to group your server members and assign permissions."),
		},
		everyone_card(),
		ui.row {
			w_full = true,
			gap = theme.size.space_12,
			kit.search_input(roles_query, "Search Roles", { id = "role_search" }),
			kit.button {
				id = "role_create",
				text = "Create Role",
				tone = "primary",
				height = theme.size.select_height,
				on_click = function()
					actions.server_action("role_create")
				end,
			},
		},
		kit.hint("Members use the color of the highest role they have on this list. Drag roles to reorder them."),
		ui.column {
			w_full = true,
			ui.row {
				w_full = true,
				gap = theme.size.space_12,
				px = theme.size.space_8,
				pb = theme.size.space_8,
				border_sides = { 0, 0, 1, 0 },
				border_color = theme.color.border,
				ui.div {
					grow = 1,
					min_w = 0,
					pl = n(theme.size.icon_size_sm) + n(theme.size.icon_size_md) + n(theme.size.space_24),
					kit.eyebrow(function()
						return "Roles — " .. tostring(#listed())
					end),
				},
				ui.div { w = MEMBERS_COLUMN(), shrink = 0, kit.eyebrow("Members") },
				ui.div { w = ACTIONS_COLUMN(), shrink = 0 },
			},
			ui.each(ui.computed(listed), function(entry: state.ServerRoleRow)
				return role_row { entry = entry }
			end, {
				key = function(entry: state.ServerRoleRow)
					return entry.id .. "|" .. entry.name .. "|" .. entry.color .. "|" .. tostring(entry.members)
				end,
				w_full = true,
			}),
			ui.show(function()
				return #listed() == 0 and roles_query:get() ~= ""
			end, kit.empty("search", "No roles match your search.")),
		},
	}
end

local function tabs()
	local pills = {}
	for index, text in TABS do
		table.insert(
			pills,
			ui.button {
				id = "role_tab_" .. string.lower(text),
				h = theme.size.button_height_sm,
				px = theme.size.space_12,
				items = "center",
				rounded = theme.size.radius_sm,
				border = 0,
				focus = { border = theme.size.border_thickness, border_color = theme.color.accent_2 },
				cursor = "pointer",
				bg = function()
					return if tab:get() == index then theme.color.bg_2 else CLEAR
				end,
				hover = { bg = theme.color.bg_3 },
				on_click = function()
					tab:set(index)
				end,
				kit.text(text, theme.size.font_size_md, theme.size.line_height_label, function()
					return if tab:get() == index then theme.color.text_2 else theme.color.text_4
				end, { weight = theme.weight.medium }),
			}
		)
	end
	return ui.row {
		w_full = true,
		gap = theme.size.space_8,
		pb = theme.size.space_12,
		border_sides = { 0, 0, 1, 0 },
		border_color = theme.color.border,
		table.unpack(pills),
	}
end

local function switch_line(title: string, note: string, value: any)
	return kit.line(
		title,
		note,
		kit.switch {
			label = title,
			checked = function()
				return value:get()
			end,
			on_change = function(checked: boolean)
				value:set(checked)
			end,
		}
	)
end

local function swatch(color: string)
	return ui.button {
		w = theme.size.settings_swatch,
		h = theme.size.settings_swatch,
		p = 0,
		rounded = theme.size.radius_sm,
		border = theme.size.space_2,
		focus = { border = theme.size.space_2 },
		cursor = "pointer",
		bg = color,
		border_color = function()
			return if string.lower(draft_color:get()) == color then theme.color.text_2 else theme.color.bg_4
		end,
		on_click = function()
			draft_color:set(color)
		end,
	}
end

local function display_tab(current: state.ServerRole)
	local swatches = {}
	for _, color in PALETTE do
		table.insert(swatches, swatch(color))
	end
	return ui.column {
		w_full = true,
		gap = theme.size.settings_row_gap,
		if not current.everyone then kit.field("Role Name", kit.input(draft_name, "", { label = "Role Name" })) else nil,
		kit.field(
			"Role Color",
			ui.column {
				w_full = true,
				gap = theme.size.space_12,
				ui.row { w_full = true, flex_wrap = true, gap = theme.size.space_8, table.unpack(swatches) },
				ui.div { w = theme.size.settings_dialog_nav_width, kit.input(draft_color, "#99aab5", { label = "Role Color" }) },
			}
		),
		switch_line(
			"Display role members separately from online members",
			"Show members with this role in their own group in the member list.",
			draft_hoist
		),
		switch_line("Allow anyone to @mention this role", "Members can ping everyone who has this role.", draft_mentionable),
	}
end

local function permissions_tab()
	local sections = {}
	for _, group in state.server.rights:get() do
		local lines = {}
		for _, right in group.rights do
			table.insert(
				lines,
				ui.show(function()
					local wanted = string.lower(rights_query:get())
					return wanted == "" or string.find(string.lower(right.name), wanted, 1, true) ~= nil
				end, ui.div {
					w_full = true,
					pt = theme.size.space_16,
					kit.line(
						right.name,
						right.hint,
						kit.switch {
							label = right.name,
							checked = function()
								return draft_granted:get()[tostring(right.index)] == true
							end,
							on_change = function(checked: boolean)
								local next = table.clone(draft_granted:get())
								next[tostring(right.index)] = if checked then true else nil
								draft_granted:set(next)
							end,
						}
					),
				})
			)
		end
		table.insert(sections, ui.column { w_full = true, kit.eyebrow(group.title), table.unpack(lines) })
	end
	return ui.column {
		w_full = true,
		gap = theme.size.settings_row_gap,
		kit.search_input(rights_query, "Search permissions"),
		table.unpack(sections),
	}
end

local function editor(current: state.ServerRole)
	load(current)
	tab:set(1)
	rights_query:set("")
	return ui.column {
		w_full = true,
		gap = theme.size.space_16,
		ui.row {
			w_full = true,
			items = "center",
			gap = theme.size.space_12,
			kit.round_button {
				id = "role_back",
				name = "chevron_left",
				tooltip = "Back to roles",
				size = n(theme.size.button_size),
				bg = theme.color.bg_3,
				on_click = function()
					actions.server_action("role_open", "")
				end,
			},
			ui.div {
				grow = 1,
				min_w = 0,
				kit.title("Edit Role — " .. current.title),
			},
			if current.deletable
				then kit.button {
					id = "role_delete",
					text = "Delete Role",
					tone = "critical_secondary",
					on_click = function()
						actions.server_action("role_delete", current.id)
					end,
				}
				else nil,
		},
		tabs(),
		ui.show(function()
			return tab:get() == 1
		end, display_tab(current), permissions_tab()),
	}
end

local function build()
	roles_query:set("")
	return ui.show(function()
		return role() ~= nil
	end, ui.presence(function()
		local current = role()
		return if current then current.id else nil
	end, function()
		local current = role()
		return if current then editor(current) else ui.div {}
	end), list())
end

return {
	keywords = "roles permissions colors color hierarchy create role everyone",
	build = build,
	save_bar = function()
		return ui.show(dirty, kit.save_bar(function()
			local current = role()
			if current then
				load(current)
			end
		end, save))
	end,
}
