local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local icon = require("@components/icon")
local avatar = require("@components/avatar")

local SORTS = { "Recently Active", "Date Posted" }
local ARCHIVED_OPACITY = 0.7
local EXCERPT_LINES = 2

local icon_sm = theme.number(theme.size.icon_size_sm)
local icon_md = theme.number(theme.size.icon_size_md)
local avatar_xs = theme.number(theme.size.avatar_size_xs)
local border = theme.number(theme.size.border_thickness)

local function by_key(item: { key: string }): string
	return item.key
end

local function view(): state.ForumView
	return state.forum.view:get()
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

local function more_when_last(id: string): (boolean) -> ()
	return function(visible: boolean)
		local cards = state.forum.cards:peek()
		if visible and #cards > 0 and cards[#cards].id == id then
			actions.forum_more()
		end
	end
end

local function primary(label: string, id: string, height: any, on_click: () -> (), glyph: string?)
	return ui.button {
		id = id,
		h = height,
		px = theme.size.space_16,
		py = 0,
		gap = theme.size.space_8,
		shrink = 0,
		items = "center",
		rounded = theme.size.radius_sm,
		border = theme.size.border_thickness,
		border_color = theme.color.button_border,
		focus = { border = border() },
		cursor = "pointer",
		bg = theme.color.accent_3,
		hover = { bg = theme.color.accent_4 },
		color = theme.color.text_0,
		on_click = on_click,
		if glyph then icon { name = glyph, size = icon_md(), color = theme.color.text_0 } else nil,
		words(label, "md", { weight = theme.weight.medium }),
	}
end

local function search_row(query: any)
	return ui.row {
		w_full = true,
		gap = theme.size.space_8,
		ui.row {
			grow = 1,
			min_w = 0,
			h = theme.size.input_height,
			items = "center",
			gap = theme.size.space_8,
			px = theme.size.space_12,
			rounded = theme.size.radius_sm,
			bg = theme.color.bg_3,
			border = theme.size.border_thickness,
			border_color = theme.color.border_light,
			ui.input {
				id = "forum_search",
				grow = 1,
				min_w = 0,
				h = theme.size.line_height_label,
				p = 0,
				bg = "#00000000",
				border = 0,
				focus = { border = 0 },
				text_size = theme.size.font_size_md,
				line_height = theme.size.line_height_label,
				color = theme.color.text_3,
				placeholder = "Search or create a post...",
				placeholder_color = theme.color.text_5,
				value = query,
				on_key = function(event: any): boolean?
					if event.key == "Enter" then
						actions.forum_new_post(query:peek())
						return true
					end
					return nil
				end,
			},
			icon { name = "search", size = icon_md(), color = theme.color.text_4 },
		},
		primary("New Post", "forum_new_post", theme.size.input_height, function()
			actions.forum_new_post(query:peek())
		end, "chat_plus"),
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
	table.insert(
		items,
		ui.menu_item {
			"List View",
			on_click = function()
				actions.forum_layout(false)
			end,
		}
	)
	table.insert(
		items,
		ui.menu_item {
			"Gallery View",
			on_click = function()
				actions.forum_layout(true)
			end,
		}
	)
	return ui.menu {
		open = open,
		label = "Sort & View",
		w = theme.size.menu_width,
		px = theme.size.space_8,
		py = theme.size.space_6,
		gap = theme.size.space_2,
		rounded = theme.size.radius_sm,
		bg = theme.color.bg_3,
		border = theme.size.border_thickness,
		border_color = theme.color.border,
		placement = { side = "bottom", align = "start", offset = 4 },
		anchor = ui.button {
			id = "forum_sort",
			h = theme.size.button_height_sm,
			px = theme.size.space_10,
			py = 0,
			gap = theme.size.space_4,
			shrink = 0,
			items = "center",
			rounded = theme.size.radius_sm,
			border = 0,
			focus = { border = 0 },
			cursor = "pointer",
			bg = theme.color.bg_2,
			hover = { bg = theme.color.bg_1 },
			color = theme.color.text_3,
			on_click = ui.toggle(open),
			icon { name = "funnel", size = icon_sm(), color = theme.color.text_3 },
			words("Sort & View", "sm", { weight = theme.weight.medium }),
		},
		table.unpack(items),
	}
end)

local function tag_chip(tag: state.ForumTag)
	return ui.button {
		tag.label,
		id = "forum_tag_" .. tag.id,
		h = theme.size.button_height_sm,
		px = theme.size.space_10,
		py = 0,
		shrink = 0,
		rounded = theme.size.space_32,
		border = theme.size.border_thickness,
		border_color = if tag.active then theme.color.button_border else theme.color.border_light,
		focus = { border = border() },
		cursor = "pointer",
		bg = if tag.active then theme.color.accent_3 else theme.color.bg_2,
		hover = { bg = if tag.active then theme.color.accent_4 else theme.color.bg_1 },
		text_size = theme.size.font_size_sm,
		line_height = theme.size.line_height_sm,
		weight = theme.weight.medium,
		color = if tag.active then theme.color.text_0 else theme.color.text_3,
		on_click = function()
			actions.forum_toggle_tag(tag.id)
		end,
	}
end

local function any_active(): boolean
	for _, tag in state.forum.tags:get() do
		if tag.active then
			return true
		end
	end
	return false
end

local function filter_row()
	return ui.row {
		w_full = true,
		flex_wrap = true,
		items = "center",
		gap = theme.size.space_8,
		sort_menu(),
		ui.show(function()
			return #state.forum.tags:get() > 0
		end, ui.div {
			w = theme.size.border_thickness,
			h = theme.size.space_20,
			shrink = 0,
			bg = theme.color.border,
		}),
		ui.each(state.forum.tags, function(tag: state.ForumTag)
			return tag_chip(tag)
		end, { key = by_key, flex_wrap = true, gap = theme.size.space_8 }),
		ui.show(any_active, ui.button {
			"Clear",
			id = "forum_clear_tags",
			p = 0,
			border = 0,
			focus = { border = 0 },
			bg = "#00000000",
			cursor = "pointer",
			text_size = theme.size.font_size_sm,
			line_height = theme.size.line_height_sm,
			weight = theme.weight.medium,
			color = theme.color.accent_1,
			on_click = function()
				for _, tag in state.forum.tags:peek() do
					if tag.active then
						actions.forum_toggle_tag(tag.id)
					end
				end
			end,
		}),
	}
end

local function small_tag(label: string, extra: { [string]: any }?)
	local node = {
		h = theme.size.space_20,
		shrink = 0,
		items = "center",
		px = theme.size.space_6,
		rounded = theme.size.space_32,
		bg = theme.color.bg_2,
		border = theme.size.border_thickness,
		border_color = theme.color.border_light,
		words(label, "xs", { weight = theme.weight.medium, color = theme.color.text_3 }),
	}
	for key, item in extra or {} do
		node[key] = item
	end
	return ui.row(node)
end

local function file_chip(file: state.ForumFile, index: number)
	return ui.row {
		h = theme.size.button_height_sm,
		shrink = 0,
		items = "center",
		gap = theme.size.space_6,
		pl = theme.size.space_8,
		pr = theme.size.space_4,
		rounded = theme.size.radius_sm,
		bg = theme.color.bg_4,
		border = theme.size.border_thickness,
		border_color = theme.color.border,
		icon { name = "file", size = icon_sm(), color = theme.color.text_4 },
		words(file.name .. "  " .. file.size, "xs", { max_w = 220, truncate = true, color = theme.color.text_3 }),
		ui.button {
			w = theme.size.space_20,
			h = theme.size.space_20,
			p = 0,
			border = 0,
			focus = { border = 0 },
			bg = "#00000000",
			cursor = "pointer",
			justify = "center",
			items = "center",
			tooltip = "Remove",
			on_click = function()
				actions.forum_remove_file(index)
			end,
			icon { name = "close", size = icon_sm(), color = theme.color.text_4 },
		},
	}
end

local tag_picker = ui.component(function(props: { chosen: any })
	local open = ui.use_state(false)
	local items = {}
	for _, tag in state.forum.tags:peek() do
		table.insert(
			items,
			ui.menu_item {
				tag.label,
				on_click = function()
					local next = {}
					local found = false
					for _, id in props.chosen:peek() do
						if id == tag.id then
							found = true
						else
							table.insert(next, id)
						end
					end
					if not found then
						table.insert(next, tag.id)
					end
					props.chosen:set(next)
				end,
			}
		)
	end
	return ui.menu {
		open = open,
		label = "Add Tags",
		w = theme.size.menu_width,
		px = theme.size.space_8,
		py = theme.size.space_6,
		gap = theme.size.space_2,
		rounded = theme.size.radius_sm,
		bg = theme.color.bg_3,
		border = theme.size.border_thickness,
		border_color = theme.color.border,
		placement = { side = "bottom", align = "start", offset = 4 },
		anchor = ui.button {
			id = "forum_add_tags",
			h = theme.size.space_20,
			px = theme.size.space_6,
			py = 0,
			gap = theme.size.space_4,
			shrink = 0,
			items = "center",
			rounded = theme.size.space_32,
			border = theme.size.border_thickness,
			border_color = theme.color.border,
			focus = { border = border() },
			cursor = "pointer",
			bg = "#00000000",
			color = theme.color.text_4,
			on_click = ui.toggle(open),
			icon { name = "plus", size = icon_sm() * 0.75, color = theme.color.text_4 },
			words(function()
				return if view().require_tag then "Add Tags (required)" else "Add Tags"
			end, "xs", { weight = theme.weight.medium }),
		},
		table.unpack(items),
	}
end)

local new_post_form = ui.component(function()
	local title = ui.use_state(state.forum.view:peek().draft_title)
	local body = ui.use_state("")
	local chosen = ui.use_state({} :: { string })
	local chosen_tags = ui.computed(function()
		local labels: { any } = {}
		for _, id in chosen:get() do
			for _, tag in state.forum.tags:get() do
				if tag.id == id then
					table.insert(labels, { key = id, label = tag.label, add = false })
				end
			end
		end
		if #state.forum.tags:get() > 0 then
			table.insert(labels, { key = "add", label = "", add = true })
		end
		return labels
	end)
	local indexed_files = ui.computed(function()
		local files = {}
		for index, file in state.forum.files:get() do
			table.insert(files, { key = file.key, name = file.name, size = file.size, index = index })
		end
		return files
	end)
	local function ready(): boolean
		local titled = string.match(title:get(), "%S") ~= nil
		return titled and (not view().require_tag or #chosen:get() > 0)
	end
	return ui.column {
		id = "forum_new_post_form",
		w_full = true,
		gap = theme.size.space_12,
		p = theme.size.space_16,
		rounded = theme.size.radius_md,
		bg = theme.color.bg_3,
		border = theme.size.border_thickness,
		border_color = theme.color.accent_2,
		ui.input {
			id = "forum_post_title",
			w_full = true,
			h = theme.size.line_height_lg,
			p = 0,
			bg = "#00000000",
			border = 0,
			focus = { border = 0 },
			text_size = theme.size.font_size_lg,
			line_height = theme.size.line_height_lg,
			weight = theme.weight.semibold,
			color = theme.color.text_2,
			placeholder = "Post Title",
			placeholder_color = theme.color.text_5,
			value = title,
		},
		ui.row {
			w_full = true,
			flex_wrap = true,
			items = "center",
			gap = theme.size.space_6,
			ui.each(chosen_tags, function(tag: any)
				return if tag.add then tag_picker { chosen = chosen } else small_tag(tag.label)
			end, { key = by_key, flex_wrap = true, gap = theme.size.space_6 }),
		},
		ui.textarea {
			id = "forum_post_body",
			w_full = true,
			h = theme.size.forum_post_body_height,
			px = theme.size.space_12,
			py = theme.size.space_10,
			rounded = theme.size.radius_sm,
			bg = theme.color.bg_4,
			border = theme.size.border_thickness,
			border_color = theme.color.border_light,
			focus = { border_color = theme.color.accent_2 },
			text_size = theme.size.font_size_md,
			line_height = theme.size.line_height_md,
			color = theme.color.text_3,
			placeholder = "Write the first message of your post",
			placeholder_color = theme.color.text_5,
			value = body,
		},
		ui.show(function()
			return #state.forum.files:get() > 0
		end, ui.row {
			w_full = true,
			flex_wrap = true,
			gap = theme.size.space_8,
			ui.each(indexed_files, function(file: any)
				return file_chip(file, file.index)
			end, { key = by_key, flex_wrap = true, gap = theme.size.space_8 }),
		}),
		ui.row {
			w_full = true,
			items = "center",
			gap = theme.size.space_8,
			ui.button {
				id = "forum_post_attach",
				w = theme.size.button_size,
				h = theme.size.button_size,
				p = 0,
				border = 0,
				focus = { border = 0 },
				bg = "#00000000",
				rounded = theme.size.radius_sm,
				hover = { bg = theme.color.hover },
				cursor = "pointer",
				justify = "center",
				items = "center",
				tooltip = "Upload a File",
				on_click = function()
					actions.forum_attach()
				end,
				icon { name = "attach", size = icon_md(), color = theme.color.text_4 },
			},
			ui.div { grow = 1 },
			ui.button {
				"Cancel",
				id = "forum_post_cancel",
				h = theme.size.button_height_md,
				px = theme.size.space_16,
				py = 0,
				border = 0,
				focus = { border = 0 },
				bg = "#00000000",
				cursor = "pointer",
				text_size = theme.size.font_size_sm,
				line_height = theme.size.line_height_sm,
				weight = theme.weight.medium,
				color = theme.color.text_3,
				hover = { color = theme.color.text_2 },
				on_click = function()
					actions.forum_cancel_post()
				end,
			},
			ui.button {
				"Post",
				id = "forum_post_submit",
				h = theme.size.button_height_md,
				px = theme.size.space_16,
				py = 0,
				shrink = 0,
				rounded = theme.size.radius_sm,
				border = theme.size.border_thickness,
				border_color = theme.color.button_border,
				focus = { border = border() },
				cursor = "pointer",
				bg = theme.color.accent_3,
				hover = { bg = theme.color.accent_4 },
				text_size = theme.size.font_size_sm,
				line_height = theme.size.line_height_label,
				weight = theme.weight.medium,
				color = theme.color.text_0,
				opacity = function()
					return if ready() then 1 else theme.number(theme.size.disabled_opacity_control)()
				end,
				on_click = function()
					if ready() then
						actions.forum_post(title:peek(), body:peek(), chosen:peek())
					end
				end,
			},
		},
	}
end)

local function card_tags(card: state.ForumCard)
	local items = {}
	for _, tag in card.tags do
		table.insert(items, small_tag(tag.label))
	end
	return items
end

local function title_row(card: state.ForumCard)
	local row = {
		w_full = true,
		flex_wrap = true,
		items = "center",
		gap = theme.size.space_6,
		if card.pinned then icon { name = "pin", size = icon_sm(), color = theme.color.accent_2 } else nil,
		words(card.title, "md", {
			min_w = 0,
			weight = theme.weight.semibold,
			color = if card.archived then theme.color.text_4 else theme.color.text_2,
		}),
	}
	for _, item in card_tags(card) do
		table.insert(row, item)
	end
	return ui.row(row)
end

local function author_line(card: state.ForumCard)
	return ui.row {
		w_full = true,
		items = "center",
		gap = theme.size.space_6,
		avatar { src = card.avatar, size = avatar_xs() },
		words(card.author .. ":", "sm", { shrink = 0, weight = theme.weight.medium, color = theme.color.text_2 }),
		words(card.excerpt, "sm", { min_w = 0, truncate = true, color = theme.color.text_4 }),
	}
end

local function stats(card: state.ForumCard)
	return ui.row {
		items = "center",
		gap = theme.size.space_12,
		ui.row {
			items = "center",
			gap = theme.size.space_4,
			icon { name = "chat", size = icon_sm(), color = theme.color.text_4 },
			words(card.count, "xs", { weight = theme.weight.semibold, color = theme.color.text_4 }),
		},
		words(card.activity, "xs", { color = theme.color.text_4 }),
	}
end

local function list_card(card: state.ForumCard)
	return ui.column {
		id = "forum_post_" .. card.id,
		w_full = true,
		gap = theme.size.space_6,
		px = theme.size.space_16,
		py = theme.size.space_12,
		rounded = theme.size.radius_md,
		bg = theme.color.bg_3,
		border = theme.size.border_thickness,
		border_color = theme.color.border,
		hover = { bg = theme.color.bg_2 },
		cursor = "pointer",
		opacity = if card.archived then ARCHIVED_OPACITY else 1,
		on_click = function()
			actions.forum_open_post(card.id)
		end,
		on_visible = more_when_last(card.id),
		title_row(card),
		author_line(card),
		stats(card),
	}
end

local function gallery_card(card: state.ForumCard)
	return ui.column {
		id = "forum_post_" .. card.id,
		w = theme.size.forum_card_width,
		shrink = 0,
		rounded = theme.size.radius_md,
		overflow = "hidden",
		bg = theme.color.bg_3,
		border = theme.size.border_thickness,
		border_color = theme.color.border,
		hover = { bg = theme.color.bg_2 },
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
			bg = theme.color.bg_2,
			icon { name = "image", size = icon_md() * 2, color = theme.color.text_5 },
		},
		ui.column {
			gap = theme.size.space_6,
			px = theme.size.space_16,
			py = theme.size.space_12,
			title_row(card),
			if card.excerpt ~= ""
				then words(card.excerpt, "sm", { wrap = true, lines = EXCERPT_LINES, color = theme.color.text_4 })
				else nil,
			stats(card),
		},
	}
end

local function empty(message: string)
	return ui.column {
		w_full = true,
		items = "center",
		gap = theme.size.space_8,
		p = theme.size.space_32,
		icon { name = "forum", size = icon_md() * 2, color = theme.color.text_5 },
		words(message, "sm", { color = theme.color.text_4 }),
	}
end

local function posts(cards: any)
	return ui.scroll {
		grow = 1,
		min_h = 0,
		w_full = true,
		ui.show(
			function()
				return #cards:get() == 0
			end,
			empty(if #state.forum.cards:peek() == 0
				then "No posts yet. Be the first to start a conversation."
				else "No posts match your search."),
			ui.show(
				function()
					return view().gallery
				end,
				ui.row {
					w_full = true,
					flex_wrap = true,
					gap = theme.size.space_16,
					pt = theme.size.space_8,
					px = theme.size.space_16,
					pb = theme.size.space_16,
					ui.each(cards, function(card: state.ForumCard)
						return gallery_card(card)
					end, { key = by_key, flex_wrap = true, gap = theme.size.space_16 }),
				},
				ui.column {
					w_full = true,
					pt = theme.size.space_8,
					px = theme.size.space_16,
					pb = theme.size.space_16,
					ui.each(cards, function(card: state.ForumCard)
						return list_card(card)
					end, { key = by_key, w_full = true, gap = theme.size.space_8 }),
				}
			)
		),
	}
end

return ui.component(function()
	local query = ui.use_state("")
	local cards = ui.computed(function()
		local text = string.lower(query:get())
		local all = state.forum.cards:get()
		if not string.match(text, "%S") then
			return all
		end
		local shown = {}
		for _, card in all do
			if string.find(string.lower(card.title), text, 1, true) then
				table.insert(shown, card)
			end
		end
		return shown
	end)
	return ui.column {
		grow = 1,
		min_w = 0,
		min_h = 0,
		h_full = true,
		bg = theme.color.bg_4,
		ui.column {
			w_full = true,
			shrink = 0,
			gap = theme.size.space_12,
			pt = theme.size.space_16,
			px = theme.size.space_16,
			pb = theme.size.space_8,
			ui.show(function()
				return view().composing
			end, ui.presence(function()
				return tostring(view().serial)
			end, function()
				return new_post_form()
			end), search_row(query)),
			filter_row(),
		},
		posts(cards),
	}
end)
