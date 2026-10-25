local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")
local widgets = require("@settings/widgets")
local kit = require("@settings/server_kit")

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
local SEGMENT = 32
local SWATCH_BORDER = 2

local draft_name = ui.state("server.role_name", "")
local draft_color = ui.state("server.role_color", "")
local draft_hoist = ui.state("server.role_hoist", false)
local draft_mentionable = ui.state("server.role_mentionable", false)
local draft_granted = ui.state("server.role_granted", {} :: { [string]: boolean })
local tab = ui.state("server.role_tab", 1)
local search = ui.state("server.role_search", "")

local function role(): state.ServerRole?
	return state.server.role:get()
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

local role_row = ui.component(function(props: { entry: state.ServerRoleRow })
	local entry = props.entry
	local hovered = ui.use_state(false)
	return ui.button {
		w_full = true,
		h = theme.size.list_row_height,
		px = theme.size.space_12,
		gap = theme.size.space_12,
		items = "center",
		border = 0,
		focus = { border = 0 },
		rounded = theme.size.radius_sm,
		cursor = "pointer",
		bg = function()
			return if hovered:get() then theme.color.modifier_hover else "#00000000"
		end,
		on_hover = function(inside: boolean)
			hovered:set(inside)
		end,
		on_click = function()
			actions.server_action("role_open", entry.id)
		end,
		icon {
			name = "shield",
			size = theme.number(theme.size.glyph_md)(),
			color = if entry.color ~= "" then entry.color else theme.color.interactive_muted,
		},
		widgets.label(entry.name, theme.size.font_md, theme.color.header_primary, {
			weight = theme.weight.medium,
			literal = true,
			truncate = true,
			grow = 1,
			min_w = 0,
		}),
		ui.row {
			shrink = 0,
			items = "center",
			gap = theme.size.space_4,
			icon { name = "members", size = theme.number(theme.size.glyph_sm)(), color = theme.color.text_muted },
			widgets.label(tostring(entry.members), theme.size.font_sm, theme.color.text_muted, { literal = true }),
		},
		if entry.movable
			then kit.icon_button {
				name = "arrow_up",
				tooltip = "Move up",
				on_click = function()
					actions.server_action("role_move", { id = entry.id, up = true })
				end,
			}
			else nil,
		if entry.movable
			then kit.icon_button {
				name = "arrow_down",
				tooltip = "Move down",
				on_click = function()
					actions.server_action("role_move", { id = entry.id, up = false })
				end,
			}
			else nil,
	}
end)

local function list()
	local function roles()
		return state.server.roles:get()
	end
	return ui.column {
		w_full = true,
		grow = 1,
		min_h = 0,
		kit.title("Roles"),
		ui.row {
			w_full = true,
			shrink = 0,
			items = "center",
			justify = "between",
			pb = theme.size.space_16,
			widgets.hint(function()
				local value = roles()
				local count = if value then value.count else 0
				return string.format("%d %s", count, if count == 1 then "role" else "roles")
			end),
			kit.button {
				text = "Create Role",
				icon = "plus",
				on_click = function()
					actions.server_action("role_create")
				end,
			},
		},
		ui.scroll {
			w_full = true,
			grow = 1,
			min_h = 0,
			ui.each(ui.computed(function()
				local value = roles()
				return if value then value.rows else {}
			end), function(entry: state.ServerRoleRow)
				return role_row { entry = entry }
			end, {
				key = function(row: state.ServerRoleRow)
					return row.id
				end,
				w_full = true,
				gap = theme.size.space_2,
				pb = theme.size.space_32,
			}),
		},
	}
end

local function segmented()
	local pills = {}
	for index, text in TABS do
		table.insert(
			pills,
			ui.button {
				h = SEGMENT,
				px = theme.size.space_12,
				items = "center",
				rounded = theme.size.radius_sm,
				border = 1,
				focus = { border = 1 },
				cursor = "pointer",
				border_color = function()
					return if tab:get() == index then theme.color.brand else theme.color.divider
				end,
				bg = function()
					return if tab:get() == index then theme.color.brand_selected else theme.color.background_secondary
				end,
				on_click = function()
					tab:set(index)
				end,
				ui.text {
					text,
					literal = true,
					text_size = theme.size.font_sm,
					weight = theme.weight.medium,
					color = function()
						return if tab:get() == index then theme.color.interactive_active else theme.color.interactive_normal
					end,
				},
			}
		)
	end
	return ui.row { shrink = 0, gap = theme.size.space_8, pb = theme.size.space_16, table.unpack(pills) }
end

local function switch_row(title: string, note: string, value: any)
	return widgets.row(title, note, widgets.switch {
		label = title,
		checked = function()
			return value:get()
		end,
		on_change = function(checked: boolean)
			value:set(checked)
		end,
	})
end

local function swatch(color: string)
	return ui.button {
		w = theme.size.settings_swatch,
		h = theme.size.settings_swatch,
		p = 0,
		rounded = theme.size.radius_sm,
		border = SWATCH_BORDER,
		focus = { border = SWATCH_BORDER },
		cursor = "pointer",
		bg = color,
		border_color = function()
			return if string.lower(draft_color:get()) == color
				then theme.color.interactive_active
				else theme.color.background_primary
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
		gap = theme.size.space_24,
		if not current.everyone then kit.field("Role name", kit.input(draft_name, "")) else nil,
		kit.field(
			"Role color",
			ui.column {
				w_full = true,
				gap = theme.size.space_12,
				ui.row { w_full = true, flex_wrap = true, gap = theme.size.space_8, table.unpack(swatches) },
				ui.div { w = theme.size.settings_nav_width, kit.input(draft_color, "#99aab5") },
			}
		),
		switch_row(
			"Display role members separately",
			"Show members with this role in their own group in the member list.",
			draft_hoist
		),
		switch_row("Allow anyone to @mention this role", "Members can ping everyone who has this role.", draft_mentionable),
	}
end

local function permissions_tab()
	local sections = {}
	for _, group in state.server.rights:get() do
		local rows = {}
		for _, right in group.rights do
			table.insert(
				rows,
				ui.show(function()
					local wanted = string.lower(search:get())
					return wanted == "" or string.find(string.lower(right.name), wanted, 1, true) ~= nil
				end, widgets.row(right.name, right.hint, widgets.switch {
					label = right.name,
					checked = function()
						return draft_granted:get()[tostring(right.index)] == true
					end,
					on_change = function(checked: boolean)
						local next = table.clone(draft_granted:get())
						next[tostring(right.index)] = if checked then true else nil
						draft_granted:set(next)
					end,
				}))
			)
		end
		table.insert(sections, ui.column { w_full = true, kit.caption(group.title), table.unpack(rows) })
	end
	return ui.column {
		w_full = true,
		gap = theme.size.space_16,
		kit.input(search, "Search permissions"),
		table.unpack(sections),
	}
end

local function editor(current: state.ServerRole)
	load(current)
	tab:set(1)
	search:set("")
	return ui.column {
		w_full = true,
		grow = 1,
		min_h = 0,
		ui.row {
			w_full = true,
			shrink = 0,
			items = "center",
			gap = theme.size.space_12,
			pb = theme.size.space_20,
			kit.icon_button {
				name = "chevron_left",
				tooltip = "Back to roles",
				on_click = function()
					actions.server_action("role_open", "")
				end,
			},
			widgets.label(current.title, theme.size.font_xl, theme.color.header_primary, {
				weight = theme.weight.bold,
				literal = true,
				truncate = true,
				grow = 1,
				min_w = 0,
			}),
			if current.deletable
				then kit.button {
					text = "Delete Role",
					tone = "danger_outline",
					on_click = function()
						actions.server_action("role_delete", current.id)
					end,
				}
				else nil,
		},
		segmented(),
		ui.scroll {
			w_full = true,
			grow = 1,
			min_h = 0,
			ui.column {
				w_full = true,
				pb = theme.size.space_32,
				ui.show(function()
					return tab:get() == 1
				end, display_tab(current), permissions_tab()),
			},
		},
		ui.show(dirty, kit.save_bar(function()
			load(current)
		end, save)),
	}
end

return function()
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
