local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local avatar = require("@components/avatar")
local icon_button = require("@components/icon_button")

local WIDTH = 340
local AVATAR = 24
local CHIP_HEIGHT = 20
local CHIP_WASH = 0.18
local LEADING = 1.618
local SNIPPET_LINES = 4

local font_tiny = theme.number(theme.size.font_tiny)
local font_sm = theme.number(theme.size.font_sm)
local font_md = theme.number(theme.size.font_md)

local function line(size: () -> number): () -> number
	return function()
		return size() * LEADING
	end
end

local function chip(label: string)
	return ui.row {
		relative = true,
		h = CHIP_HEIGHT,
		shrink = 0,
		items = "center",
		px = theme.size.space_8,
		rounded = theme.size.radius_sm,
		overflow = "hidden",
		ui.div {
			absolute = true,
			left = 0,
			top = 0,
			right = 0,
			bottom = 0,
			bg = theme.color.brand,
			opacity = CHIP_WASH,
		},
		ui.text {
			label,
			literal = true,
			text_size = theme.size.font_tiny,
			weight = theme.weight.bold,
			color = theme.color.brand,
		},
	}
end

local function by_key(row: state.SearchHit): string
	return row.key
end

local function chips()
	return ui.show(
		function()
			return #state.search.chips:get() > 0
		end,
		ui.row {
			shrink = 0,
			flex_wrap = true,
			gap = theme.size.space_4,
			px = theme.size.space_16,
			pt = theme.size.space_8,
			ui.each(state.search.chips, function(label: string)
				return chip(label)
			end),
		}
	)
end

local function is_last(id: string): boolean
	local hits = state.search.hits:get()
	return #hits > 0 and hits[#hits].id == id
end

local function header()
	return ui.row {
		h = theme.size.top_bar_height,
		shrink = 0,
		items = "center",
		justify = "between",
		px = theme.size.space_16,
		border_sides = { 0, 0, 1, 0 },
		border_color = theme.color.background_tertiary,
		ui.text {
			function()
				return state.search.panel:get().title
			end,
			literal = true,
			text_size = theme.size.font_md,
			line_height = line(font_md),
			weight = theme.weight.semibold,
			color = theme.color.header_primary,
		},
		icon_button {
			name = "close",
			tooltip = "Close results",
			on_click = actions.close_search,
		},
	}
end

local function notice()
	return ui.show(function()
		return state.search.panel:get().notice ~= ""
	end, ui.text {
		function()
			return state.search.panel:get().notice
		end,
		literal = true,
		wrap = true,
		shrink = 0,
		px = theme.size.space_16,
		pt = theme.size.space_8,
		text_size = theme.size.font_sm,
		line_height = line(font_sm),
		color = theme.color.text_muted,
	})
end

local function hit(row: state.SearchHit)
	return ui.column {
		gap = theme.size.space_4,
		mx = theme.size.space_8,
		mb = theme.size.space_8,
		p = theme.size.space_8,
		rounded = theme.size.radius_md,
		bg = theme.color.background_primary,
		hover = { bg = theme.color.modifier_selected },
		cursor = "pointer",
		on_click = function()
			actions.open_result(row.id)
		end,
		context_menu = function(event: any)
			actions.result_menu(row.id, event.x, event.y)
			return nil
		end,
		on_visible = function(visible: boolean)
			if visible and is_last(row.id) and state.search.panel:get().more then
				actions.more_results()
			end
		end,
		ui.row {
			items = "center",
			gap = theme.size.space_8,
			avatar { src = row.avatar, size = AVATAR },
			ui.text {
				row.author,
				literal = true,
				shrink = 0,
				text_size = theme.size.font_sm,
				line_height = line(font_sm),
				weight = theme.weight.semibold,
				color = theme.color.header_primary,
			},
			ui.text {
				row.time,
				literal = true,
				truncate = true,
				min_w = 0,
				text_size = theme.size.font_tiny,
				line_height = line(font_tiny),
				color = theme.color.text_muted,
			},
		},
		ui.text {
			row.text,
			literal = true,
			wrap = true,
			lines = SNIPPET_LINES,
			text_size = theme.size.font_sm,
			line_height = line(font_sm),
			color = theme.color.text_normal,
		},
		if row.place ~= ""
			then ui.text {
				row.place,
				literal = true,
				truncate = true,
				text_size = theme.size.font_tiny,
				line_height = line(font_tiny),
				color = theme.color.text_muted,
			}
			else nil,
	}
end

local function results()
	return ui.scroll {
		grow = 1,
		min_h = 0,
		ui.column {
			w_full = true,
			pt = theme.size.space_8,
			ui.each(state.search.hits, function(row: state.SearchHit)
				return hit(row)
			end, { key = by_key }),
		},
	}
end

return function()
	return ui.column {
		w = WIDTH,
		h_full = true,
		shrink = 0,
		header(),
		chips(),
		notice(),
		results(),
	}
end
