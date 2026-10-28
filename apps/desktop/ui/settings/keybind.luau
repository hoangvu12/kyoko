local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local widgets = require("./widgets")
local icon_button = require("@components/icon_button")

local CHIP_WIDTH = 144
local MODIFIERS = { Control = true, Shift = true, Alt = true, Meta = true, OS = true }

local function describe(binding: string): string
	local parts = {}
	for part in string.gmatch(binding, "[^+]+") do
		local trimmed = string.gsub(part, "^%s*(.-)%s*$", "%1")
		trimmed = string.gsub(trimmed, "^Key(%u)$", "%1")
		trimmed = string.gsub(trimmed, "^Digit(%d)$", "%1")
		trimmed = string.gsub(trimmed, "^Mouse(%d)$", "Mouse %1")
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

local function side_serial(): number
	local pressed = state.keybinds.side_button:get()
	return if pressed then pressed.serial else 0
end

return ui.component(function(props: { key: string, label: string, id: string?, mouse: boolean? })
	local recording = ui.use_state(false)
	local armed = ui.use_state(0)
	local function current(): string
		local found = widgets.value(props.key)
		return if type(found) == "string" then found else ""
	end
	if props.mouse then
		local listening = false
		ui.effect(function()
			local wanted = recording:get()
			if wanted ~= listening then
				listening = wanted
				actions.listen_side_buttons(wanted)
			end
		end)
		ui.effect(function()
			local pressed = state.keybinds.side_button:get()
			if pressed and recording:get() and pressed.serial ~= armed:get() then
				recording:set(false)
				widgets.set(props.key, pressed.binding)
			end
		end)
	end
	return ui.column {
		w_full = true,
		gap = theme.size.space_4,
		ui.row {
			w_full = true,
			items = "center",
			gap = theme.size.space_16,
			p = theme.size.space_12,
			rounded = theme.size.radius_md,
			bg = theme.color.bg_3,
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
				bg = theme.color.bg_2,
				widgets.label(function()
					if recording:get() then
						return if props.mouse then "Press a key or side button..." else "Press a key..."
					end
					local found = current()
					return if found == "" then "Not set" else describe(found)
				end, theme.size.font_sm, function()
					return if recording:get() or current() ~= "" then theme.color.text_normal else theme.color.text_muted
				end, { weight = theme.weight.semibold, literal = true }),
			},
			widgets.button {
				id = if props.id then props.id .. "_button" else nil,
				text = "Record Keybind",
				tone = "secondary",
				on_click = function()
					armed:set(side_serial())
					recording:set(true)
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
