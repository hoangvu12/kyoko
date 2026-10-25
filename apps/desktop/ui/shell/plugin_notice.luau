local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")

local CLOSE = 20
local CLOSE_ICON = 14

local function text(): string
	local notice = state.plugins.notice:get()
	return if notice then notice.text else ""
end

local function link(label: string, on_click: () -> ())
	return ui.button {
		h = CLOSE,
		px = theme.size.space_8,
		shrink = 0,
		border = 0,
		focus = { border = 0 },
		rounded = theme.size.radius_sm,
		cursor = "pointer",
		bg = "#00000000",
		on_click = on_click,
		ui.text {
			label,
			literal = true,
			text_size = theme.size.font_sm,
			weight = theme.weight.semibold,
			color = theme.color.text_on_brand,
		},
	}
end

return function()
	return ui.show(
		function()
			return text() ~= "" and state.settings.page:get() == ""
		end,
		ui.row {
			w_full = true,
			shrink = 0,
			items = "center",
			gap = theme.size.space_8,
			px = theme.size.space_16,
			py = theme.size.space_4,
			bg = theme.color.danger,
			icon { name = "plugins", size = CLOSE_ICON, color = theme.color.text_on_brand },
			ui.text {
				text,
				literal = true,
				wrap = true,
				grow = 1,
				min_w = 0,
				text_size = theme.size.font_sm,
				weight = theme.weight.medium,
				color = theme.color.text_on_brand,
			},
			link("Open Plugins", function()
				actions.open_settings("plugins")
			end),
			ui.button {
				w = CLOSE,
				h = CLOSE,
				p = 0,
				shrink = 0,
				border = 0,
				focus = { border = 0 },
				justify = "center",
				items = "center",
				cursor = "pointer",
				tooltip = "Dismiss",
				bg = "#00000000",
				on_click = function()
					actions.plugin_action("dismiss_notice")
				end,
				icon { name = "close", size = CLOSE_ICON, color = theme.color.text_on_brand },
			},
		}
	)
end
