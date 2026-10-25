local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local widgets = require("./widgets")

local CARD_WIDTH = 220
local SWATCH_HEIGHT = 32
local CHIP = 20
local CHIP_FILL = 0.18
local LINKS_LINES = 5
local LINE = 20
local TEXT_INSET = 4
local TEXT_TOP = 6

local function of(source: string)
	return ui.computed(function()
		local found = {}
		for _, entry in state.themes.list:get() do
			if entry.source == source then
				table.insert(found, entry)
			end
		end
		return found
	end)
end

local function chip(text: string, color: string)
	return ui.row {
		relative = true,
		h = CHIP,
		px = theme.size.space_8,
		shrink = 0,
		items = "center",
		rounded = theme.size.radius_sm,
		ui.div {
			absolute = true,
			left = 0,
			top = 0,
			right = 0,
			bottom = 0,
			pointer_events = "none",
			rounded = theme.size.radius_sm,
			bg = color,
			opacity = CHIP_FILL,
		},
		ui.text {
			string.upper(text),
			literal = true,
			text_size = theme.size.font_tiny,
			weight = theme.weight.bold,
			color = color,
		},
	}
end

local function strip(entry: state.ThemeEntry)
	local cells = {}
	for _, color in entry.swatch do
		table.insert(cells, ui.div { grow = 1, h_full = true, bg = if color ~= "" then color else nil })
	end
	return ui.row {
		w_full = true,
		h = SWATCH_HEIGHT,
		rounded = theme.size.radius_sm,
		overflow = "hidden",
		table.unpack(cells),
	}
end

local function byline(entry: state.ThemeEntry): string
	local parts = {}
	if entry.version ~= "" then
		table.insert(parts, "v" .. entry.version)
	end
	if entry.author ~= "" then
		table.insert(parts, "by " .. entry.author)
	end
	if entry.scheme ~= "" then
		table.insert(parts, entry.scheme .. " only")
	end
	return table.concat(parts, " · ")
end

local function control(entry: state.ThemeEntry): any
	if entry.source == "builtin" then
		if entry.enabled then
			return chip("Applied", theme.color.success)
		end
		return widgets.button {
			id = "theme_apply_" .. entry.id,
			text = "Apply",
			tone = "primary",
			small = true,
			on_click = function()
				actions.theme_action("apply", entry.id)
			end,
		}
	elseif entry.source == "local" then
		return widgets.switch {
			id = "theme_switch_" .. entry.id,
			label = entry.name,
			checked = function(): boolean
				return entry.enabled
			end,
			on_change = function(on: boolean)
				actions.theme_action("toggle", { id = entry.id, on = on })
			end,
		}
	end
	if entry.enabled then
		return chip("On", theme.color.success)
	end
	return nil
end

local function card(entry: state.ThemeEntry)
	local line = byline(entry)
	return ui.column {
		w = CARD_WIDTH,
		gap = theme.size.space_12,
		p = theme.size.space_16,
		rounded = theme.size.radius_md,
		bg = theme.color.background_secondary,
		strip(entry),
		ui.row {
			w_full = true,
			items = "center",
			justify = "between",
			gap = theme.size.space_8,
			widgets.label(entry.name, theme.size.font_md, theme.color.header_primary, {
				literal = true,
				weight = theme.weight.semibold,
				truncate = true,
				min_w = 0,
				shrink = 1,
			}),
			control(entry),
		},
		if line ~= "" or entry.description ~= ""
			then ui.column {
				w_full = true,
				gap = theme.size.space_2,
				if line ~= "" then widgets.hint(line) else nil,
				if entry.description ~= ""
					then widgets.label(entry.description, theme.size.font_sm, theme.color.text_normal, {
						literal = true,
						wrap = true,
					})
					else nil,
			}
			else nil,
		if entry.error ~= ""
			then widgets.label(entry.error, theme.size.font_sm, theme.color.danger, { literal = true, wrap = true })
			else nil,
		if entry.note ~= "" then widgets.hint(entry.note) else nil,
	}
end

local function cards(source: string)
	return ui.each(of(source), card, {
		key = function(entry: state.ThemeEntry)
			return entry.id .. "|" .. tostring(entry.enabled) .. "|" .. table.concat(entry.swatch, ",") .. "|" .. entry.error .. "|" .. entry.note .. "|" .. entry.name
		end,
		w_full = true,
		direction = "row",
		flex_wrap = true,
		gap = theme.size.space_16,
	})
end

local function none(source: string): () -> boolean
	local list = of(source)
	return function()
		return #list:get() == 0
	end
end

local function section(title: string, ...: any)
	return ui.column {
		w_full = true,
		gap = theme.size.space_8,
		widgets.heading(title),
		...,
	}
end

local links_box = ui.component(function()
	local typed = ui.use_state(state.themes.links:get())
	return ui.column {
		w_full = true,
		gap = theme.size.space_8,
		widgets.hint(
			"Paste direct links to .css or theme.json files, one per line. Start a line with @light or @dark to use it only with light or dark colours."
		),
		ui.div {
			w_full = true,
			px = theme.size.space_8,
			py = theme.size.space_4,
			rounded = theme.size.radius_sm,
			bg = theme.color.input_background,
			ui.textarea {
				id = "theme_links",
				value = typed,
				label = "Theme links",
				placeholder = "https://example.github.io/theme/theme.css",
				placeholder_color = theme.color.text_muted,
				wrap = true,
				w_full = true,
				h = LINKS_LINES * LINE + TEXT_TOP * 2,
				px = TEXT_INSET,
				py = TEXT_TOP,
				bg = "#00000000",
				border = 0,
				focus = { border = 0 },
				text_size = theme.size.font_sm,
				line_height = LINE,
				color = theme.color.text_normal,
			},
		},
		ui.row {
			gap = theme.size.space_8,
			widgets.button {
				id = "theme_links_save",
				text = "Save Links",
				tone = "primary",
				on_click = function()
					actions.theme_action("set_links", typed:get())
				end,
			},
			widgets.button {
				text = "Refresh",
				on_click = function()
					actions.theme_action("refresh")
				end,
			},
		},
	}
end)

return function()
	return ui.column {
		w_full = true,
		gap = theme.size.space_24,
		ui.row {
			widgets.button {
				text = "Open Themes Folder",
				small = true,
				on_click = function()
					actions.theme_action("open_folder")
				end,
			},
		},
		section("Built In", cards("builtin")),
		section(
			"Local Themes",
			ui.show(none("local"), widgets.hint("No themes in the themes folder. Add a .css or theme.json file and it shows up here.")),
			cards("local")
		),
		section("Online Themes", links_box {}, cards("online")),
	}
end
