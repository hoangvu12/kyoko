local theme = require("../theme")
local state = require("../state")
local icon = require("@components/icon")

local CONTENT_WIDTH = 560
local BADGE = 64
local BADGE_ICON = 28
local DOT = 8
local CHIP = 28

local icon_sm = theme.number(theme.size.icon_size_sm)

local function view(): state.HiddenView
	return state.hidden.view:get()
end

local function by_key(item: { key: string }): string
	return item.key
end

local function heading(text: string)
	return ui.text {
		text,
		text_size = theme.size.font_size_xs,
		line_height = theme.size.line_height_xs,
		weight = theme.weight.semibold,
		color = theme.color.text_5,
	}
end

local function section(title: string, ...: any)
	return ui.column {
		w_full = true,
		gap = theme.size.space_6,
		p = theme.size.space_12,
		rounded = theme.size.radius_md,
		bg = theme.color.bg_3,
		border = theme.size.border_thickness,
		border_color = theme.color.border,
		heading(title),
		...,
	}
end

local function body(text: () -> string, id: string?)
	return ui.text {
		text,
		id = id,
		literal = true,
		w_full = true,
		wrap = true,
		text_size = theme.size.font_size_md,
		line_height = theme.size.line_height_md,
		color = theme.color.text_3,
	}
end

local function chip(entry: state.HiddenOverwrite, prefix: string)
	return ui.row {
		id = `{prefix}_{entry.key}`,
		h = CHIP,
		px = theme.size.space_8,
		gap = theme.size.space_6,
		shrink = 0,
		items = "center",
		rounded = theme.size.radius_sm,
		bg = theme.color.bg_4,
		border = theme.size.border_thickness,
		border_color = theme.color.border,
		if entry.member
			then icon { name = "profile", size = icon_sm(), color = theme.color.text_4 }
			else ui.div {
				w = DOT,
				h = DOT,
				shrink = 0,
				rounded = DOT,
				bg = if entry.color ~= "" then entry.color else theme.color.text_4,
			},
		ui.text {
			entry.name,
			literal = true,
			text_size = theme.size.font_size_sm,
			line_height = theme.size.line_height_sm,
			color = theme.color.text_3,
		},
	}
end

local function people(field: string, prefix: string, empty: string)
	local list = ui.computed(function()
		return (view() :: any)[field]
	end)
	return ui.column {
		w_full = true,
		ui.show(function()
			return #list:get() == 0
		end, ui.text {
			empty,
			text_size = theme.size.font_size_sm,
			line_height = theme.size.line_height_sm,
			color = theme.color.text_5,
		}),
		ui.each(list, function(entry: state.HiddenOverwrite)
			return chip(entry, prefix)
		end, { key = by_key, w_full = true, flex_wrap = true, gap = theme.size.space_6 }),
	}
end

local function tag(text: () -> string)
	return ui.show(function()
		return text() ~= ""
	end, ui.text {
		text,
		literal = true,
		px = theme.size.space_6,
		rounded = theme.size.radius_xs,
		bg = theme.color.bg_3,
		text_size = theme.size.font_size_xs,
		line_height = theme.size.line_height_xs,
		weight = theme.weight.medium,
		color = theme.color.text_4,
	})
end

local function title()
	return ui.column {
		w_full = true,
		items = "center",
		gap = theme.size.space_8,
		ui.row {
			w = BADGE,
			h = BADGE,
			rounded = BADGE,
			justify = "center",
			items = "center",
			bg = theme.color.bg_3,
			border = theme.size.border_thickness,
			border_color = theme.color.border,
			icon { name = "lock", size = BADGE_ICON, color = theme.color.text_4 },
		},
		ui.row {
			items = "center",
			gap = theme.size.space_6,
			icon {
				name = function()
					return view().icon
				end,
				size = icon_sm(),
				color = theme.color.text_4,
			},
			ui.text {
				function()
					return view().name
				end,
				id = "hidden_channel_name",
				literal = true,
				text_size = theme.size.font_size_xl,
				line_height = theme.size.line_height_xl,
				weight = theme.weight.semibold,
				color = theme.color.text_2,
			},
		},
		ui.text {
			function()
				return `{view().kind} · You do not have access to this channel`
			end,
			literal = true,
			text_size = theme.size.font_size_sm,
			line_height = theme.size.line_height_sm,
			color = theme.color.text_4,
		},
		ui.row {
			gap = theme.size.space_6,
			tag(function()
				return if view().nsfw then "Age-Restricted" else ""
			end),
			tag(function()
				return view().slowmode
			end),
		},
	}
end

return function()
	return ui.scroll {
		id = "hidden_channel_view",
		w_full = true,
		grow = 1,
		min_h = 0,
		ui.column {
			w_full = true,
			items = "center",
			px = theme.size.space_24,
			py = theme.size.space_32,
			ui.column {
				w_full = true,
				max_w = CONTENT_WIDTH,
				gap = theme.size.space_16,
				title(),
				ui.show(function()
					return view().topic ~= ""
				end, section(
					"TOPIC",
					body(function()
						return view().topic
					end, "hidden_channel_topic")
				)),
				section(
					"LAST ACTIVITY",
					body(function()
						return view().activity
					end, "hidden_channel_activity"),
					ui.text {
						function()
							return view().created
						end,
						literal = true,
						text_size = theme.size.font_size_sm,
						line_height = theme.size.line_height_sm,
						color = theme.color.text_4,
					}
				),
				section("ALLOWED TO VIEW", people("allowed", "hidden_allowed", "No role or member is let in by name.")),
				section("NOT ALLOWED TO VIEW", people("denied", "hidden_denied", "No role or member is kept out by name.")),
			},
		},
	}
end
