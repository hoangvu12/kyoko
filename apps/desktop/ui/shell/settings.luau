local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")
local avatar = require("@components/avatar")
local motion = require("@components/motion")
local pages = require("@settings/pages")
local kit = require("@settings/kit")
local search = require("@settings/search")

local CLEAR = "#00000000"
local OPEN_TRAVEL = 80
local OPEN_DURATION = 250
local OPEN_EASE = "out(3)"
local LOG_OUT = "log-out"
local LOG_OUT_WORDS = "log out logout sign out"

local open = ui.state("open", false)
local offset = ui.state("offset", 0)
local nav_offset = ui.state("nav_offset", 0)
local nav_view = nil :: any
local current_heading = ui.state("current_heading", "")
local elements: { [string]: any } = {}
local focused = ""

local function n(token: string): number
	return theme.number(token)()
end

local function page_key(): string
	return state.settings.page:get()
end

local function query(): string
	return search.query:get()
end

local function shown(page: pages.Page): boolean
	return search.page_matches(page :: any, query())
end

local function log_out_shown(): boolean
	return not search.active() or search.row_matches({ title = LOG_OUT_WORDS }, query())
end

local function section_shown(section: pages.Section): boolean
	for _, page in section.pages do
		if shown(page) then
			return true
		end
	end
	return false
end

local function earlier_shown(index: number): boolean
	for earlier = 1, index - 1 do
		if section_shown(pages.sections[earlier]) then
			return true
		end
	end
	return false
end

local function nothing_matches(): boolean
	if not search.active() or log_out_shown() then
		return false
	end
	for _, section in pages.sections do
		if section_shown(section) then
			return false
		end
	end
	return true
end

local function order(): { string }
	local keys = {}
	for _, section in pages.sections do
		for _, page in section.pages do
			if shown(page) then
				table.insert(keys, page.key)
			end
		end
	end
	if log_out_shown() then
		table.insert(keys, LOG_OUT)
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
	local page = pages.find(page_key())
	local first = page and page.headings and page.headings[1]
	local top = first and kit.anchors[first.key]
	local anchor = kit.anchors[key]
	if not top or not anchor or top.y == nil or anchor.y == nil then
		return nil
	end
	return anchor.y - top.y
end

local function heading_in_view(): string
	local page = pages.find(page_key())
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

local nav_item = ui.component(function(props: {
	key: string,
	title: string,
	icon: string,
	greyed: boolean?,
	danger: boolean?,
	selected: () -> boolean,
	on_click: any,
})
	local hovered = ui.use_state(false)
	local function ink(): string
		if props.danger then
			return theme.color.red_2
		end
		return if props.selected() or hovered:get() then theme.color.text_3 else theme.color.text_4
	end
	return ui.button {
		id = "settings_nav_" .. props.key,
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
		on_focus = function()
			focused = props.key
		end,
		on_mount = function(element: any)
			elements[props.key] = element
		end,
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

local function scroll_to(key: string)
	current_heading:set(key)
	local at = heading_offset(key)
	if at then
		offset:set(at)
	end
end

local function sub_item(heading: any)
	local function now(): boolean
		return current() == heading.key
	end
	return ui.button {
		id = "settings_sub_" .. heading.key,
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

local function sub_sections(page: pages.Page)
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

local function nav_group(page: pages.Page)
	return ui.show(function()
		return shown(page)
	end, ui.column {
		w_full = true,
		nav_item {
			key = page.key,
			title = page.title,
			icon = page.icon,
			greyed = page.greyed,
			selected = function()
				return page_key() == page.key
			end,
			on_click = function()
				actions.open_settings(page.key)
			end,
		},
		sub_sections(page),
	})
end

local function separator()
	return ui.div {
		w_full = true,
		h = theme.size.border_thickness,
		shrink = 0,
		my = theme.size.space_6,
		bg = theme.color.border,
	}
end

local function group_header(title: string)
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

local function build_info()
	return ui.column {
		w_full = true,
		mt = theme.size.space_12,
		px = theme.size.space_8,
		kit.text(function()
			return "Kyoko " .. state.settings.about:get().version
		end, theme.size.font_size_xs, theme.size.line_height_xs, theme.color.text_5),
		kit.text(function()
			return state.settings.about:get().platform or ""
		end, theme.size.font_size_xs, theme.size.line_height_xs, theme.color.text_5),
		ui.button {
			id = "settings_data_folder",
			p = 0,
			border = 0,
			focus = { border = 0 },
			bg = CLEAR,
			cursor = "pointer",
			on_click = function()
				actions.settings_action("open_data_folder")
			end,
			kit.text("Open data folder", theme.size.font_size_xs, theme.size.line_height_xs, theme.color.accent_1),
		},
	}
end

local function nav_list()
	local items = {}
	for index, section in pages.sections do
		if index > 1 then
			table.insert(
				items,
				ui.show(function()
					return section_shown(section) and earlier_shown(index)
				end, separator())
			)
		end
		if section.title then
			local title = section.title
			table.insert(
				items,
				ui.show(function()
					return section_shown(section)
				end, group_header(title))
			)
		end
		for _, page in section.pages do
			table.insert(items, nav_group(page))
		end
	end
	table.insert(items, ui.show(log_out_shown, separator()))
	table.insert(
		items,
		ui.show(
			log_out_shown,
			nav_item {
				key = LOG_OUT,
				title = "Log Out",
				icon = "log_out",
				danger = true,
				selected = function()
					return false
				end,
				on_click = actions.sign_out(),
			}
		)
	)
	table.insert(
		items,
		ui.show(
			nothing_matches,
			ui.div {
				id = "settings_no_match",
				w_full = true,
				px = theme.size.space_8,
				py = theme.size.space_8,
				kit.text(function()
					return "No settings match '" .. query() .. "'"
				end, theme.size.font_size_sm, theme.size.line_height_sm, theme.color.text_4, { wrap = true }),
			}
		)
	)
	table.insert(items, build_info())
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
		ui.column {
			w_full = true,
			gap = theme.size.space_4,
			pt = theme.size.space_14,
			pb = theme.size.space_16,
			table.unpack(items),
		},
	}
end

local function account_field(name: string): () -> string
	return function()
		local found = state.settings.account:get()
		return if found then (found :: any)[name] or "" else ""
	end
end

local function profile_card()
	return ui.row {
		w_full = true,
		shrink = 0,
		items = "center",
		gap = theme.size.space_20,
		pt = theme.size.settings_profile_top,
		pr = theme.size.space_16,
		pl = theme.size.space_24,
		ui.presence(function()
			return account_field("avatar")() .. "|" .. account_field("status")()
		end, function()
			return ui.div {
				w = theme.size.avatar_size_md,
				h = theme.size.avatar_size_md,
				shrink = 0,
				rounded = theme.size.avatar_size_md,
				bg = theme.color.bg_2,
				avatar {
					src = account_field("avatar")(),
					size = n(theme.size.avatar_size_md),
					status = account_field("status")(),
					cutout = theme.color.bg_4,
				},
			}
		end),
		ui.column {
			grow = 1,
			min_w = 0,
			kit.text(account_field("name"), theme.size.font_size_md, theme.size.line_height_label, theme.color.text_2, {
				weight = theme.weight.semibold,
				truncate = true,
			}),
			ui.row {
				id = "settings_edit_profiles",
				items = "center",
				gap = theme.size.space_4,
				cursor = "pointer",
				on_click = function()
					actions.open_settings("profiles")
				end,
				kit.text("Edit Profiles", theme.size.font_size_sm, theme.size.line_height_sm, theme.color.text_4, {
					weight = theme.weight.medium,
				}),
				icon { name = "edit", size = theme.size.icon_size_xs :: any, color = theme.color.text_4 },
			},
		},
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
			id = "settings_search",
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
			id = "settings_search_clear",
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
		profile_card(),
		search_field(),
		nav_list(),
	}
end

local close_button = ui.component(function()
	local hovered = ui.use_state(false)
	return ui.button {
		id = "settings_close",
		tooltip = "Close",
		label = "Close settings",
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
		on_click = function()
			open:set(false)
		end,
		icon {
			name = "close",
			size = theme.size.icon_size_md :: any,
			color = function()
				return if hovered:get() then theme.color.text_3 else theme.color.text_4
			end,
		},
	}
end)

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
				local page = pages.find(page_key())
				return if page then page.title else ""
			end, theme.size.font_size_md, theme.size.line_height_label, theme.color.text_3, {
				weight = theme.weight.semibold,
			}),
			close_button {},
		},
		ui.scroll {
			id = "settings_page",
			offset = offset,
			w_full = true,
			grow = 1,
			min_h = 0,
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
						local page = pages.find(key)
						return if page then pages.build(page) else ui.div {}
					end),
				},
			},
		},
	}
end

local function reveal(key: string)
	local item = elements[key]
	local first = elements[pages.sections[1].pages[1].key]
	if not item or not first or not nav_view or item.y == nil or first.y == nil then
		return
	end
	local at = item.y - first.y
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
			actions.close_settings()
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
		label = "User Settings",
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
			id = "settings_dialog",
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

return function()
	local window = ui.ref()
	sync()
	return ui.div {
		absolute = true,
		left = 0,
		top = 0,
		right = 0,
		bottom = 0,
		pointer_events = "none",
		ref = window,
		dialog(window),
	}
end
