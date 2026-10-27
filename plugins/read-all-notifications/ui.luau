local TILE = "$kyoko.guildbar_avatar_size"
local ICON = "$kyoko.icon_size_md"

local function area_of(slot: any): string
	return if type(slot) == "table" and type(slot.area) == "string" then slot.area else "rail"
end

local read_all = ui.component(function(props: any)
	local hovered = ui.use_state(false)
	local area = area_of(props.slot)
	return ui.button {
		id = "read_all_notifications_" .. area,
		w = TILE,
		h = TILE,
		shrink = 0,
		p = 0,
		border = 0,
		focus = { border = 0 },
		rounded = "$kyoko.radius_md",
		justify = "center",
		items = "center",
		cursor = "pointer",
		bg = function()
			return if hovered:get() then "$kyoko.accent_2" else "$kyoko.hover"
		end,
		tooltip = "Read All Notifications",
		on_hover = function(inside: boolean)
			hovered:set(inside)
		end,
		on_click = function()
			ui.invoke("plugin.read-all-notifications.action", { key = "read_all", slot = "rail.buttons" })
		end,
		ui.svg {
			id = "read_all_notifications_icon_" .. area,
			src = "check_check.svg",
			w = ICON,
			h = ICON,
			shrink = 0,
			tint = function()
				return if hovered:get() then "$kyoko.bg_4" else "$kyoko.text_3"
			end,
		},
	}
end)

ui.contribute("rail.buttons", read_all {})
