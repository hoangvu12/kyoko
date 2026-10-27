local ICON = 20

local activity = ui.shared("plugin.game-activity-toggle.activity", true)

local function sharing(): boolean
	return activity:get() ~= false
end

local function area_of(slot: any): string
	return if type(slot) == "table" and type(slot.area) == "string" then slot.area else "panel"
end

local toggle = ui.component(function(props: any)
	local hovered = ui.use_state(false)
	local area = area_of(props.slot)
	return ui.button {
		id = "game_activity_toggle_" .. area,
		w = "$kyoko.button_size",
		h = "$kyoko.button_size",
		shrink = 0,
		p = 0,
		border = 0,
		focus = { border = 0 },
		rounded = "$kyoko.radius_sm",
		justify = "center",
		items = "center",
		cursor = "pointer",
		bg = function()
			return if hovered:get() then "$kyoko.hover" else "#00000000"
		end,
		tooltip = ui.text { function()
			return if sharing() then "Disable Game Activity" else "Enable Game Activity"
		end },
		on_hover = function(inside: boolean)
			hovered:set(inside)
		end,
		on_click = function()
			ui.invoke("plugin.game-activity-toggle.action", { key = "toggle", slot = "panel.buttons" })
		end,
		ui.svg {
			id = "game_activity_toggle_icon_" .. area,
			src = function()
				return if sharing() then "gamepad.svg" else "gamepad_off.svg"
			end,
			w = ICON,
			h = ICON,
			shrink = 0,
			tint = function()
				if not sharing() then
					return "$kyoko.danger"
				end
				return if hovered:get() then "$kyoko.text_3" else "$kyoko.text_4"
			end,
		},
	}
end)

ui.contribute("panel.buttons", toggle {})
