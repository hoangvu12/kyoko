local HIT = 32
local ICON = 24

local settings = ui.shared("plugin.silent-typing.settings", {})

local function setting(key: string): any
	local current = settings:get()
	return if type(current) == "table" then current[key] else nil
end

local function silent(): boolean
	return setting("silent") == true
end

local function area_of(slot: any): string
	return if type(slot) == "table" and type(slot.area) == "string" then slot.area else "composer"
end

local toggle = ui.component(function(props: any)
	local hovered = ui.use_state(false)
	return ui.show(function()
		return setting("button") ~= false
	end, ui.button {
		id = "silent_typing_" .. area_of(props.slot),
		w = HIT,
		h = HIT,
		shrink = 0,
		p = 0,
		bg = "#00000000",
		border = 0,
		focus = { border = 0 },
		rounded = "$kyoko.radius_sm",
		justify = "center",
		items = "center",
		cursor = "pointer",
		tooltip = ui.text { function()
			return if silent() then "Disable Silent Typing" else "Enable Silent Typing"
		end },
		on_hover = function(inside: boolean)
			hovered:set(inside)
		end,
		on_click = function()
			ui.invoke("plugin.silent-typing.set", { key = "silent", value = not silent() })
		end,
		ui.svg {
			id = "silent_typing_icon_" .. area_of(props.slot),
			src = function()
				return if silent() then "keyboard_off.svg" else "keyboard.svg"
			end,
			w = ICON,
			h = ICON,
			shrink = 0,
			tint = function()
				if silent() then
					return "$kyoko.danger"
				end
				return if hovered:get() then "$kyoko.interactive_hover" else "$kyoko.interactive_normal"
			end,
		},
	})
end)

ui.contribute("composer.buttons", toggle {})
