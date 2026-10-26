local theme = require("../theme")
local icon = require("@components/icon")
local widgets = require("@settings/widgets")

local INPUT = 32
local INPUT_INSET = 11
local BADGE = 20
local SMALL_BUTTON = 28
local BUTTON = 32
local MENU_WIDTH = theme.size.context_menu_width
local MENU_ITEM = 28
local EMPTY_ICON = 48

local M = {}

function M.image(src: string, w: number, h: number)
	if src == "" then
		return ui.div { w = w, h = h, shrink = 0 }
	end
	return ui.image { src = src, w = w, h = h, fit = "contain" }
end

function M.title(text: string)
	return widgets.label(text, theme.size.font_xl, theme.color.header_primary, {
		weight = theme.weight.bold,
		literal = true,
		shrink = 0,
		pb = theme.size.space_20,
	})
end

function M.caption(text: any)
	return widgets.label(function()
		local value = if type(text) == "function" then text() else text
		return string.upper(value)
	end, theme.size.font_tiny, theme.color.header_secondary, {
		weight = theme.weight.bold,
		literal = true,
		shrink = 0,
		pb = theme.size.space_8,
	})
end

function M.empty(name: string, text: string)
	return ui.column {
		w_full = true,
		items = "center",
		gap = theme.size.space_8,
		p = theme.size.space_32,
		icon { name = name, size = EMPTY_ICON, color = theme.color.interactive_muted },
		widgets.label(text, theme.size.font_md, theme.color.text_muted, { literal = true }),
	}
end

function M.input(value: any, placeholder: string, extra: { [string]: any }?)
	local props: { [string]: any } = {
		w_full = true,
		h = INPUT,
		px = INPUT_INSET,
		rounded = theme.size.radius_sm,
		border = 1,
		border_color = theme.color.divider,
		focus = { border = 1, border_color = theme.color.brand },
		bg = theme.color.input_background,
		text_size = theme.size.font_sm,
		color = theme.color.text_normal,
		placeholder = placeholder,
		placeholder_color = theme.color.text_muted,
		value = value,
	}
	for key, entry in extra or {} do
		props[key] = entry
	end
	return ui.input(props)
end

function M.field(caption: string, content: any)
	return ui.column {
		w_full = true,
		gap = theme.size.space_8,
		widgets.label(string.upper(caption), theme.size.font_tiny, theme.color.header_secondary, {
			weight = theme.weight.bold,
			literal = true,
		}),
		content,
	}
end

function M.badge(name: string, color: string, tint: string)
	return ui.row {
		h = BADGE,
		shrink = 0,
		px = theme.size.space_8,
		items = "center",
		rounded = theme.size.radius_sm,
		bg = if tint ~= "" then tint else theme.color.modifier_selected,
		ui.text {
			name,
			literal = true,
			text_size = theme.size.font_tiny,
			weight = theme.weight.bold,
			color = if color ~= "" then color else theme.color.interactive_normal,
		},
	}
end

M.button = ui.component(function(props: {
	text: string,
	tone: string?,
	icon: string?,
	small: boolean?,
	on_click: () -> (),
	id: string?,
})
	local hovered = ui.use_state(false)
	local tone = props.tone or "primary"
	local function fill(): string
		local hover = hovered:get()
		if tone == "primary" then
			return if hover then theme.color.brand_hover else theme.color.brand
		elseif tone == "success" then
			return theme.color.success
		elseif tone == "danger" then
			return if hover then theme.color.danger_hover else theme.color.danger
		end
		return if hover then theme.color.modifier_hover else "#00000000"
	end
	local function ink(): string
		if tone == "outline" or tone == "ghost" then
			return theme.color.text_normal
		elseif tone == "danger_outline" then
			return theme.color.danger
		end
		return theme.color.text_on_brand
	end
	local outlined = tone == "outline" or tone == "danger_outline"
	return ui.button {
		id = props.id,
		h = if props.small then SMALL_BUTTON else BUTTON,
		px = if props.small then theme.size.space_8 else theme.size.space_16,
		gap = theme.size.space_8,
		shrink = 0,
		border = if outlined then 1 else 0,
		border_color = if tone == "danger_outline" then theme.color.danger else theme.color.divider,
		focus = { border = if outlined then 1 else 0 },
		rounded = theme.size.radius_sm,
		justify = "center",
		items = "center",
		cursor = "pointer",
		bg = fill,
		on_hover = function(inside: boolean)
			hovered:set(inside)
		end,
		on_click = props.on_click,
		if props.icon then icon { name = props.icon, size = 16, color = ink } else nil,
		if props.text ~= ""
			then ui.text { props.text, literal = true, text_size = theme.size.font_sm, color = ink }
			else nil,
	}
end)

M.icon_button = ui.component(function(props: { name: string, tooltip: string, on_click: () -> (), tint: string? })
	local hovered = ui.use_state(false)
	return ui.button {
		w = SMALL_BUTTON,
		h = SMALL_BUTTON,
		shrink = 0,
		p = 0,
		border = 0,
		focus = { border = 0 },
		rounded = theme.size.radius_sm,
		justify = "center",
		items = "center",
		cursor = "pointer",
		tooltip = props.tooltip,
		bg = "#00000000",
		on_hover = function(inside: boolean)
			hovered:set(inside)
		end,
		on_click = props.on_click,
		icon {
			name = props.name,
			size = 20,
			color = function()
				if props.tint then
					return props.tint
				end
				return if hovered:get() then theme.color.interactive_hover else theme.color.interactive_normal
			end,
		},
	}
end)

M.select = ui.component(function(props: { labels: { string }, current: () -> number, pick: (number) -> () })
	local open = ui.use_state(false)
	local items = {}
	for index, text in props.labels do
		table.insert(
			items,
			ui.menu_item {
				text,
				h = MENU_ITEM,
				checked = function()
					return props.current() == index
				end,
				on_click = function()
					props.pick(index)
				end,
			}
		)
	end
	return ui.menu {
		open = open,
		label = "Choose",
		w = MENU_WIDTH,
		px = theme.size.space_8,
		py = theme.size.space_6,
		gap = theme.size.space_2,
		rounded = theme.size.radius_sm,
		bg = theme.color.bg_3,
		border = theme.size.border_thickness,
		border_color = theme.color.border,
		placement = { side = "bottom", align = "start", offset = 4 },
		anchor = ui.button {
			h = BUTTON,
			px = theme.size.space_16,
			gap = theme.size.space_8,
			items = "center",
			rounded = theme.size.radius_sm,
			border = 1,
			border_color = theme.color.divider,
			focus = { border = 1 },
			bg = "#00000000",
			cursor = "pointer",
			on_click = ui.toggle(open),
			ui.text {
				function()
					return props.labels[props.current()] or ""
				end,
				literal = true,
				text_size = theme.size.font_sm,
				color = theme.color.text_normal,
			},
			icon { name = "chevron_down", size = 16, color = theme.color.text_normal },
		},
		table.unpack(items),
	}
end)

function M.save_bar(on_reset: () -> (), on_save: () -> ())
	return ui.row {
		w_full = true,
		shrink = 0,
		items = "center",
		justify = "between",
		gap = theme.size.space_16,
		mb = theme.size.space_16,
		px = theme.size.space_16,
		py = theme.size.space_12,
		rounded = theme.size.radius_md,
		bg = theme.color.background_floating,
		border = 1,
		border_color = theme.color.divider,
		widgets.label("Careful, you have unsaved changes", theme.size.font_md, theme.color.text_normal, {
			literal = true,
		}),
		ui.row {
			items = "center",
			gap = theme.size.space_8,
			M.button { text = "Reset", tone = "ghost", on_click = on_reset },
			M.button { text = "Save Changes", tone = "success", on_click = on_save },
		},
	}
end

function M.rows(source: any, build: (any) -> any, empty: any?)
	local rows = ui.computed(function()
		local value = source()
		return if value then value.rows else {}
	end)
	return ui.scroll {
		w_full = true,
		grow = 1,
		min_h = 0,
		ui.column {
			w_full = true,
			pb = theme.size.space_32,
			ui.each(rows, build, { key = function(row: any)
				return row.id
			end, w_full = true }),
			if empty
				then ui.show(function()
					local value = source()
					return value ~= nil and (value.loaded ~= false) and #value.rows == 0
				end, empty)
				else nil,
		},
	}
end

return M
