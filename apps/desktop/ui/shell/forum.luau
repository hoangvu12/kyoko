local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")
local icon_button = require("@components/icon_button")
local avatar = require("@components/avatar")

local LEADING = 1.618
local BUTTON = 32
local CHIP = 28
local BADGE = 20
local AVATAR = 24
local GLYPH = 16
local EMPTY_GLYPH = 48
local MENU_WIDTH = 160
local SORTS = { "Recently Active", "Date Posted" }
local ARCHIVED_OPACITY = 0.7
local EXCERPT_LINES = 2

local font_sm = theme.number(theme.size.font_sm)
local font_md = theme.number(theme.size.font_md)

local function line(size: () -> number): () -> number
	return function()
		return size() * LEADING
	end
end

local function by_key(item: { key: string }): string
	return item.key
end

local function view(): state.ForumView
	return state.forum.view:get()
end

local function more_when_last(id: string): (boolean) -> ()
	return function(visible: boolean)
		local cards = state.forum.cards:peek()
		if visible and #cards > 0 and cards[#cards].id == id then
			actions.forum_more()
		end
	end
end

local function new_post()
	return ui.button {
		id = "forum_new_post",
		h = BUTTON,
		px = theme.size.space_12,
		py = 0,
		gap = theme.size.space_8,
		shrink = 0,
		items = "center",
		rounded = theme.size.radius_sm,
		border = 0,
		focus = { border = 0 },
		cursor = "pointer",
		bg = theme.color.brand,
		hover = { bg = theme.color.brand_hover },
		color = theme.color.text_on_brand,
		on_click = function()
			actions.forum_new_post()
		end,
		icon { name = "plus", size = GLYPH, color = theme.color.text_on_brand },
		ui.text { "New Post", text_size = theme.size.font_sm, weight = theme.weight.medium },
	}
end

local sort_menu = ui.component(function()
	local open = ui.use_state(false)
	local items = {}
	for index, label in SORTS do
		table.insert(
			items,
			ui.menu_item {
				label,
				on_click = function()
					actions.forum_sort(index)
				end,
			}
		)
	end
	return ui.menu {
		open = open,
		label = "Sort posts",
		w = MENU_WIDTH,
		px = theme.size.space_8,
		py = theme.size.space_6,
		gap = theme.size.space_2,
		rounded = theme.size.radius_sm,
		bg = theme.color.bg_3,
		border = theme.size.border_thickness,
		border_color = theme.color.border,
		placement = { side = "bottom", align = "end", offset = 4 },
		anchor = ui.button {
			h = BUTTON,
			px = theme.size.space_12,
			py = 0,
			gap = theme.size.space_8,
			shrink = 0,
			items = "center",
			rounded = theme.size.radius_sm,
			border = 1,
			border_color = theme.color.divider,
			focus = { border = 1 },
			cursor = "pointer",
			bg = "#00000000",
			hover = { bg = theme.color.modifier_hover },
			color = theme.color.text_normal,
			on_click = ui.toggle(open),
			ui.text {
				function()
					return view().sort
				end,
				literal = true,
				text_size = theme.size.font_sm,
			},
			icon { name = "chevron_down", size = GLYPH, color = theme.color.interactive_normal },
		},
		table.unpack(items),
	}
end)

local function layout_button(name: string, tooltip: string, gallery: boolean)
	local function active(): boolean
		return view().gallery == gallery
	end
	return ui.row {
		shrink = 0,
		rounded = theme.size.radius_sm,
		bg = function()
			return if active() then theme.color.modifier_selected else "#00000000"
		end,
		icon_button {
			name = name,
			tooltip = tooltip,
			filled_hover = true,
			active = active,
			on_click = function()
				actions.forum_layout(gallery)
			end,
		},
	}
end

local function tag_chip(tag: state.ForumTag)
	return ui.button {
		tag.label,
		id = "forum_tag_" .. tag.id,
		h = CHIP,
		px = theme.size.space_12,
		py = 0,
		shrink = 0,
		rounded = CHIP,
		border = 1,
		border_color = if tag.active then theme.color.brand else theme.color.divider,
		focus = { border = 1 },
		cursor = "pointer",
		bg = if tag.active then theme.color.brand_tint_strong else theme.color.background_secondary,
		hover = { bg = theme.color.modifier_selected },
		text_size = theme.size.font_sm,
		color = if tag.active then theme.color.interactive_active else theme.color.interactive_normal,
		on_click = function()
			actions.forum_toggle_tag(tag.id)
		end,
	}
end

local function toolbar()
	return ui.column {
		w_full = true,
		shrink = 0,
		gap = theme.size.space_12,
		px = theme.size.space_16,
		py = theme.size.space_12,
		ui.row {
			w_full = true,
			items = "center",
			gap = theme.size.space_8,
			new_post(),
			ui.div { grow = 1 },
			sort_menu(),
			layout_button("list_layout", "List View", false),
			layout_button("grid_layout", "Gallery View", true),
		},
		ui.show(function()
			return #state.forum.tags:get() > 0
		end, ui.row {
			w_full = true,
			flex_wrap = true,
			items = "center",
			gap = theme.size.space_8,
			ui.text {
				"TAGS",
				text_size = theme.size.font_tiny,
				weight = theme.weight.bold,
				color = theme.color.header_secondary,
			},
			ui.each(state.forum.tags, function(tag: state.ForumTag)
				return tag_chip(tag)
			end, { key = by_key, flex_wrap = true, gap = theme.size.space_8 }),
		}),
	}
end

local function badge(tag: state.ForumBadge)
	return ui.row {
		h = BADGE,
		shrink = 0,
		items = "center",
		px = theme.size.space_8,
		rounded = theme.size.radius_sm,
		bg = if tag.moderated then theme.color.tag_badge_moderated else theme.color.tag_badge,
		ui.text {
			tag.label,
			literal = true,
			text_size = theme.size.font_tiny,
			weight = theme.weight.bold,
			color = if tag.moderated then theme.color.brand else theme.color.interactive_normal,
		},
	}
end

local function title_row(card: state.ForumCard)
	return ui.row {
		w_full = true,
		items = "center",
		gap = theme.size.space_8,
		ui.show(card.pinned, icon { name = "pin", size = GLYPH, color = theme.color.brand }),
		ui.text {
			card.title,
			literal = true,
			min_w = 0,
			truncate = true,
			text_size = theme.size.font_md,
			line_height = line(font_md),
			weight = theme.weight.bold,
			color = theme.color.header_primary,
		},
	}
end

local function meta(card: state.ForumCard)
	return ui.row {
		w_full = true,
		flex_wrap = true,
		items = "center",
		gap = theme.size.space_8,
		avatar { src = card.avatar, size = AVATAR },
		ui.text {
			card.author,
			literal = true,
			shrink = 0,
			text_size = theme.size.font_sm,
			line_height = line(font_sm),
			weight = theme.weight.semibold,
			color = theme.color.header_secondary,
		},
		ui.row {
			items = "center",
			gap = theme.size.space_4,
			icon { name = "chat", size = GLYPH, color = theme.color.text_muted },
			ui.text {
				card.replies,
				literal = true,
				text_size = theme.size.font_sm,
				line_height = line(font_sm),
				color = theme.color.text_muted,
			},
		},
		ui.text {
			card.activity,
			literal = true,
			truncate = true,
			min_w = 0,
			text_size = theme.size.font_sm,
			line_height = line(font_sm),
			color = theme.color.text_muted,
		},
	}
end

local function badges(card: state.ForumCard)
	if #card.tags == 0 then
		return nil
	end
	local items = {}
	for _, tag in card.tags do
		table.insert(items, badge(tag))
	end
	return ui.row { flex_wrap = true, gap = theme.size.space_4, table.unpack(items) }
end

local function list_card(card: state.ForumCard)
	return ui.row {
		w_full = true,
		px = theme.size.space_16,
		pb = theme.size.space_8,
		ui.column {
			id = "forum_post_" .. card.id,
			grow = 1,
			min_w = 0,
			p = theme.size.space_16,
			gap = theme.size.space_8,
			rounded = theme.size.radius_md,
			bg = theme.color.background_secondary,
			hover = { bg = theme.color.modifier_selected },
			cursor = "pointer",
			opacity = if card.archived then ARCHIVED_OPACITY else 1,
			on_click = function()
				actions.forum_open_post(card.id)
			end,
			on_visible = more_when_last(card.id),
			title_row(card),
			if card.excerpt ~= ""
				then ui.text {
					card.excerpt,
					literal = true,
					wrap = true,
					lines = EXCERPT_LINES,
					text_size = theme.size.font_sm,
					line_height = line(font_sm),
					color = theme.color.text_muted,
				}
				else nil,
			badges(card),
			meta(card),
		},
	}
end

local function gallery_card(card: state.ForumCard)
	return ui.column {
		w = theme.size.forum_card_width,
		shrink = 0,
		rounded = theme.size.radius_md,
		overflow = "hidden",
		bg = theme.color.background_secondary,
		hover = { bg = theme.color.modifier_selected },
		cursor = "pointer",
		opacity = if card.archived then ARCHIVED_OPACITY else 1,
		on_click = function()
			actions.forum_open_post(card.id)
		end,
		on_visible = more_when_last(card.id),
		ui.row {
			w_full = true,
			h = theme.size.forum_thumb_height,
			justify = "center",
			items = "center",
			bg = theme.color.background_tertiary,
			icon { name = "image", size = EMPTY_GLYPH, color = theme.color.interactive_muted },
		},
		ui.column {
			gap = theme.size.space_8,
			p = theme.size.space_12,
			title_row(card),
			badges(card),
			meta(card),
		},
	}
end

local function empty()
	return ui.column {
		w_full = true,
		items = "center",
		gap = theme.size.space_8,
		p = theme.size.space_32,
		icon { name = "forum", size = EMPTY_GLYPH, color = theme.color.interactive_muted },
		ui.text {
			"No posts yet. Be the first to start a conversation.",
			text_size = theme.size.font_sm,
			color = theme.color.text_muted,
		},
	}
end

local function posts()
	return ui.scroll {
		grow = 1,
		min_h = 0,
		w_full = true,
		ui.show(
			function()
				return #state.forum.cards:get() == 0
			end,
			empty(),
			ui.show(
				function()
					return view().gallery
				end,
				ui.row {
					w_full = true,
					flex_wrap = true,
					gap = theme.size.space_16,
					px = theme.size.space_16,
					pb = theme.size.space_16,
					ui.each(state.forum.cards, function(card: state.ForumCard)
						return gallery_card(card)
					end, { key = by_key, flex_wrap = true, gap = theme.size.space_16 }),
				},
				ui.column {
					w_full = true,
					pb = theme.size.space_16,
					ui.each(state.forum.cards, function(card: state.ForumCard)
						return list_card(card)
					end, { key = by_key, w_full = true }),
				}
			)
		),
	}
end

return function()
	return ui.column {
		grow = 1,
		min_w = 0,
		min_h = 0,
		h_full = true,
		bg = theme.color.background_primary,
		toolbar(),
		posts(),
	}
end
