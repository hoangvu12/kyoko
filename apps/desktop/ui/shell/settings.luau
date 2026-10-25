local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")
local pages = require("@settings/pages")
local widgets = require("@settings/widgets")

local NAV_WIDTH = 218
local CONTENT_WIDTH = 740
local CONTENT_ROOM = 140
local NAV_ROOM = 40
local TOP = 60
local NAV_ITEM = 32
local NAV_PAD = 10
local NAV_ICON = 18
local CLOSE = 36
local CLOSE_ICON = 18
local TITLE_LINE = 24

local function page_key(): string
	return state.settings.page:get()
end

local nav_item = ui.component(function(props: { page: pages.Page })
	local hovered = ui.use_state(false)
	local function selected(): boolean
		return page_key() == props.page.key
	end
	local function color(): string
		if selected() then
			return theme.color.interactive_active
		end
		return if hovered:get() then theme.color.interactive_hover else theme.color.interactive_normal
	end
	return ui.button {
		w_full = true,
		h = NAV_ITEM,
		px = NAV_PAD,
		gap = theme.size.space_8,
		items = "center",
		border = 0,
		focus = { border = 0 },
		rounded = theme.size.radius_sm,
		cursor = "pointer",
		bg = function()
			if selected() then
				return theme.color.modifier_selected
			end
			return if hovered:get() then theme.color.modifier_hover else "#00000000"
		end,
		on_hover = function(inside: boolean)
			hovered:set(inside)
		end,
		on_click = function()
			actions.open_settings(props.page.key)
		end,
		icon { name = props.page.icon, size = NAV_ICON, color = color },
		ui.text {
			props.page.title,
			literal = true,
			min_w = 0,
			truncate = true,
			text_size = theme.size.font_md,
			weight = theme.weight.medium,
			color = color,
		},
	}
end)

local log_out = ui.component(function()
	local hovered = ui.use_state(false)
	return ui.button {
		w_full = true,
		h = NAV_ITEM,
		px = NAV_PAD,
		gap = theme.size.space_8,
		items = "center",
		border = 0,
		focus = { border = 0 },
		rounded = theme.size.radius_sm,
		cursor = "pointer",
		bg = function()
			return if hovered:get() then theme.color.modifier_hover else "#00000000"
		end,
		on_hover = function(inside: boolean)
			hovered:set(inside)
		end,
		on_click = actions.sign_out(),
		icon { name = "log_out", size = NAV_ICON, color = theme.color.danger },
		ui.text {
			"Log Out",
			text_size = theme.size.font_md,
			weight = theme.weight.medium,
			color = theme.color.interactive_normal,
		},
	}
end)

local function nav()
	local items = {}
	for _, section in pages.sections do
		if section.title then
			table.insert(
				items,
				ui.div {
					px = NAV_PAD,
					pt = theme.size.space_16,
					pb = theme.size.space_4,
					widgets.label(string.upper(section.title), theme.size.font_tiny, theme.color.header_secondary, {
						weight = theme.weight.bold,
						literal = true,
					}),
				}
			)
		end
		for _, page in section.pages do
			table.insert(items, nav_item { page = page })
		end
		if not section.title then
			table.insert(items, ui.div { h = theme.size.space_8 })
		end
	end
	table.insert(items, ui.div { mt = theme.size.space_8, mb = theme.size.space_8, mx = NAV_PAD, h = 1, bg = theme.color.divider })
	table.insert(items, log_out())
	table.insert(
		items,
		ui.div {
			mx = NAV_PAD,
			mt = theme.size.space_8,
			widgets.label(function()
				return "Kyoko " .. state.settings.about:get().version
			end, theme.size.font_tiny, theme.color.text_muted, { literal = true }),
		}
	)
	return ui.scroll {
		w = NAV_WIDTH,
		h_full = true,
		shrink = 0,
		ui.column {
			w_full = true,
			pt = TOP,
			pl = theme.size.space_20,
			pr = 6,
			pb = theme.size.space_32,
			table.unpack(items),
		},
	}
end

local function content()
	return ui.column {
		w_full = true,
		gap = theme.size.space_24,
		ui.text {
			function()
				local page = pages.find(page_key())
				return if page then page.title else ""
			end,
			literal = true,
			text_size = theme.size.font_lg,
			line_height = TITLE_LINE,
			weight = theme.weight.semibold,
			color = theme.color.header_primary,
		},
		ui.presence(function()
			local key = page_key()
			return if key ~= "" then key else nil
		end, function(key: string)
			local page = pages.find(key)
			return if page then page.build() else ui.div {}
		end),
	}
end

local close_button = ui.component(function()
	local hovered = ui.use_state(false)
	return ui.column {
		absolute = true,
		top = TOP,
		right = theme.number(theme.size.space_16),
		items = "center",
		gap = theme.size.space_4,
		ui.button {
			w = CLOSE,
			h = CLOSE,
			p = 0,
			rounded = CLOSE,
			border = 2,
			focus = { border = 2 },
			justify = "center",
			items = "center",
			cursor = "pointer",
			tooltip = "Close",
			border_color = function()
				return if hovered:get() then theme.color.interactive_hover else theme.color.interactive_normal
			end,
			bg = function()
				return if hovered:get() then theme.color.modifier_hover else "#00000000"
			end,
			on_hover = function(inside: boolean)
				hovered:set(inside)
			end,
			on_click = actions.close_settings,
			icon { name = "close", size = CLOSE_ICON, color = theme.color.interactive_normal },
		},
		ui.text {
			"ESC",
			text_size = theme.size.font_xs,
			weight = theme.weight.bold,
			color = theme.color.interactive_normal,
		},
	}
end)

local function view()
	local root = ui.ref()
	local function right(): number
		local width = root.width or 0
		return math.max(0, math.min(CONTENT_WIDTH + CONTENT_ROOM, width - NAV_WIDTH - NAV_ROOM))
	end
	return ui.row {
		ref = root,
		absolute = true,
		top = theme.number(theme.size.title_bar_height),
		left = 0,
		right = 0,
		bottom = 0,
		bg = theme.color.background_primary,
		color = theme.color.text_normal,
		focusable = true,
		on_mount = function(element: any)
			element:focus()
		end,
		on_key = function(event: any): boolean?
			if event.key == "Escape" then
				actions.close_settings()
				return true
			end
			return false
		end,
		ui.row {
			grow = 1,
			min_w = 0,
			h_full = true,
			justify = "end",
			bg = theme.color.background_secondary,
			nav(),
		},
		ui.div {
			relative = true,
			w = right,
			h_full = true,
			shrink = 0,
			ui.scroll {
				w_full = true,
				max_w = CONTENT_WIDTH,
				h_full = true,
				ui.column {
					w_full = true,
					pt = TOP,
					pl = theme.size.space_32,
					pr = theme.size.space_16,
					pb = theme.size.space_32,
					content(),
				},
			},
			close_button(),
		},
	}
end

return function()
	return ui.show(function()
		return page_key() ~= ""
	end, view())
end
