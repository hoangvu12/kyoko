local theme = require("../theme")
local icon = require("@components/icon")
local motion = require("@components/motion")
local kit = require("./kit")
local search = require("./search")

local CLEAR = "#00000000"
local OPEN_TRAVEL = 80
local OPEN_DURATION = 250
local OPEN_EASE = "out(3)"

export type Page = {
	key: string,
	title: string,
	icon: string,
	build: (() -> any)?,
	headings: { kit.Heading }?,
	keywords: (string | () -> string)?,
	greyed: boolean?,
}
export type Section = { key: string, title: string?, pages: { Page } }
export type Extra = { key: string, title: string, icon: string, words: string, danger: boolean?, on_click: any }

export type Spec = {
	prefix: string,
	label: string,
	page_key: () -> string,
	sections: () -> { Section },
	shape: () -> string,
	find: (string) -> Page?,
	build: (Page) -> any,
	select: (string) -> (),
	close: () -> (),
	card: () -> any,
	extras: { Extra }?,
	footer: (() -> any)?,
	overlay: ((string) -> any)?,
}

local M = {}

local function n(token: string): number
	return theme.number(token)()
end

M.nav_item = ui.component(function(props: {
	id: string,
	title: string,
	icon: string,
	greyed: boolean?,
	danger: boolean?,
	selected: () -> boolean,
	on_click: any,
	on_focus: () -> (),
	on_mount: (any) -> (),
})
	local hovered = ui.use_state(false)
	local function ink(): string
		if props.danger then
			return theme.color.red_2
		end
		return if props.selected() or hovered:get() then theme.color.text_3 else theme.color.text_4
	end
	return ui.button {
		id = props.id,
		label = props.title,
		w_full = true,
		min_h = theme.size.settings_item_height,
		p = theme.size.space_10,
		gap = theme.size.space_10,
		items = "center",
		rounded = theme.size.radius_sm,
		border = 0,
		focus = { border = theme.size.border_thickness, border_color = theme.color.accent_2 },
		cursor = "pointer",
		opacity = if props.greyed then theme.size.disabled_opacity_control else 1,
		x = motion.slide(hovered, 0, theme.size.list_item_hover_shift),
		bg = function()
			if props.selected() then
				return theme.color.bg_2
			end
			return if hovered:get() then theme.color.bg_3 else CLEAR
		end,
		on_hover = function(inside: boolean)
			hovered:set(inside)
		end,
		on_focus = props.on_focus,
		on_mount = props.on_mount,
		on_click = props.on_click,
		icon { name = props.icon, size = theme.size.icon_size_md :: any, color = ink },
		ui.text {
			props.title,
			literal = true,
			min_w = 0,
			wrap = true,
			text_size = theme.size.font_size_md,
			line_height = theme.size.line_height_label,
			weight = theme.weight.medium,
			color = ink,
		},
	}
end)

function M.separator()
	return ui.div {
		w_full = true,
		h = theme.size.border_thickness,
		shrink = 0,
		my = theme.size.space_6,
		bg = theme.color.border,
	}
end

function M.group_header(title: string)
	return ui.div {
		w_full = true,
		mt = theme.size.space_14,
		mb = theme.size.space_4,
		px = theme.size.space_8,
		kit.text(title, theme.size.font_size_sm, theme.size.line_height_sm, theme.color.text_5, {
			weight = theme.weight.medium,
		}),
	}
end

function M.card(picture: any, title: any, subtitle: any)
	return ui.row {
		w_full = true,
		shrink = 0,
		items = "center",
		pt = theme.size.settings_profile_top,
		pr = theme.size.space_16,
		pl = theme.size.space_24,
		gap = theme.size.space_16,
		picture,
		ui.column {
			grow = 1,
			min_w = 0,
			kit.text(title, theme.size.font_size_md, theme.size.line_height_label, theme.color.text_2, {
				weight = theme.weight.semibold,
				truncate = true,
			}),
			subtitle,
		},
	}
end

M.close_button = ui.component(function(props: { id: string, label: string, on_click: () -> () })
	local hovered = ui.use_state(false)
	return ui.button {
		id = props.id,
		tooltip = "Close",
		label = props.label,
		w = theme.size.button_size,
		h = theme.size.button_size,
		shrink = 0,
		p = 0,
		border = 0,
		focus = { border = theme.size.border_thickness, border_color = theme.color.accent_2 },
		rounded = theme.size.radius_sm,
		justify = "center",
		items = "center",
		cursor = "pointer",
		bg = function()
			return if hovered:get() then theme.color.hover else CLEAR
		end,
		on_hover = function(inside: boolean)
			hovered:set(inside)
		end,
		on_click = props.on_click,
		icon {
			name = "close",
			size = theme.size.icon_size_md :: any,
			color = function()
				return if hovered:get() then theme.color.text_3 else theme.color.text_4
			end,
		},
	}
end)

function M.frame(spec: Spec)
	local prefix = spec.prefix
	local open = ui.state(prefix .. ".open", false)
	local offset = ui.state(prefix .. ".offset", 0)
	local nav_offset = ui.state(prefix .. ".nav_offset", 0)
	local current_heading = ui.state(prefix .. ".heading", "")
	local nav_view = nil :: any
	local elements: { [string]: any } = {}
	local focused = ""
	local extras = spec.extras or {}

	local function page_key(): string
		return spec.page_key()
	end

	local function query(): string
		return search.query:get()
	end

	local function shown(page: Page): boolean
		return search.page_matches(page :: any, query())
	end

	local function extra_shown(extra: Extra): () -> boolean
		return function()
			return not search.active() or search.row_matches({ title = extra.words }, query())
		end
	end

	local function section_shown(section: Section): boolean
		for _, page in section.pages do
			if shown(page) then
				return true
			end
		end
		return false
	end

	local function earlier_shown(sections: { Section }, index: number): boolean
		for earlier = 1, index - 1 do
			if section_shown(sections[earlier]) then
				return true
			end
		end
		return false
	end

	local function any_extra_shown(): boolean
		for _, extra in extras do
			if extra_shown(extra)() then
				return true
			end
		end
		return false
	end

	local function nothing_matches(): boolean
		if not search.active() or any_extra_shown() then
			return false
		end
		for _, section in spec.sections() do
			if section_shown(section) then
				return false
			end
		end
		return true
	end

	local function order(): { string }
		local keys = {}
		for _, section in spec.sections() do
			for _, page in section.pages do
				if shown(page) then
					table.insert(keys, page.key)
				end
			end
		end
		for _, extra in extras do
			if extra_shown(extra)() then
				table.insert(keys, extra.key)
			end
		end
		return keys
	end

	local function step(direction: number): boolean
		local keys = order()
		if #keys == 0 then
			return false
		end
		local index = table.find(keys, focused) or (if direction > 0 then 0 else #keys + 1)
		local target = keys[math.clamp(index + direction, 1, #keys)]
		local element = elements[target]
		if element then
			element:focus()
		end
		return true
	end

	local function arrows(event: any): boolean
		if event.key == "ArrowDown" or event.key == "Down" then
			return step(1)
		elseif event.key == "ArrowUp" or event.key == "Up" then
			return step(-1)
		end
		return false
	end

	local function heading_offset(key: string): number?
		local page = spec.find(page_key())
		local first = page and page.headings and page.headings[1]
		local top = first and kit.anchors[first.key]
		local anchor = kit.anchors[key]
		if not top or not anchor or top.y == nil or anchor.y == nil then
			return nil
		end
		return anchor.y - top.y
	end

	local function heading_in_view(): string
		local page = spec.find(page_key())
		local scrolled = offset:get()
		local found = ""
		for _, heading in (page and page.headings) or {} do
			local at = heading_offset(heading.key)
			if found == "" or (at and at <= scrolled + n(theme.size.settings_page_padding_top)) then
				found = heading.key
			end
		end
		return found
	end

	local function current(): string
		local picked = current_heading:get()
		if picked ~= "" then
			return picked
		end
		return heading_in_view()
	end

	local function scroll_to(key: string)
		current_heading:set(key)
		local at = heading_offset(key)
		if at then
			offset:set(at)
		end
	end

	local function item(key: string, props: { [string]: any })
		props.id = prefix .. "_nav_" .. key
		props.on_focus = function()
			focused = key
		end
		props.on_mount = function(element: any)
			elements[key] = element
		end
		return M.nav_item(props)
	end

	local function sub_item(heading: any)
		local function now(): boolean
			return current() == heading.key
		end
		return ui.button {
			id = prefix .. "_sub_" .. heading.key,
			relative = true,
			w_full = true,
			h = theme.size.settings_item_height,
			pl = theme.size.space_20,
			p = 0,
			border = 0,
			focus = { border = 0 },
			bg = CLEAR,
			items = "center",
			cursor = "pointer",
			on_click = function()
				scroll_to(heading.key)
			end,
			ui.show(now, ui.div {
				absolute = true,
				left = -n(theme.size.border_thickness),
				top = (n(theme.size.settings_item_height) - n(theme.size.line_height_sm)) / 2,
				w = theme.size.space_2,
				h = theme.size.line_height_sm,
				rounded = theme.size.space_2,
				bg = theme.color.text_3,
			}),
			ui.text {
				heading.title or "",
				literal = true,
				truncate = true,
				min_w = 0,
				text_size = theme.size.font_size_md,
				line_height = theme.size.line_height_label,
				weight = theme.weight.medium,
				color = function()
					return if now() then theme.color.text_3 else theme.color.text_5
				end,
			},
		}
	end

	local function sub_sections(page: Page)
		local headings = {}
		for _, heading in page.headings or {} do
			if heading.title then
				table.insert(headings, sub_item(heading))
			end
		end
		if #headings < 2 then
			return nil
		end
		return ui.show(function()
			return page_key() == page.key and not search.active()
		end, ui.column {
			w_full = true,
			ml = theme.size.space_20,
			border_sides = { 0, 0, 0, 1 },
			border_color = theme.color.border_light,
			table.unpack(headings),
		})
	end

	local function nav_group(page: Page)
		return ui.show(function()
			return shown(page)
		end, ui.column {
			w_full = true,
			item(page.key, {
				title = page.title,
				icon = page.icon,
				greyed = page.greyed,
				selected = function()
					return page_key() == page.key
				end,
				on_click = function()
					spec.select(page.key)
				end,
			}),
			sub_sections(page),
		})
	end

	local function nav_items(): { any }
		local items = {}
		local sections = spec.sections()
		for index, section in sections do
			if index > 1 then
				table.insert(
					items,
					ui.show(function()
						return section_shown(section) and earlier_shown(sections, index)
					end, M.separator())
				)
			end
			if section.title and section.title ~= "" then
				local title = section.title
				table.insert(
					items,
					ui.show(function()
						return section_shown(section)
					end, M.group_header(title))
				)
			end
			for _, page in section.pages do
				table.insert(items, nav_group(page))
			end
		end
		for _, extra in extras do
			local visible = extra_shown(extra)
			table.insert(items, ui.show(visible, M.separator()))
			table.insert(
				items,
				ui.show(
					visible,
					item(extra.key, {
						title = extra.title,
						icon = extra.icon,
						danger = extra.danger,
						selected = function()
							return false
						end,
						on_click = extra.on_click,
					})
				)
			)
		end
		table.insert(
			items,
			ui.show(
				nothing_matches,
				ui.div {
					id = prefix .. "_no_match",
					w_full = true,
					px = theme.size.space_8,
					py = theme.size.space_8,
					kit.text(function()
						return "No settings match '" .. query() .. "'"
					end, theme.size.font_size_sm, theme.size.line_height_sm, theme.color.text_4, { wrap = true }),
				}
			)
		)
		if spec.footer then
			table.insert(items, spec.footer())
		end
		return items
	end

	local function nav_list()
		return ui.scroll {
			w_full = true,
			grow = 1,
			min_h = 0,
			px = theme.size.space_16,
			offset = nav_offset,
			on_key = arrows,
			on_mount = function(element: any)
				nav_view = element
			end,
			ui.presence(spec.shape, function()
				table.clear(elements)
				return ui.column {
					w_full = true,
					gap = theme.size.space_4,
					pt = theme.size.space_14,
					pb = theme.size.space_16,
					table.unpack(nav_items()),
				}
			end),
		}
	end

	local function search_field()
		return ui.row {
			shrink = 0,
			h = theme.size.settings_search_height,
			mt = theme.size.space_24,
			mx = theme.size.space_16,
			px = theme.size.space_12,
			gap = theme.size.space_10,
			items = "center",
			rounded = theme.size.radius_sm,
			bg = theme.color.bg_3,
			icon { name = "search", size = theme.size.icon_size_sm :: any, color = theme.color.text_4 },
			ui.input {
				id = prefix .. "_search",
				value = search.query,
				label = "Search settings",
				placeholder = "Search",
				placeholder_color = theme.color.text_5,
				grow = 1,
				min_w = 0,
				p = 0,
				bg = CLEAR,
				border = 0,
				focus = { border = 0 },
				text_size = theme.size.font_size_md,
				color = theme.color.text_3,
				on_key = function(event: any): boolean?
					if event.key == "ArrowDown" or event.key == "Down" then
						focused = ""
						return step(1)
					end
					return false
				end,
			},
			ui.show(search.active, ui.button {
				id = prefix .. "_search_clear",
				tooltip = "Clear search",
				p = 0,
				border = 0,
				focus = { border = 0 },
				bg = CLEAR,
				cursor = "pointer",
				on_click = function()
					search.query:set("")
				end,
				icon { name = "close", size = theme.size.icon_size_sm :: any, color = theme.color.text_4 },
			}),
		}
	end

	local function nav()
		return ui.column {
			w = theme.size.settings_dialog_nav_width,
			h_full = true,
			shrink = 0,
			border_sides = { 0, 1, 0, 0 },
			border_color = theme.color.border,
			spec.card(),
			search_field(),
			nav_list(),
		}
	end

	local function content()
		return ui.column {
			grow = 1,
			min_w = 0,
			h_full = true,
			ui.row {
				w_full = true,
				h = theme.size.settings_topbar_box,
				shrink = 0,
				items = "center",
				justify = "between",
				px = theme.size.space_16,
				border_sides = { 0, 0, 1, 0 },
				border_color = theme.color.border,
				kit.text(function()
					local page = spec.find(page_key())
					return if page then page.title else ""
				end, theme.size.font_size_md, theme.size.line_height_label, theme.color.text_3, {
					weight = theme.weight.semibold,
				}),
				M.close_button {
					id = prefix .. "_close",
					label = "Close " .. string.lower(spec.label),
					on_click = function()
						open:set(false)
					end,
				},
			},
			ui.div {
				relative = true,
				w_full = true,
				grow = 1,
				min_h = 0,
				ui.scroll {
					id = prefix .. "_page",
					offset = offset,
					w_full = true,
					h_full = true,
					px = theme.size.space_24,
					on_wheel = function()
						current_heading:set("")
					end,
					ui.row {
						w_full = true,
						justify = "center",
						ui.column {
							grow = 1,
							basis = 0,
							min_w = 0,
							max_w = function()
								return n(theme.size.settings_column_width)
							end,
							pt = theme.size.settings_page_padding_top,
							pb = theme.size.space_56,
							ui.presence(function()
								local key = page_key()
								return if key ~= "" then key else nil
							end, function(key: string)
								local page = spec.find(key)
								return if page then spec.build(page) else ui.div {}
							end),
						},
					},
				},
				if spec.overlay
					then ui.presence(function()
						local key = page_key()
						return if key ~= "" then key else nil
					end, function(key: string)
						return spec.overlay(key) or ui.div {}
					end)
					else nil,
			},
		}
	end

	local function reveal(key: string)
		local sections = spec.sections()
		local first_page = sections[1] and sections[1].pages[1]
		local item_view = elements[key]
		local first = first_page and elements[first_page.key]
		if not item_view or not first or not nav_view or item_view.y == nil or first.y == nil then
			return
		end
		local at = item_view.y - first.y
		local view = nav_view.height or 0
		local shown_from = nav_offset:get()
		if at < shown_from or at + n(theme.size.settings_item_height) > shown_from + view then
			nav_offset:set(math.max(0, at - view / 3))
		end
	end

	local function sync()
		ui.effect(function()
			local key = page_key()
			offset:set(0)
			current_heading:set("")
			if key ~= "" then
				open:set(true)
				ui.after(32, function()
					reveal(key)
				end)
			else
				open:set(false)
				search.query:set("")
			end
		end)
		ui.effect(function()
			if not open:get() and page_key() ~= "" then
				spec.close()
			end
		end)
	end

	local function dialog(window: any)
		local frame = nil :: any
		ui.effect(function()
			if open:get() and frame and not motion.reduced() then
				anime.animate(frame, {
					y = { -OPEN_TRAVEL, 0 },
					opacity = { 0, 1 },
					duration = OPEN_DURATION,
					ease = OPEN_EASE,
				})
			end
		end)
		local function side(): number
			local width = window.width or 0
			local spare = (width - n(theme.size.settings_dialog_nav_width) - n(theme.size.settings_column_width)) / 2
				- n(theme.size.space_40)
			return math.clamp(spare, n(theme.size.gap), n(theme.size.settings_dialog_inset))
		end
		return ui.modal {
			open = open,
			label = spec.label,
			p = 0,
			bg = CLEAR,
			border = 0,
			rounded = 0,
			w = function()
				return math.max(0, math.min(n(theme.size.settings_dialog_max_width), (window.width or 0) - side() * 2))
			end,
			h = function()
				return math.max(0, (window.height or 0) - n(theme.size.settings_dialog_inset) * 2)
			end,
			ui.row {
				id = prefix .. "_dialog",
				w_full = true,
				h_full = true,
				overflow = "hidden",
				bg = theme.color.bg_4,
				border = theme.size.border_thickness,
				border_color = theme.color.border,
				rounded = theme.size.radius_lg,
				color = theme.color.text_3,
				on_mount = function(element: any)
					frame = element
				end,
				nav(),
				content(),
			},
		}
	end

	local window = ui.ref()
	sync()
	return ui.div {
		absolute = true,
		left = 0,
		top = 0,
		right = 0,
		bottom = 0,
		pointer_events = "none",
		invisible = function()
			return not open:get()
		end,
		ref = window,
		dialog(window),
	}
end

return M
