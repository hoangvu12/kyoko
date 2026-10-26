local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local avatar = require("@components/avatar")
local icon = require("@components/icon")
local icon_button = require("@components/icon_button")

local TABS = {
	{ key = "messages", label = "Messages" },
	{ key = "media", label = "Media" },
	{ key = "links", label = "Links" },
	{ key = "files", label = "Files" },
	{ key = "pins", label = "Pins" },
}
local SNIPPET_LINES = 4
local PAGE_BUTTONS = 5

local icon_sm = theme.number(theme.size.icon_size_sm)
local icon_md = theme.number(theme.size.icon_size_md)
local avatar_md = theme.number(theme.size.avatar_size_md)
local button = theme.number(theme.size.button_size)

local function panel(): state.SearchPanel
	return state.search.panel:get()
end

local function words(value: any, size: string, extra: { [string]: any }?)
	local node = {
		value,
		literal = true,
		text_size = (theme.size :: any)["font_size_" .. size],
		line_height = if size == "md" then theme.size.line_height_label else (theme.size :: any)["line_height_" .. size],
	}
	for key, item in extra or {} do
		node[key] = item
	end
	return ui.text(node)
end

local function by_key(row: state.SearchHit): string
	return row.key
end

local function chip(label: string)
	return ui.row {
		h = theme.size.space_20,
		shrink = 0,
		items = "center",
		px = theme.size.space_6,
		rounded = theme.size.radius_xs,
		bg = theme.color.bg_3,
		border = theme.size.border_thickness,
		border_color = theme.color.border,
		words(label, "xs", { weight = theme.weight.medium, color = theme.color.text_3 }),
	}
end

local function sort_button()
	return ui.button {
		id = "search_sort",
		p = 0,
		gap = theme.size.space_4,
		border = 0,
		focus = { border = 0 },
		bg = "#00000000",
		items = "center",
		cursor = "pointer",
		color = theme.color.text_3,
		hover = { color = theme.color.text_2 },
		tooltip = "Sort results",
		on_click = function()
			actions.search_sort(not state.search.panel:peek().oldest)
		end,
		words(function()
			return if panel().oldest then "Oldest" else "Newest"
		end, "sm", { weight = theme.weight.medium }),
		icon { name = "chevron_down", size = icon_sm() },
	}
end

local function tab(item: { key: string, label: string })
	local function chosen(): boolean
		return panel().tab == item.key
	end
	return ui.button {
		item.label,
		id = "search_tab_" .. item.key,
		h = theme.size.search_tab_height,
		px = theme.size.space_10,
		py = 0,
		shrink = 0,
		border = 0,
		focus = { border = 0 },
		rounded = theme.size.radius_sm,
		cursor = "pointer",
		bg = function()
			return if chosen() then theme.color.active else "#00000000"
		end,
		hover = { bg = theme.color.hover },
		text_size = theme.size.font_size_sm,
		line_height = theme.size.line_height_sm,
		weight = theme.weight.medium,
		color = function()
			return if chosen() then theme.color.text_2 else theme.color.text_4
		end,
		on_click = function()
			actions.search_tab(item.key)
		end,
	}
end

local function header()
	local tabs = {}
	for _, item in TABS do
		table.insert(tabs, tab(item))
	end
	return ui.column {
		w_full = true,
		shrink = 0,
		gap = theme.size.space_12,
		px = theme.size.space_16,
		py = theme.size.space_12,
		border_sides = { 0, 0, 1, 0 },
		border_color = theme.color.border,
		ui.row {
			w_full = true,
			items = "center",
			gap = theme.size.space_8,
			words(function()
				return panel().title
			end, "md", { grow = 1, min_w = 0, truncate = true, weight = theme.weight.semibold, color = theme.color.text_2 }),
			sort_button(),
			icon_button {
				name = "close",
				hit_size = button(),
				icon_size = icon_md(),
				tooltip = "Close results",
				on_click = actions.close_search,
			},
		},
		ui.row { w_full = true, gap = theme.size.space_4, table.unpack(tabs) },
		ui.show(function()
			return #state.search.chips:get() > 0
		end, ui.row {
			w_full = true,
			flex_wrap = true,
			gap = theme.size.space_4,
			ui.each(state.search.chips, function(label: string)
				return chip(label)
			end, { flex_wrap = true, gap = theme.size.space_4 }),
		}),
	}
end

local function notice()
	return ui.show(function()
		return panel().notice ~= ""
	end, words(function()
		return panel().notice
	end, "sm", { wrap = true, shrink = 0, px = theme.size.space_16, pt = theme.size.space_8, color = theme.color.text_4 }))
end

local function snippet(row: state.SearchHit)
	local runs = {}
	for _, run in row.runs or { { text = row.text, mark = false } } do
		if run.mark then
			table.insert(
				runs,
				ui.span {
					run.text,
					literal = true,
					bg = theme.color.accent_3,
					color = theme.color.text_0,
					radius = theme.number(theme.size.radius_xs)(),
					padding = theme.number(theme.size.space_2)(),
				}
			)
		else
			table.insert(runs, ui.span { run.text, literal = true })
		end
	end
	local props: { [string]: any } = {
		w_full = true,
		wrap = true,
		lines = SNIPPET_LINES,
		text_size = theme.size.font_size_md,
		line_height = theme.size.line_height_md,
		color = theme.color.text_3,
	}
	for _, run in runs do
		table.insert(props, run)
	end
	return ui.rich(props)
end

local hit = ui.component(function(props: { row: state.SearchHit })
	local row = props.row
	local hovered = ui.use_state(false)
	return ui.column {
		w_full = true,
		gap = theme.size.space_8,
		ui.show(row.group ~= "", ui.row {
			w_full = true,
			items = "center",
			gap = theme.size.space_6,
			px = theme.size.space_4,
			pt = theme.size.space_8,
			icon { name = row.group_icon, size = icon_sm(), color = theme.color.text_4 },
			words(row.group, "sm", { min_w = 0, truncate = true, weight = theme.weight.semibold, color = theme.color.text_3 }),
		}),
		ui.column {
			id = "search_hit_" .. row.id,
			relative = true,
			w_full = true,
			pl = theme.size.message_card_inset,
			pr = theme.size.space_12,
			py = theme.size.space_8,
			rounded = theme.size.radius_sm,
			bg = theme.color.bg_3,
			border = theme.size.border_thickness,
			border_color = function()
				return if hovered:get() then theme.color.border_light else theme.color.border
			end,
			cursor = "pointer",
			on_hover = function(inside: boolean)
				hovered:set(inside)
			end,
			on_click = function()
				actions.open_result(row.id)
			end,
			context_menu = function(event: any)
				actions.result_menu(row.id, event.x, event.y)
				return nil
			end,
			ui.div {
				absolute = true,
				left = theme.number(theme.size.space_12),
				top = theme.number(theme.size.space_8),
				avatar { src = row.avatar, size = avatar_md() },
			},
			ui.row {
				w_full = true,
				items = "end",
				gap = theme.size.space_6,
				words(row.author, "md", {
					shrink = 0,
					line_height = theme.size.line_height_md,
					weight = theme.weight.medium,
					color = theme.color.text_2,
				}),
				words(row.time, "xs", {
					min_w = 0,
					truncate = true,
					line_height = theme.size.line_height_md,
					weight = theme.weight.medium,
					color = theme.color.text_5,
				}),
			},
			snippet(row),
			ui.show(function()
				return hovered:get()
			end, ui.button {
				"Jump",
				absolute = true,
				top = theme.number(theme.size.space_8),
				right = theme.number(theme.size.space_8),
				h = theme.size.space_24,
				px = theme.size.space_6,
				py = 0,
				rounded = theme.size.radius_sm,
				bg = theme.color.bg_2,
				border = theme.size.border_thickness,
				border_color = theme.color.border_light,
				focus = { border = 1 },
				cursor = "pointer",
				text_size = theme.size.font_size_xs,
				line_height = theme.size.line_height_label,
				weight = theme.weight.medium,
				color = theme.color.text_3,
				hover = { bg = theme.color.bg_1 },
				on_click = function()
					actions.open_result(row.id)
				end,
			}),
		},
	}
end)

local function results()
	return ui.scroll {
		grow = 1,
		min_h = 0,
		w_full = true,
		ui.column {
			w_full = true,
			gap = theme.size.space_8,
			px = theme.size.space_16,
			py = theme.size.space_8,
			ui.each(state.search.hits, function(row: state.SearchHit)
				return hit { row = row }
			end, { key = by_key, gap = theme.size.space_8 }),
		},
	}
end

local function page_button(label: any, id: string, chosen: () -> boolean, target: () -> number, shown: () -> boolean)
	return ui.show(shown, ui.button {
		id = id,
		w = theme.size.button_size,
		h = theme.size.button_size,
		p = 0,
		border = 0,
		focus = { border = 0 },
		rounded = theme.size.radius_sm,
		justify = "center",
		items = "center",
		cursor = "pointer",
		bg = function()
			return if chosen() then theme.color.accent_3 else "#00000000"
		end,
		hover = { bg = theme.color.hover },
		color = function()
			return if chosen() then theme.color.text_0 else theme.color.text_3
		end,
		on_click = function()
			actions.search_page(target())
		end,
		label,
	})
end

local function first_shown(): number
	local current = panel()
	local first = math.max(1, current.page - math.floor(PAGE_BUTTONS / 2))
	return math.max(1, math.min(first, current.pages - PAGE_BUTTONS + 1))
end

local function pagination()
	local buttons = {}
	table.insert(
		buttons,
		page_button(icon { name = "chevron_left", size = icon_md() }, "search_page_back", function()
			return false
		end, function()
			return math.max(1, panel().page - 1)
		end, function()
			return true
		end)
	)
	for slot = 1, PAGE_BUTTONS do
		local function number(): number
			return first_shown() + slot - 1
		end
		table.insert(
			buttons,
			page_button(
				words(function()
					return tostring(number())
				end, "sm", { weight = theme.weight.semibold }),
				"search_page_" .. slot,
				function()
					return number() == panel().page
				end,
				number,
				function()
					return number() <= panel().pages
				end
			)
		)
	end
	table.insert(
		buttons,
		page_button(icon { name = "chevron_right", size = icon_md() }, "search_page_next", function()
			return false
		end, function()
			return math.min(panel().pages, panel().page + 1)
		end, function()
			return true
		end)
	)
	return ui.show(function()
		return panel().pages > 1
	end, ui.row {
		w_full = true,
		shrink = 0,
		justify = "center",
		gap = theme.size.space_4,
		p = theme.size.space_12,
		border_sides = { 1, 0, 0, 0 },
		border_color = theme.color.border,
		table.unpack(buttons),
	})
end

return function()
	return ui.column {
		w = theme.size.search_results_width,
		h_full = true,
		shrink = 0,
		header(),
		notice(),
		results(),
		pagination(),
	}
end
