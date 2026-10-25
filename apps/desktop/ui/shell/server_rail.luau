local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")

local function by_id(item: { id: string })
	return item.id
end

local function guild_button(guild: state.Guild)
	local function chosen()
		return state.sidebar.guild_id:get() == guild.id
	end
	return ui.row {
		w_full = true,
		justify = "center",
		items = "center",
		ui.button {
			w = theme.size.server_icon,
			h = theme.size.server_icon,
			p = 0,
			border = 0,
			focus = { border = 0 },
			rounded = function()
				return if chosen() then 16 else 24
			end,
			bg = function()
				return if chosen() then theme.color.brand else theme.color.background_primary
			end,
			hover = { bg = theme.color.brand },
			justify = "center",
			items = "center",
			cursor = "pointer",
			tooltip = guild.name,
			on_click = actions.select_guild(guild.id),
			ui.text {
				guild.initials,
				literal = true,
				text_size = theme.size.font_sm,
				weight = theme.weight.semibold,
				color = theme.color.header_primary,
			},
		},
	}
end

return function()
	return ui.scroll {
		w = theme.size.server_rail_width,
		h_full = true,
		shrink = 0,
		bg = theme.color.background_tertiary,
		ui.each(state.rail.guilds, guild_button, {
			key = by_id,
			w_full = true,
			gap = theme.size.space_8,
			py = theme.size.space_12,
		}),
	}
end
