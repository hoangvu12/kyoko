local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local kit = require("./kit")

local SWATCH_WIDTH = 72
local SWATCH_HEIGHT = 12
local CHIP = 20
local CHIP_FILL = 0.18
local LINKS_LINES = 5
local LINE = 20
local TEXT_INSET = 4
local TEXT_TOP = 6

local tab = ui.state("themes_tab", "local")

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
		w = SWATCH_WIDTH,
		h = SWATCH_HEIGHT,
		shrink = 0,
		rounded = theme.size.radius_xs,
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
			return chip("Applied", theme.color.green_2)
		end
		return kit.button {
			id = "theme_apply_" .. entry.id,
			text = "Apply",
			on_click = function()
				actions.theme_action("apply", entry.id)
			end,
		}
	elseif entry.source == "local" then
		return kit.vc_switch {
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
		return chip("On", theme.color.green_2)
	end
	return nil
end

local function card(entry: state.ThemeEntry)
	local line = byline(entry)
	return ui.column {
		w_full = true,
		gap = theme.size.space_8,
		p = theme.size.space_16,
		rounded = theme.size.radius_sm,
		bg = theme.color.bg_3,
		ui.row {
			w_full = true,
			items = "center",
			gap = theme.size.space_12,
			ui.column {
				grow = 1,
				min_w = 0,
				kit.text(entry.name, theme.size.font_size_md, theme.size.line_height_label, theme.color.text_2, {
					weight = theme.weight.semibold,
					truncate = true,
				}),
				if line ~= "" then kit.text(line, theme.size.font_size_xs, theme.size.line_height_xs, theme.color.text_4) else nil,
			},
			strip(entry),
			control(entry),
		},
		if entry.description ~= "" then kit.hint(entry.description) else nil,
		if entry.error ~= ""
			then kit.text(entry.error, theme.size.font_size_sm, theme.size.line_height_sm, theme.color.red_2, { wrap = true })
			else nil,
		if entry.note ~= "" then kit.hint(entry.note) else nil,
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
		kit.caption(title),
		...,
	}
end

local tab_button = ui.component(function(props: { key: string, title: string })
	local function chosen(): boolean
		return tab:get() == props.key
	end
	return ui.button {
		id = "theme_tab_" .. props.key,
		p = 0,
		pb = theme.size.space_8,
		bg = "#00000000",
		rounded = 0,
		border = 0,
		focus = { border = 0 },
		border_sides = function()
			return if chosen() then { 0, 0, 2, 0 } else { 0, 0, 0, 0 }
		end,
		border_color = theme.color.accent_1,
		cursor = "pointer",
		role = "tab",
		checked = chosen,
		on_click = function()
			tab:set(props.key)
		end,
		ui.text {
			props.title,
			literal = true,
			text_size = theme.size.font_size_sm,
			line_height = theme.size.line_height_sm,
			weight = theme.weight.semibold,
			color = function()
				return if chosen() then theme.color.accent_1 else theme.color.text_3
			end,
		},
	}
end)

local function warning()
	return ui.column {
		w_full = true,
		gap = theme.size.space_8,
		p = theme.size.space_16,
		rounded = theme.size.radius_sm,
		bg = theme.color.background_feedback_warning,
		border = theme.size.border_thickness,
		border_color = theme.color.border_feedback_warning,
		kit.caption("How Kyoko reads themes"),
		kit.text(
			"Kyoko takes a theme's colours, sizes and fonts: a theme.json, or the variable block of a .css theme. Selectors for Discord's web page are skipped, so a theme never slows the app down.",
			theme.size.font_size_sm,
			theme.size.line_height_sm,
			theme.color.text_3,
			{ wrap = true }
		),
	}
end

local links_box = ui.component(function()
	local typed = ui.use_state(state.themes.links:get())
	return ui.column {
		w_full = true,
		gap = theme.size.space_8,
		kit.hint(
			"Paste direct links to .css or theme.json files, one per line. Start a line with @light or @dark to use it only with light or dark colours."
		),
		ui.div {
			w_full = true,
			px = theme.size.space_8,
			py = theme.size.space_4,
			rounded = theme.size.radius_sm,
			bg = theme.color.bg_3,
			border = theme.size.border_thickness,
			border_color = theme.color.border_light,
			ui.textarea {
				id = "theme_links",
				value = typed,
				label = "Theme links",
				placeholder = "https://example.github.io/theme/theme.css",
				placeholder_color = theme.color.text_5,
				wrap = true,
				w_full = true,
				h = LINKS_LINES * LINE + TEXT_TOP * 2,
				px = TEXT_INSET,
				py = TEXT_TOP,
				bg = "#00000000",
				border = 0,
				focus = { border = 0 },
				text_size = theme.size.font_size_sm,
				line_height = LINE,
				color = theme.color.text_3,
			},
		},
		ui.row {
			gap = theme.size.space_8,
			kit.button {
				id = "theme_links_save",
				text = "Save Links",
				tone = "primary",
				on_click = function()
					actions.theme_action("set_links", typed:get())
				end,
			},
			kit.button {
				text = "Refresh",
				on_click = function()
					actions.theme_action("refresh")
				end,
			},
		},
	}
end)

local function local_tab()
	return ui.column {
		w_full = true,
		gap = theme.size.space_16,
		warning(),
		kit.card(
			kit.caption("Themes Folder"),
			kit.hint("Put a .css or theme.json file in the themes folder and it shows up below."),
			ui.row {
				pt = theme.size.space_8,
				gap = theme.size.space_8,
				kit.button {
					text = "Open Themes Folder",
					icon = "folder",
					on_click = function()
						actions.theme_action("open_folder")
					end,
				},
				kit.button {
					text = "Refresh",
					icon = "refresh",
					on_click = function()
						actions.theme_action("refresh")
					end,
				},
			}
		),
		section("Built In", cards("builtin")),
		section(
			"Local Themes",
			ui.show(none("local"), kit.hint("No themes in the themes folder. Add a .css or theme.json file and it shows up here.")),
			cards("local")
		),
	}
end

local function online_tab()
	return ui.column {
		w_full = true,
		gap = theme.size.space_16,
		section("Online Themes", links_box {}, cards("online")),
	}
end

return function()
	return ui.column {
		w_full = true,
		gap = theme.size.space_10,
		ui.row {
			gap = theme.size.space_56,
			tab_button { key = "local", title = "Local Themes" },
			tab_button { key = "online", title = "Online Themes" },
		},
		ui.presence(function()
			return tab:get()
		end, function(key: string)
			return if key == "online" then online_tab() else local_tab()
		end),
	}
end
