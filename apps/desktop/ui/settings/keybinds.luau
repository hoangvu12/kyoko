local theme = require("../theme")
local widgets = require("./widgets")
local icon_button = require("@components/icon_button")

local CHIP_WIDTH = 144
local MODIFIERS = { Control = true, Shift = true, Alt = true, Meta = true, OS = true }
local BINDS = {
	{ key = "keybinds.toggle_window", label = "Show or Hide Kyoko" },
}

local function describe(binding: string): string
	local parts = {}
	for part in string.gmatch(binding, "[^+]+") do
		local trimmed = string.gsub(part, "^%s*(.-)%s*$", "%1")
		trimmed = string.gsub(trimmed, "^Key(%u)$", "%1")
		trimmed = string.gsub(trimmed, "^Digit(%d)$", "%1")
		table.insert(parts, string.upper(trimmed))
	end
	return table.concat(parts, " + ")
end

local function binding_of(event: any): string?
	if MODIFIERS[event.key] then
		return nil
	end
	local parts = {}
	if event.ctrl then
		table.insert(parts, "ctrl")
	end
	if event.alt then
		table.insert(parts, "alt")
	end
	if event.shift then
		table.insert(parts, "shift")
	end
	if event.meta then
		table.insert(parts, "super")
	end
	table.insert(parts, event.code or event.key)
	return table.concat(parts, "+")
end

local bind_row = ui.component(function(props: { key: string, label: string })
	local recording = ui.use_state(false)
	local row = ui.ref()
	local function current(): string
		local found = widgets.value(props.key)
		return if type(found) == "string" then found else ""
	end
	return ui.column {
		w_full = true,
		gap = theme.size.space_4,
		ui.row {
			ref = row,
			w_full = true,
			items = "center",
			gap = theme.size.space_16,
			p = theme.size.space_12,
			rounded = theme.size.radius_md,
			bg = theme.color.background_secondary,
			focusable = true,
			on_key = function(event: any): boolean?
				if not recording:get() then
					return false
				end
				if event.key == "Escape" then
					recording:set(false)
					return true
				end
				local binding = binding_of(event)
				if binding then
					recording:set(false)
					widgets.set(props.key, binding)
				end
				return true
			end,
			on_blur = function()
				recording:set(false)
			end,
			widgets.label(props.label, theme.size.font_md, theme.color.header_primary, {
				weight = theme.weight.medium,
				literal = true,
				grow = 1,
				min_w = 0,
				truncate = true,
			}),
			ui.div {
				shrink = 0,
				min_w = CHIP_WIDTH,
				px = theme.size.space_12,
				py = theme.size.space_4,
				rounded = theme.size.radius_sm,
				bg = theme.color.background_tertiary,
				widgets.label(function()
					if recording:get() then
						return "Press a key..."
					end
					local found = current()
					return if found == "" then "Not set" else describe(found)
				end, theme.size.font_sm, function()
					return if recording:get() or current() ~= "" then theme.color.text_normal else theme.color.text_muted
				end, { weight = theme.weight.semibold, literal = true }),
			},
			widgets.button {
				text = "Record Keybind",
				tone = "secondary",
				on_click = function()
					recording:set(true)
					row:focus()
				end,
			},
			icon_button {
				name = "close",
				tooltip = "Clear keybind",
				on_click = function()
					recording:set(false)
					widgets.set(props.key, "")
				end,
			},
		},
		ui.show(function()
			return widgets.note(props.key)() ~= ""
		end, widgets.label(widgets.note(props.key), theme.size.font_xs, theme.color.danger, { literal = true })),
	}
end)

return function()
	local rows = {}
	for _, bind in BINDS do
		table.insert(rows, bind_row(bind))
	end
	table.insert(
		rows,
		widgets.label(
			"Keybinds work while Kyoko is in the background.",
			theme.size.font_xs,
			theme.color.text_muted,
			{ literal = true }
		)
	)
	return ui.column { w_full = true, gap = theme.size.space_12, table.unpack(rows) }
end