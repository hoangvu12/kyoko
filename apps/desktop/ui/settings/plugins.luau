local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")
local widgets = require("./widgets")
local kit = require("./kit")

local CHIP = 20
local CHIP_FILL = 0.18
local BANNER_FILL = 0.14
local BANNER_EDGE = 4
local FIELD = 32
local FIELD_WIDTH = 320
local CELL_WIDTH = 200
local PROMPT_WIDTH = 520
local SAVE_DELAY = 500

local opened = ui.state("plugins_opened", "")
local prompt = ui.state("plugins_prompt", "")
local prompt_open = ui.state("plugins_prompt_open", false)
local filter = ui.state("plugins_filter", "")

local function find(id: string): state.Plugin?
	for _, plugin in state.plugins.list:get() do
		if plugin.id == id then
			return plugin
		end
	end
	return nil
end

local function value_of(id: string, key: string): any
	local plugin = find(id)
	if not plugin then
		return nil
	end
	for _, setting in plugin.settings do
		if setting.key == key then
			return setting.value
		end
	end
	return nil
end

local function status_color(status: string): string
	if status == "running" then
		return theme.color.success
	elseif status == "approval" then
		return theme.color.brand
	elseif status == "suspended" or status == "incompatible" then
		return theme.color.warning
	elseif status == "stopped" or status == "failed" then
		return theme.color.danger
	end
	return theme.color.text_muted
end

local function origin_color(origin: string): string
	if origin == "bundled" then
		return theme.color.success
	elseif origin == "development" then
		return theme.color.link
	end
	return theme.color.warning
end

local function fill(color: string, opacity: number, radius: string)
	return ui.div {
		absolute = true,
		left = 0,
		top = 0,
		right = 0,
		bottom = 0,
		pointer_events = "none",
		rounded = radius,
		bg = color,
		opacity = opacity,
	}
end

local function chip(text: string, color: string)
	return ui.row {
		relative = true,
		h = CHIP,
		px = theme.size.space_8,
		shrink = 0,
		items = "center",
		rounded = theme.size.radius_sm,
		fill(color, CHIP_FILL, theme.size.radius_sm),
		ui.text {
			string.upper(text),
			literal = true,
			text_size = theme.size.font_tiny,
			weight = theme.weight.bold,
			color = color,
		},
	}
end

local function banner(text: any, color: string)
	return ui.row {
		relative = true,
		w_full = true,
		items = "center",
		px = theme.size.space_12,
		py = theme.size.space_8,
		rounded = theme.size.radius_sm,
		overflow = "hidden",
		fill(color, BANNER_FILL, theme.size.radius_sm),
		ui.div { absolute = true, left = 0, top = 0, bottom = 0, w = BANNER_EDGE, bg = color },
		widgets.label(text, theme.size.font_sm, theme.color.text_normal, { literal = true, wrap = true, grow = 1, min_w = 0 }),
	}
end

local function open_prompt(id: string)
	prompt:set(id)
	prompt_open:set(true)
end

local function toggle(plugin: state.Plugin, on: boolean)
	if not on then
		actions.plugin_action("disable", plugin.id)
	elseif plugin.needs_approval then
		open_prompt(plugin.id)
	else
		actions.plugin_action("enable", plugin.id)
	end
end

local function settings_button(plugin: state.Plugin)
	return ui.button {
		id = "plugin_settings_" .. plugin.id,
		tooltip = "Settings",
		label = plugin.name .. " settings",
		w = theme.size.icon_size_md,
		h = theme.size.icon_size_md,
		shrink = 0,
		p = 0,
		border = 0,
		focus = { border = 0 },
		bg = "#00000000",
		cursor = "pointer",
		justify = "center",
		items = "center",
		hover = { brightness = 1.3 },
		on_click = function()
			opened:set(plugin.id)
		end,
		icon { name = "settings", size = theme.size.icon_size_md :: any, color = theme.color.text_5 },
	}
end

local function card(plugin: state.Plugin)
	local problem = if plugin.notice ~= "" then plugin.notice else plugin.error
	local buttons = {}
	if plugin.enabled and plugin.needs_approval then
		table.insert(
			buttons,
			kit.button {
				text = "Review Permissions",
				tone = "primary",
				on_click = function()
					open_prompt(plugin.id)
				end,
			}
		)
	end
	if plugin.origin ~= "bundled" then
		table.insert(
			buttons,
			kit.button {
				text = "Reload",
				on_click = function()
					actions.plugin_action("reload", plugin.id)
				end,
			}
		)
	end
	return ui.column {
		grow = 1,
		basis = 0,
		min_w = 0,
		gap = theme.size.space_8,
		p = theme.size.space_16,
		rounded = theme.size.radius_sm,
		bg = theme.color.bg_3,
		ui.row {
			w_full = true,
			items = "center",
			gap = theme.size.space_12,
			kit.text(plugin.name, theme.size.font_size_md, theme.size.line_height_label, theme.color.text_2, {
				weight = theme.weight.semibold,
				truncate = true,
				grow = 1,
				min_w = 0,
			}),
			settings_button(plugin),
			kit.vc_switch {
				id = "plugin_switch_" .. plugin.id,
				label = plugin.name,
				checked = function(): boolean
					return plugin.enabled and not plugin.needs_approval
				end,
				on_change = function(on: boolean)
					toggle(plugin, on)
				end,
			},
		},
		if plugin.description ~= "" then kit.hint(plugin.description) else nil,
		ui.row {
			items = "center",
			gap = theme.size.space_8,
			wrap = true,
			kit.text("v" .. plugin.version, theme.size.font_size_xs, theme.size.line_height_xs, theme.color.text_4),
			chip(plugin.origin_label, origin_color(plugin.origin)),
			chip(plugin.status_label, status_color(plugin.status)),
		},
		if problem ~= ""
			then kit.text(problem, theme.size.font_size_sm, theme.size.line_height_sm, theme.color.red_2, { wrap = true })
			else nil,
		if #buttons > 0 then ui.row { items = "center", gap = theme.size.space_8, wrap = true, table.unpack(buttons) } else nil,
	}
end

local function any_needing_review(): boolean
	for _, plugin in state.plugins.list:get() do
		if plugin.enabled and plugin.needs_approval then
			return true
		end
	end
	return false
end

local function matching(): { state.Plugin }
	local wanted = string.lower(filter:get())
	local found = {}
	for _, plugin in state.plugins.list:get() do
		local text = string.lower(plugin.name .. " " .. plugin.description .. " " .. plugin.authors)
		if wanted == "" or string.find(text, wanted, 1, true) then
			table.insert(found, plugin)
		end
	end
	return found
end

local function pairs_of()
	return ui.computed(function()
		local rows = {}
		local list = matching()
		for index = 1, #list, 2 do
			table.insert(rows, { first = list[index], second = list[index + 1] })
		end
		return rows
	end)
end

local function grid()
	return ui.each(pairs_of(), function(pair: any)
		return ui.row {
			w_full = true,
			items = "stretch",
			gap = theme.size.space_16,
			card(pair.first),
			if pair.second then card(pair.second) else ui.div { grow = 1, basis = 0 },
		}
	end, {
		key = function(pair: any)
			local function key_of(plugin: state.Plugin?): string
				if not plugin then
					return ""
				end
				return table.concat({
					plugin.id,
					tostring(plugin.enabled),
					tostring(plugin.needs_approval),
					plugin.status,
					plugin.notice,
					plugin.error,
				}, "|")
			end
			return key_of(pair.first) .. "/" .. key_of(pair.second)
		end,
		w_full = true,
		gap = theme.size.space_16,
	})
end

local function filter_field()
	return ui.row {
		w_full = true,
		h = theme.size.settings_search_height,
		px = theme.size.space_12,
		items = "center",
		rounded = theme.size.radius_sm,
		bg = theme.color.bg_3,
		border = theme.size.border_thickness,
		border_color = theme.color.border_light,
		ui.input {
			id = "plugin_filter",
			value = filter,
			label = "Search for a plugin",
			placeholder = "Search for a plugin...",
			placeholder_color = theme.color.text_5,
			grow = 1,
			min_w = 0,
			p = 0,
			bg = "#00000000",
			border = 0,
			focus = { border = 0 },
			text_size = theme.size.font_size_md,
			color = theme.color.text_3,
		},
	}
end

local function installed()
	return ui.column {
		w_full = true,
		gap = theme.size.space_24,
		ui.column {
			w_full = true,
			gap = theme.size.space_8,
			kit.card(
				kit.caption("Plugin Management"),
				kit.hint("Plugins run sandboxed, and each one asks before it can read or change anything. Press a plugin's cog for its settings and permissions."),
				ui.row {
					pt = theme.size.space_8,
					kit.button {
						text = "Open Plugins Folder",
						icon = "folder",
						on_click = function()
							actions.plugin_action("open_folder")
						end,
					},
				}
			),
			ui.show(
				any_needing_review,
				banner("Some plugins ask for permissions you have not reviewed yet.", theme.color.accent_2)
			),
			ui.show(function()
				local notice = state.plugins.notice:get()
				return notice ~= nil and notice.text ~= ""
			end, banner(function(): string
				local notice = state.plugins.notice:get()
				return if notice then notice.text else ""
			end, theme.color.red_2)),
		},
		ui.column {
			w_full = true,
			gap = theme.size.space_8,
			kit.caption("Filters"),
			filter_field(),
		},
		ui.column {
			w_full = true,
			gap = theme.size.space_12,
			kit.caption("Plugins"),
			ui.show(function()
				return #state.plugins.list:get() == 0
			end, kit.hint("No plugins are installed.")),
			ui.show(function()
				return #state.plugins.list:get() > 0 and #matching() == 0
			end, kit.hint("No plugin matches your search.")),
			grid(),
		},
	}
end

local function stepper(id: string, setting: state.PluginSetting)
	local function current(): number
		local found = value_of(id, setting.key)
		return if type(found) == "number" then found else setting.min
	end
	local function nudge(direction: number)
		local next_value = math.clamp(current() + direction * setting.step, setting.min, setting.max)
		actions.plugin_action("set", { id = id, key = setting.key, value = next_value })
	end
	return ui.row {
		items = "center",
		gap = theme.size.space_8,
		widgets.button {
			text = "-",
			small = true,
			on_click = function()
				nudge(-1)
			end,
		},
		widgets.label(function(): string
			return tostring(current())
		end, theme.size.font_md, theme.color.text_normal, { literal = true, min_w = 32 }),
		widgets.button {
			text = "+",
			small = true,
			on_click = function()
				nudge(1)
			end,
		},
	}
end

local text_field = ui.component(function(props: { id: string, setting: state.PluginSetting })
	local initial = tostring(props.setting.value or "")
	local text = ui.use_state(initial)
	local cancel: (() -> ())? = nil
	ui.effect(function()
		local typed = text:get()
		if cancel then
			cancel()
			cancel = nil
		end
		if typed == initial then
			return
		end
		cancel = ui.after(SAVE_DELAY, function()
			actions.plugin_action("set", { id = props.id, key = props.setting.key, value = typed })
		end)
	end)
	local field = if props.setting.multiline then ui.textarea else ui.input
	return field {
		w = FIELD_WIDTH,
		h = if props.setting.multiline then nil else FIELD,
		min_h = FIELD,
		px = theme.size.space_8,
		rounded = theme.size.radius_sm,
		bg = theme.color.input_background,
		border = 0,
		focus = { border = 0 },
		text_size = theme.size.font_sm,
		color = theme.color.text_normal,
		value = text,
	}
end)

type Row = { [string]: any }

local function copy_rows(value: any): { Row }
	local rows = {}
	if type(value) ~= "table" then
		return rows
	end
	for _, row in value do
		local copied = {}
		if type(row) == "table" then
			for key, cell in row do
				copied[key] = cell
			end
		end
		table.insert(rows, copied)
	end
	return rows
end

local cell_text = ui.component(function(props: { id: string, initial: string, multiline: boolean, on_change: (string) -> () })
	local text = ui.use_state(props.initial)
	local last = props.initial
	ui.effect(function()
		local typed = text:get()
		if typed ~= last then
			last = typed
			props.on_change(typed)
		end
	end)
	local field = if props.multiline then ui.textarea else ui.input
	return field {
		id = props.id,
		w = CELL_WIDTH,
		h = if props.multiline then nil else FIELD,
		min_h = FIELD,
		px = theme.size.space_8,
		rounded = theme.size.radius_sm,
		bg = theme.color.input_background,
		border = 0,
		focus = { border = 0 },
		text_size = theme.size.font_sm,
		color = theme.color.text_normal,
		value = text,
	}
end)

local cell_switch = ui.component(function(props: { id: string, label: string, initial: boolean, on_change: (boolean) -> () })
	local on = ui.use_state(props.initial)
	return widgets.switch {
		id = props.id,
		label = props.label,
		checked = function(): boolean
			return on:get()
		end,
		on_change = function(value: boolean)
			on:set(value)
			props.on_change(value)
		end,
	}
end)

local cell_number = ui.component(function(props: { field: state.PluginSetting, initial: number, on_change: (number) -> () })
	local value = ui.use_state(props.initial)
	local function nudge(direction: number)
		local field = props.field
		local next_value = math.clamp(value:get() + direction * field.step, field.min, field.max)
		value:set(next_value)
		props.on_change(next_value)
	end
	return ui.row {
		items = "center",
		gap = theme.size.space_8,
		widgets.button {
			text = "-",
			small = true,
			on_click = function()
				nudge(-1)
			end,
		},
		widgets.label(function(): string
			return tostring(value:get())
		end, theme.size.font_md, theme.color.text_normal, { literal = true, min_w = 32 }),
		widgets.button {
			text = "+",
			small = true,
			on_click = function()
				nudge(1)
			end,
		},
	}
end)

local cell_choice = ui.component(function(props: { field: state.PluginSetting, initial: any, on_change: (any) -> () })
	local chosen = ui.use_state(props.initial)
	return widgets.radios(props.field.options, function(): any
		return chosen:get()
	end, function(value: any)
		chosen:set(value)
		props.on_change(value)
	end)
end)

local function list_cell(prefix: string, row: Row, field: state.PluginSetting, edited: (boolean) -> ())
	local function store(value: any, now: boolean)
		row[field.key] = value
		edited(now)
	end
	local initial = row[field.key]
	if initial == nil then
		initial = field.value
	end
	if field.type == "boolean" then
		return ui.row {
			items = "center",
			gap = theme.size.space_8,
			h = FIELD,
			shrink = 0,
			cell_switch {
				id = prefix .. field.key,
				label = field.label,
				initial = initial == true,
				on_change = function(on: boolean)
					store(on, true)
				end,
			},
			widgets.label(field.label, theme.size.font_sm, theme.color.text_normal, { literal = true }),
		}
	end
	local input: any = nil
	if field.type == "number" then
		input = cell_number {
			field = field,
			initial = if type(initial) == "number" then initial else field.min,
			on_change = function(value: number)
				store(value, true)
			end,
		}
	elseif field.type == "select" then
		input = cell_choice {
			field = field,
			initial = initial,
			on_change = function(value: any)
				store(value, true)
			end,
		}
	else
		input = cell_text {
			id = prefix .. field.key,
			initial = tostring(initial or ""),
			multiline = field.multiline,
			on_change = function(typed: string)
				store(typed, false)
			end,
		}
	end
	return ui.column {
		gap = theme.size.space_4,
		shrink = 0,
		widgets.label(field.label, theme.size.font_tiny, theme.color.text_muted, {
			literal = true,
			weight = theme.weight.semibold,
		}),
		input,
	}
end

local list_editor = ui.component(function(props: { id: string, setting: state.PluginSetting })
	local setting = props.setting
	local base = "plugin_list_" .. setting.key .. "_"
	local rows = copy_rows(setting.value)
	local revision = ui.use_state(0)
	local pending: (() -> ())? = nil
	local function send()
		actions.plugin_action("set", { id = props.id, key = setting.key, value = rows })
	end
	local function save(now: boolean)
		if pending then
			pending()
			pending = nil
		end
		if now then
			send()
			return
		end
		pending = ui.after(SAVE_DELAY, function()
			pending = nil
			send()
		end)
	end
	local function restructure(change: () -> ())
		change()
		save(true)
		revision:set(revision:get() + 1)
	end
	local function blank(): Row
		local row = {}
		for _, field in setting.fields do
			row[field.key] = field.value
		end
		return row
	end
	local function move_button(prefix: string, text: string, from: number, to: number)
		return widgets.button {
			id = prefix,
			text = text,
			small = true,
			on_click = function()
				restructure(function()
					rows[from], rows[to] = rows[to], rows[from]
				end)
			end,
		}
	end
	local function row_view(index: number, row: Row)
		local prefix = base .. index .. "_"
		local cells = {}
		for _, field in setting.fields do
			table.insert(cells, list_cell(prefix, row, field, save))
		end
		local buttons = {}
		if index > 1 then
			table.insert(buttons, move_button(prefix .. "up", "Move Up", index, index - 1))
		end
		if index < #rows then
			table.insert(buttons, move_button(prefix .. "down", "Move Down", index, index + 1))
		end
		table.insert(
			buttons,
			widgets.button {
				id = prefix .. "remove",
				text = "Remove",
				tone = "danger",
				small = true,
				on_click = function()
					restructure(function()
						table.remove(rows, index)
					end)
				end,
			}
		)
		return ui.column {
			w_full = true,
			gap = theme.size.space_12,
			p = theme.size.space_12,
			rounded = theme.size.radius_md,
			bg = theme.color.background_secondary,
			ui.row {
				w_full = true,
				items = "center",
				justify = "between",
				gap = theme.size.space_8,
				widgets.label(setting.item .. " " .. index, theme.size.font_sm, theme.color.header_primary, {
					literal = true,
					weight = theme.weight.semibold,
				}),
				ui.row { items = "center", gap = theme.size.space_4, table.unpack(buttons) },
			},
			ui.row { w_full = true, items = "end", gap = theme.size.space_12, flex_wrap = true, table.unpack(cells) },
		}
	end
	return ui.column {
		w_full = true,
		gap = theme.size.space_8,
		py = theme.size.space_12,
		border_sides = { 0, 0, 1, 0 },
		border_color = theme.color.divider,
		widgets.label(setting.label, theme.size.font_md, theme.color.header_primary, {
			literal = true,
			weight = theme.weight.medium,
		}),
		if setting.description ~= "" then widgets.hint(setting.description) else nil,
		ui.presence(function(): number
			return revision:get()
		end, function()
			if #rows == 0 then
				return widgets.hint("Nothing added yet.")
			end
			local views = {}
			for index, row in rows do
				table.insert(views, row_view(index, row))
			end
			return ui.column { w_full = true, gap = theme.size.space_8, table.unpack(views) }
		end),
		ui.show(
			function(): boolean
				return revision:get() >= 0 and #rows < setting.rows
			end,
			ui.row {
				widgets.button {
					id = base .. "add",
					text = "Add " .. setting.item,
					small = true,
					on_click = function()
						restructure(function()
							table.insert(rows, blank())
						end)
					end,
				},
			}
		),
	}
end)

local function control(id: string, setting: state.PluginSetting): any
	if setting.type == "boolean" then
		return widgets.switch {
			label = setting.label,
			checked = function(): boolean
				return value_of(id, setting.key) == true
			end,
			on_change = function(on: boolean)
				actions.plugin_action("set", { id = id, key = setting.key, value = on })
			end,
		}
	elseif setting.type == "number" then
		return stepper(id, setting)
	elseif setting.type == "string" then
		return text_field { id = id, setting = setting }
	elseif setting.type == "action" then
		return widgets.button {
			text = setting.label,
			small = true,
			on_click = function()
				actions.plugin_action("action", { id = id, key = setting.key })
			end,
		}
	end
	return nil
end

local function field_row(id: string, setting: state.PluginSetting)
	if setting.type == "list" then
		return list_editor { id = id, setting = setting }
	end
	if setting.type == "select" then
		return ui.column {
			w_full = true,
			gap = theme.size.space_8,
			py = theme.size.space_12,
			border_sides = { 0, 0, 1, 0 },
			border_color = theme.color.divider,
			widgets.label(setting.label, theme.size.font_md, theme.color.header_primary, {
				literal = true,
				weight = theme.weight.medium,
			}),
			if setting.description ~= "" then widgets.hint(setting.description) else nil,
			widgets.radios(setting.options, function(): any
				return value_of(id, setting.key)
			end, function(value: any)
				actions.plugin_action("set", { id = id, key = setting.key, value = value })
			end),
		}
	end
	return widgets.row(setting.label, if setting.description ~= "" then setting.description else nil, control(id, setting))
end

local function permission_row(permission: state.PluginPermission)
	return ui.column {
		w_full = true,
		gap = theme.size.space_2,
		ui.row {
			items = "center",
			gap = theme.size.space_8,
			widgets.label(permission.text, theme.size.font_sm, theme.color.text_normal, {
				literal = true,
				wrap = true,
				weight = theme.weight.medium,
				shrink = 1,
				min_w = 0,
			}),
			if permission.sensitive then chip("Sensitive", theme.color.warning) else nil,
			if permission.reserved then chip("Not used yet", theme.color.text_muted) else nil,
		},
		widgets.label(permission.name, theme.size.font_tiny, theme.color.text_muted, { literal = true, font = theme.font.code }),
	}
end

local function detail(id: string)
	local plugin = find(id)
	if not plugin then
		return ui.div {}
	end
	local rows = {}
	for _, setting in plugin.settings do
		table.insert(rows, field_row(id, setting))
	end
	local permissions = {}
	for _, permission in plugin.permissions do
		table.insert(permissions, permission_row(permission))
	end
	return ui.column {
		w_full = true,
		gap = theme.size.space_24,
		ui.row {
			items = "center",
			gap = theme.size.space_12,
			widgets.button {
				text = "Plugins",
				small = true,
				on_click = function()
					opened:set("")
				end,
			},
			widgets.label(plugin.name, theme.size.font_md, theme.color.header_primary, {
				literal = true,
				weight = theme.weight.semibold,
			}),
		},
		if #rows > 0
			then ui.column { w_full = true, table.unpack(rows) }
			else widgets.hint("This plugin has no settings."),
		ui.column {
			w_full = true,
			gap = theme.size.space_12,
			widgets.heading("Permissions"),
			if #permissions > 0
				then ui.column { w_full = true, gap = theme.size.space_12, table.unpack(permissions) }
				else widgets.hint("This plugin does not ask for any permission."),
			if not plugin.needs_approval and #permissions > 0
				then ui.row {
					widgets.button {
						text = "Revoke Permissions",
						tone = "danger",
						small = true,
						on_click = function()
							actions.plugin_action("revoke", id)
						end,
					},
				}
				else nil,
		},
	}
end

local function prompt_body(id: string)
	local plugin = find(id)
	if not plugin then
		return ui.div {}
	end
	local rows = {}
	local names = {}
	for _, permission in plugin.permissions do
		table.insert(rows, permission_row(permission))
		table.insert(names, permission.name)
	end
	return ui.column {
		w_full = true,
		gap = theme.size.space_16,
		widgets.label("Allow " .. plugin.name .. "?", theme.size.font_lg, theme.color.header_primary, {
			literal = true,
			weight = theme.weight.semibold,
		}),
		ui.row {
			items = "center",
			gap = theme.size.space_8,
			widgets.hint("Version " .. plugin.version),
			chip(plugin.origin_label, origin_color(plugin.origin)),
		},
		if #rows > 0
			then ui.column {
				w_full = true,
				gap = theme.size.space_12,
				widgets.heading("This plugin asks for"),
				table.unpack(rows),
			}
			else widgets.hint("This plugin does not ask for any permission."),
		ui.row {
			w_full = true,
			justify = "end",
			gap = theme.size.space_8,
			widgets.button {
				text = "Cancel",
				on_click = function()
					prompt_open:set(false)
				end,
			},
			widgets.button {
				id = "plugin_allow",
				text = if #rows > 0 then "Allow" else "Enable",
				tone = "primary",
				on_click = function()
					prompt_open:set(false)
					actions.plugin_action("approve", { id = id, permissions = names })
				end,
			},
		},
	}
end

local function permission_prompt()
	return ui.modal {
		open = prompt_open,
		label = "Plugin permissions",
		w = PROMPT_WIDTH,
		p = theme.size.space_16,
		rounded = theme.size.radius_md,
		bg = theme.color.background_primary,
		ui.presence(function(): string?
			local id = prompt:get()
			return if id ~= "" then id else nil
		end, prompt_body),
	}
end

return function()
	return ui.column {
		w_full = true,
		ui.presence(function(): string
			local plugin = find(opened:get())
			if not plugin then
				return "list"
			end
			return plugin.id .. "|" .. (if plugin.needs_approval then "review" else "granted")
		end, function(key: string)
			if key == "list" then
				return installed()
			end
			return detail(string.match(key, "^[^|]+") or "")
		end),
		permission_prompt(),
	}
end
