local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")

local readable = { text = true, announcement = true, thread = true }

local icons = {
	text = "hash",
	announcement = "megaphone",
	voice = "volume",
	stage = "stage",
	forum = "forum",
	thread = "thread",
}

local function by_id(item: { id: string })
	return item.id
end

local function category(channel: state.Channel)
	return ui.row {
		w_full = true,
		pt = theme.size.space_16,
		pb = theme.size.space_4,
		px = theme.size.space_4,
		gap = theme.size.space_4,
		items = "center",
		icon { name = "chevron_down", size = 12, color = theme.color.text_muted },
		ui.text {
			string.upper(channel.name),
			literal = true,
			grow = 1,
			min_w = 0,
			truncate = true,
			text_size = theme.size.font_tiny,
			weight = theme.weight.semibold,
			color = theme.color.text_muted,
		},
	}
end

local function channel_row(channel: state.Channel)
	if channel.kind == "category" then
		return category(channel)
	end
	local openable = readable[channel.kind] == true
	local function open()
		return state.timeline.channel_id:get() == channel.id
	end
	local function tone()
		if open() then
			return theme.color.interactive_active
		end
		return if channel.unread then theme.color.header_primary else theme.color.interactive_normal
	end
	return ui.row {
		w_full = true,
		h = theme.size.channel_row_height,
		px = theme.size.space_8,
		gap = theme.size.space_8,
		rounded = theme.size.radius_sm,
		items = "center",
		bg = function()
			return if open() then theme.color.modifier_selected else nil
		end,
		hover = { bg = theme.color.modifier_hover },
		cursor = if openable then "pointer" else nil,
		on_click = if openable then actions.select_channel(channel.id) else nil,
		opacity = if channel.muted then 0.5 else 1,
		icon { name = icons[channel.kind] or "hash", size = 20, color = theme.color.text_muted },
		ui.text {
			channel.name,
			literal = true,
			grow = 1,
			min_w = 0,
			truncate = true,
			text_size = theme.size.font_md,
			weight = if channel.unread then theme.weight.semibold else theme.weight.medium,
			color = tone,
		},
		ui.show(
			channel.mentions > 0,
			ui.text {
				tostring(channel.mentions),
				px = 6,
				rounded = 8,
				bg = theme.color.mention_badge,
				text_size = theme.size.font_tiny,
				weight = theme.weight.bold,
				color = theme.color.text_on_brand,
			}
		),
	}
end

local function header()
	return ui.row {
		w_full = true,
		h = theme.size.guild_header_height,
		shrink = 0,
		px = theme.size.space_16,
		items = "center",
		border_sides = { 0, 0, 1, 0 },
		border_color = theme.color.background_tertiary,
		ui.text {
			function()
				return state.sidebar.guild_name:get()
			end,
			literal = true,
			min_w = 0,
			truncate = true,
			text_size = theme.size.font_md,
			weight = theme.weight.semibold,
			color = theme.color.header_primary,
		},
	}
end

return function()
	return ui.column {
		w_full = true,
		grow = 1,
		min_h = 0,
		bg = theme.color.background_secondary,
		header(),
		ui.scroll {
			grow = 1,
			min_h = 0,
			ui.each(state.sidebar.channels, channel_row, {
				key = by_id,
				w_full = true,
				px = theme.size.space_8,
				pb = theme.size.space_16,
				gap = theme.size.space_2,
			}),
		},
	}
end
