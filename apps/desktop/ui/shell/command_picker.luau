local theme = require("../theme")
local actions = require("../actions")
local icon = require("@components/icon")

local HUES = { "accent_3", "green_3", "yellow_3", "blue_3", "red_3", "purple_1" }

local function by_key(item: { key: string })
	return item.key
end

local function hue(visual: any): any
	local index = ((visual.hue or 0) % #HUES) + 1
	return (theme.color :: any)[HUES[index]]
end

local function app_icon(visual: any, size: any, letter_size: any, glyph_size: any)
	if visual.kind == "frequent" then
		return ui.row {
			w = size,
			h = size,
			shrink = 0,
			rounded = size,
			items = "center",
			justify = "center",
			bg = theme.color.bg_2,
			icon { name = "recent", size = glyph_size, color = theme.color.text_3 },
		}
	elseif visual.kind == "built_in" then
		return ui.row {
			w = size,
			h = size,
			shrink = 0,
			rounded = theme.size.radius_sm,
			items = "center",
			justify = "center",
			bg = theme.color.bg_2,
			icon { name = "slash", size = glyph_size, color = theme.color.text_3 },
		}
	end
	return ui.row {
		w = size,
		h = size,
		shrink = 0,
		rounded = size,
		overflow = "hidden",
		items = "center",
		justify = "center",
		bg = hue(visual),
		ui.show(
			(visual.image or "") ~= "",
			ui.image { src = visual.image, w = size, h = size, fit = "cover" },
			ui.text {
				visual.letter or "",
				literal = true,
				text_size = letter_size,
				weight = theme.weight.bold,
				color = theme.color.text_0,
			}
		),
	}
end

local function header_icon(visual: any)
	if visual.kind == "frequent" then
		return icon { name = "recent", size = theme.number(theme.size.icon_size_sm)(), color = theme.color.text_4 }
	end
	return app_icon(visual, theme.size.icon_size_sm, theme.size.font_size_tag, theme.size.icon_size_xs)
end

local function section_header(item: any)
	return ui.row {
		w_full = true,
		h = theme.size.command_section_header_height,
		shrink = 0,
		px = theme.size.space_8,
		gap = theme.size.space_8,
		items = "center",
		header_icon(item.app),
		ui.text {
			item.title,
			literal = true,
			truncate = true,
			min_w = 0,
			text_size = theme.size.font_size_sm,
			line_height = theme.size.line_height_sm,
			weight = theme.weight.semibold,
			color = theme.color.text_3,
		},
	}
end

local function chip_label(name: string)
	return ui.text {
		name,
		literal = true,
		shrink = 0,
		h = theme.size.command_chip_height,
		px = theme.size.space_4,
		rounded = theme.size.radius_xs,
		bg = theme.color.bg_4,
		text_size = theme.size.font_size_xs,
		line_height = theme.size.command_chip_height,
		weight = theme.weight.medium,
		color = theme.color.text_3,
	}
end

local function command_row(item: any, shared: any, area: string)
	local function selected(): boolean
		return shared.picker:get().selected == item.index
	end
	local title: { [any]: any } = {
		gap = theme.size.space_4,
		items = "center",
		min_w = 0,
		ui.text {
			item.name,
			literal = true,
			shrink = 0,
			mr = theme.size.space_4,
			text_size = theme.size.font_size_md,
			line_height = theme.size.line_height_label,
			weight = theme.weight.medium,
			color = theme.color.text_2,
		},
	}
	for _, chip in item.chips or {} do
		table.insert(title, chip_label(chip.name))
	end
	return ui.row {
		id = area .. "_command_" .. item.index,
		w_full = true,
		h = theme.size.command_row_height,
		shrink = 0,
		px = theme.size.space_8,
		gap = theme.size.space_12,
		items = "center",
		rounded = theme.size.radius_sm,
		cursor = "pointer",
		bg = function()
			return if selected() then theme.color.hover else "#00000000"
		end,
		hover = { bg = theme.color.hover },
		on_click = function()
			actions.composer_choose(item.index, "", area)
		end,
		ui.show(item.frequent == true, app_icon(item.app, theme.size.avatar_size_sm, theme.size.font_size_sm, theme.size.icon_size_md)),
		ui.column {
			grow = 1,
			min_w = 0,
			ui.row(title),
			ui.text {
				item.description,
				literal = true,
				truncate = true,
				min_w = 0,
				text_size = theme.size.font_size_description,
				line_height = theme.size.line_height_description,
				color = theme.color.text_4,
			},
		},
		ui.text {
			item.app_name,
			literal = true,
			shrink = 0,
			text_size = theme.size.font_size_xs,
			line_height = theme.size.line_height_xs,
			color = theme.color.text_4,
		},
	}
end

local function rail_item(item: any, shared: any, area: string)
	local function current(): boolean
		return shared.picker:get().section == item.index
	end
	return ui.column {
		w_full = true,
		gap = theme.size.space_8,
		items = "center",
		shrink = 0,
		ui.show(item.app.kind == "built_in", ui.div {
			w = theme.size.space_20,
			h = theme.size.border_thickness,
			bg = theme.color.border,
		}),
		ui.row {
			id = area .. "_command_rail_" .. item.index,
			w_full = true,
			items = "center",
			cursor = "pointer",
			tooltip = item.title,
			on_click = function()
				actions.composer_command_section(item.index, area)
			end,
			ui.div {
				w = theme.size.space_4,
				h = theme.size.space_20,
				shrink = 0,
				rounded_tr = theme.number(theme.size.space_4),
				rounded_br = theme.number(theme.size.space_4),
				bg = function()
					return if current() then theme.color.text_2 else "#00000000"
				end,
			},
			ui.row {
				grow = 1,
				justify = "center",
				app_icon(item.app, theme.size.avatar_size_sm, theme.size.font_size_sm, theme.size.icon_size_md),
			},
			ui.div { w = theme.size.space_4, shrink = 0 },
		},
	}
end

local function keep_selected_visible(shared: any, offset: any)
	ui.effect(function()
		local picker = shared.picker:get()
		if not picker.open then
			return
		end
		local header = theme.number(theme.size.command_section_header_height)()
		local row = theme.number(theme.size.command_row_height)()
		local view = theme.number(theme.size.command_picker_height)() - theme.number(theme.size.space_8)()
		local top = 0
		local section_top = 0
		for _, item in picker.items or {} do
			if item.kind == "header" then
				section_top = top
				top += header
			elseif item.index == picker.selected then
				local first = section_top + header == top
				local from = if first then section_top else top
				local current = offset:peek()
				if from < current then
					offset:set(from)
				elseif top + row > current + view then
					offset:set(top + row - view)
				end
				return
			else
				top += row
			end
		end
	end)
end

local M = {}

function M.picker(shared: any, area: string)
	local offset = ui.state(area .. "_command_scroll", 0)
	keep_selected_visible(shared, offset)
	local function items()
		return shared.picker:get().items or {}
	end
	local function rail()
		return shared.picker:get().rail or {}
	end
	return ui.row {
		id = area .. "_command_picker",
		mx = theme.size.space_8,
		mb = theme.size.space_8,
		h = theme.size.command_picker_height,
		shrink = 0,
		overflow = "hidden",
		rounded = theme.size.radius_md,
		bg = theme.color.bg_3,
		border = theme.size.border_thickness,
		border_color = theme.color.border,
		ui.column {
			w = theme.size.command_rail_width,
			h_full = true,
			shrink = 0,
			py = theme.size.space_8,
			gap = theme.size.space_8,
			bg = theme.color.bg_4,
			border = theme.size.border_thickness,
			border_sides = { 0, 1, 0, 0 },
			border_color = theme.color.border,
			ui.each(ui.computed(rail), function(item: any)
				return rail_item(item, shared, area)
			end, { key = by_key, w_full = true, gap = theme.size.space_8 }),
		},
		ui.scroll {
			offset = offset,
			grow = 1,
			min_w = 0,
			h_full = true,
			ui.column {
				w_full = true,
				px = theme.size.space_8,
				pb = theme.size.space_8,
				ui.each(ui.computed(items), function(item: any)
					if item.kind == "header" then
						return section_header(item)
					end
					return command_row(item, shared, area)
				end, { key = by_key, w_full = true }),
			},
		},
	}
end

local function chip(shared: any, area: string, row: any)
	local typed = ui.use_state(row:peek().value or "")
	local measured = ui.use_state(0)
	local box = ui.ref()
	local seen = -1
	ui.effect(function()
		local value = typed:get()
		local current = row:peek()
		if value ~= current.value then
			actions.composer_option_changed(current.index, value, area)
		end
	end)
	ui.effect(function()
		local element = box:get()
		local serial = shared.command:get().serial or 0
		if element == nil or serial == seen then
			return
		end
		seen = serial
		if row:peek().focused then
			(element :: any):focus()
		end
	end)
	local function focused(): boolean
		return row:get().focused == true
	end
	local function shows_value(): boolean
		return focused() or typed:get() ~= ""
	end
	return ui.row {
		id = area .. "_option_" .. row:peek().name,
		relative = true,
		h = theme.size.command_option_height,
		shrink = 0,
		px = theme.size.space_6,
		gap = theme.size.space_4,
		items = "center",
		rounded = theme.size.radius_xs,
		bg = theme.color.bg_4,
		border = theme.size.border_thickness,
		border_color = function()
			local item = row:get()
			if item.invalid then
				return theme.color.red_2
			elseif item.focused and shared.command:get().optional_open ~= true then
				return theme.color.accent_2
			end
			return theme.color.bg_4
		end,
		cursor = "text",
		on_click = function()
			actions.composer_option_focus(row:peek().index, area)
		end,
		ui.text {
			function()
				local name = row:get().name
				return if shows_value() then name .. ":" else name
			end,
			literal = true,
			shrink = 0,
			text_size = theme.size.font_size_sm,
			line_height = theme.size.line_height_sm,
			weight = theme.weight.semibold,
			color = theme.color.text_3,
		},
		ui.text {
			function()
				return typed:get()
			end,
			literal = true,
			absolute = true,
			left = 0,
			top = 0,
			opacity = 0,
			pointer_events = "none",
			text_size = theme.size.font_size_sm,
			line_height = theme.size.line_height_sm,
			on_resize = function(size)
				measured:set(size.w)
			end,
		},
		ui.input {
			id = area .. "_option_input_" .. row:peek().name,
			value = typed,
			ref = box,
			label = row:peek().name,
			w = function()
				if not shows_value() then
					return 0
				end
				return measured:get() + theme.number(theme.size.border_thickness)()
			end,
			h = theme.size.line_height_sm,
			p = 0,
			bg = "#00000000",
			border = 0,
			focus = { border = 0 },
			text_size = theme.size.font_size_sm,
			line_height = theme.size.line_height_sm,
			color = theme.color.text_2,
			on_focus = function()
				actions.composer_option_focus(row:peek().index, area)
			end,
			on_key = function(event)
				return actions.composer_option_key(event, row:peek().index, typed:peek(), area)
			end,
		},
	}
end

local function tail(shared: any, area: string)
	local typed = ui.state(area .. "_command_tail", "")
	local box = ui.ref()
	local seen = -1
	ui.effect(function()
		local element = box:get()
		local command = shared.command:get()
		local serial = command.serial or 0
		if element == nil or serial == seen then
			return
		end
		seen = serial
		if #(command.fields or {}) == 0 then
			(element :: any):focus()
		end
	end)
	ui.effect(function()
		if typed:get() ~= "" then
			typed:set("")
		end
	end)
	return ui.input {
		id = area .. "_command_tail",
		value = typed,
		ref = box,
		label = "Command",
		grow = 1,
		min_w = theme.size.border_thickness,
		h = theme.size.line_height_md,
		p = 0,
		bg = "#00000000",
		border = 0,
		focus = { border = 0 },
		text_size = theme.size.font_size_md,
		color = theme.color.text_3,
		on_key = function(event)
			return actions.composer_option_key(event, 0, "", area)
		end,
	}
end

function M.app_slot(shared: any)
	local function current()
		local app = shared.command:get().app
		if app == nil then
			return {}
		end
		local key = (app.kind or "") .. ":" .. (app.letter or "") .. ":" .. tostring(app.hue or 0) .. ":" .. (app.image or "")
		return { { key = key, app = app } }
	end
	return ui.row {
		h_full = true,
		shrink = 0,
		pl = theme.size.space_16,
		pr = theme.size.space_4,
		items = "center",
		ui.each(ui.computed(current), function(item: any)
			return app_icon(item.app, theme.size.icon_size_lg, theme.size.font_size_xs, theme.size.icon_size_sm)
		end, { key = by_key }),
	}
end
function M.editor(shared: any, area: string)
	local function fields()
		return shared.command:get().fields or {}
	end
	return ui.row {
		grow = 1,
		min_w = 0,
		h_full = true,
		ml = theme.size.space_12,
		gap = theme.size.space_8,
		items = "center",
		overflow = "hidden",
		ui.text {
			function()
				return shared.command:get().name or ""
			end,
			literal = true,
			shrink = 0,
			text_size = theme.size.font_size_md,
			line_height = theme.size.line_height_md,
			weight = theme.weight.semibold,
			color = theme.color.text_2,
		},
		ui.each(ui.computed(fields), function(_item: any, _index: number, row: any)
			return chip(shared, area, row)
		end, { key = by_key, gap = theme.size.space_8, items = "center" }),
		tail(shared, area),
	}
end

local function clear_button(area: string)
	return ui.button {
		id = area .. "_command_clear",
		w = theme.size.icon_size_lg,
		h = theme.size.icon_size_lg,
		shrink = 0,
		p = 0,
		rounded = theme.size.icon_size_lg,
		border = 0,
		focus = { border = 0 },
		items = "center",
		justify = "center",
		cursor = "pointer",
		tooltip = "Clear command",
		bg = theme.color.bg_2,
		hover = { bg = theme.color.bg_1 },
		on_click = function()
			actions.composer_cancel_command(area)
		end,
		icon { name = "close", size = theme.number(theme.size.icon_size_xs)(), color = theme.color.text_3 },
	}
end

function M.hint(shared: any, area: string)
	local function hint()
		return shared.command:get().hint or { name = "", description = "", error = false }
	end
	local function open(): boolean
		return shared.command:get().optional_open == true
	end
	return ui.row {
		id = area .. "_command_hint",
		w_full = true,
		h = theme.size.command_hint_height,
		shrink = 0,
		pl = theme.size.space_16,
		pr = theme.size.space_8,
		gap = theme.size.space_8,
		items = "center",
		text_size = theme.size.font_size_sm,
		line_height = theme.size.line_height_sm,
		ui.text {
			function()
				return hint().name
			end,
			literal = true,
			shrink = 0,
			weight = theme.weight.semibold,
			color = theme.color.text_2,
		},
		ui.text {
			function()
				return hint().description
			end,
			literal = true,
			grow = 1,
			min_w = 0,
			truncate = true,
			color = function()
				return if hint().error then theme.color.red_2 else theme.color.text_4
			end,
		},
		ui.show(function()
			return (shared.command:get().optional_count or 0) > 0
		end, ui.button {
			id = area .. "_command_optional",
			h = theme.size.command_option_height,
			shrink = 0,
			px = theme.size.space_8,
			py = 0,
			rounded = theme.size.radius_xs,
			border = 0,
			focus = { border = 0 },
			cursor = "pointer",
			bg = function()
				return if open() then theme.color.active else "#00000000"
			end,
			hover = { bg = theme.color.hover },
			text_size = theme.size.font_size_sm,
			weight = theme.weight.medium,
			color = function()
				return if open() then theme.color.text_2 else theme.color.link
			end,
			on_click = function()
				actions.composer_toggle_optional(area)
			end,
			ui.text(function()
				return "+" .. tostring(shared.command:get().optional_count or 0) .. " optional"
			end),
		}),
		clear_button(area),
	}
end

function M.optional(shared: any, area: string)
	local function rows()
		return shared.command:get().optional or {}
	end
	return ui.column {
		id = area .. "_command_optional_list",
		mx = theme.size.space_8,
		mb = theme.size.space_8,
		p = theme.size.space_8,
		gap = theme.size.space_2,
		shrink = 0,
		rounded = theme.size.radius_md,
		bg = theme.color.bg_3,
		border = theme.size.border_thickness,
		border_color = theme.color.border,
		ui.row {
			h = theme.size.space_24,
			px = theme.size.space_8,
			items = "center",
			ui.text {
				"OPTIONAL",
				literal = true,
				text_size = theme.size.font_size_xs,
				line_height = theme.size.line_height_xs,
				weight = theme.weight.semibold,
				color = theme.color.text_4,
			},
		},
		ui.each(ui.computed(rows), function(item: any, index: number)
			local function selected(): boolean
				return shared.command:get().optional_selected == index
			end
			return ui.row {
				id = area .. "_command_optional_" .. index,
				w_full = true,
				h = theme.size.autocomplete_row_height,
				px = theme.size.space_8,
				gap = theme.size.space_8,
				items = "center",
				rounded = theme.size.radius_sm,
				cursor = "pointer",
				bg = function()
					return if selected() then theme.color.hover else "#00000000"
				end,
				hover = { bg = theme.color.hover },
				on_click = function()
					actions.composer_add_optional(index, area)
				end,
				ui.text {
					item.name,
					literal = true,
					shrink = 0,
					text_size = theme.size.font_size_md,
					line_height = theme.size.line_height_label,
					weight = theme.weight.medium,
					color = theme.color.text_2,
				},
				ui.text {
					item.description,
					literal = true,
					min_w = 0,
					truncate = true,
					text_size = theme.size.font_size_sm,
					line_height = theme.size.line_height_sm,
					color = theme.color.text_4,
				},
			}
		end, { key = by_key, w_full = true, gap = theme.size.space_2 }),
	}
end

return M
