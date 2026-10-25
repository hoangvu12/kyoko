local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local widgets = require("@settings/widgets")
local kit = require("@settings/server_kit")

local TILE = 96
local NAME_LIMIT = 100
local DESCRIPTION_LINES = 3
local LINE = 22
local TEXT_INSET = 10
local TEXT_TOP = 8

local name = ui.state("server.overview_name", "")
local description = ui.state("server.overview_description", "")

local function saved(): state.ServerOverview
	return state.server.overview:get() or { name = "", description = "", icon = "", pending = false }
end

local function trimmed(text: string): string
	return (string.gsub(text, "^%s*(.-)%s*$", "%1"))
end

local function wanted_name(): string
	return string.sub(trimmed(name:get()), 1, NAME_LIMIT)
end

local function name_changed(): boolean
	local value = wanted_name()
	return value ~= "" and value ~= saved().name
end

local function description_changed(): boolean
	return trimmed(description:get()) ~= saved().description
end

local function reset()
	local current = saved()
	name:set(current.name)
	description:set(current.description)
	actions.server_action("reset_icon")
end

local picking = false

local function pick_icon()
	if picking then
		return
	end
	picking = true
	ui.task(function()
		local ok, files = pcall(function()
			return ui.open_files({ title = "Upload Icon", multiple = false }):await()
		end)
		picking = false
		if ok and files[1] then
			actions.server_action("icon", files[1].path)
		end
	end)
end

local function save()
	actions.server_action("save_overview", {
		name = if name_changed() then wanted_name() else nil,
		description = if description_changed() then trimmed(description:get()) else nil,
	})
end

local function initials(text: string): string
	local letters = {}
	for word in string.gmatch(text, "%S+") do
		table.insert(letters, string.sub(word, 1, 1))
	end
	return table.concat(letters)
end

local function tile()
	return ui.row {
		w = TILE,
		h = TILE,
		shrink = 0,
		rounded = TILE,
		overflow = "hidden",
		justify = "center",
		items = "center",
		bg = theme.color.background_tertiary,
		ui.presence(function()
			local icon = saved().icon
			return if icon ~= "" then icon else nil
		end, function(icon: string)
			return ui.image { src = icon, w = TILE, h = TILE, fit = "cover" }
		end),
		ui.show(function()
			return saved().icon == ""
		end, widgets.label(function()
			return initials(saved().name)
		end, theme.size.font_lg, theme.color.header_primary, { literal = true })),
	}
end

return function()
	local seen: state.ServerOverview? = nil
	ui.effect(function()
		local current = state.server.overview:get()
		if current and (seen == nil or seen.name ~= current.name or seen.description ~= current.description) then
			seen = current
			name:set(current.name)
			description:set(current.description)
		end
	end)
	return ui.column {
		w_full = true,
		grow = 1,
		min_h = 0,
		kit.title("Overview"),
		ui.scroll {
			w_full = true,
			grow = 1,
			min_h = 0,
			ui.column {
				w_full = true,
				gap = theme.size.space_24,
				pb = theme.size.space_32,
				ui.row {
					items = "center",
					gap = theme.size.space_24,
					tile(),
					ui.column {
						items = "start",
						gap = theme.size.space_8,
						kit.button {
							text = "Upload Icon",
							on_click = pick_icon,
						},
						widgets.hint("Minimum 128 by 128. A square image works best."),
					},
				},
				kit.field("Server name", kit.input(name, "")),
				kit.field(
					"Description",
					ui.div {
						w_full = true,
						px = theme.size.space_12,
						py = theme.size.space_8,
						rounded = theme.size.radius_sm,
						bg = theme.color.input_background,
						ui.textarea {
							value = description,
							label = "Description",
							placeholder = "Tell people what this server is about",
							placeholder_color = theme.color.text_muted,
							wrap = true,
							w_full = true,
							h = DESCRIPTION_LINES * LINE + TEXT_TOP * 2,
							px = TEXT_INSET,
							py = TEXT_TOP,
							bg = "#00000000",
							border = 0,
							focus = { border = 0 },
							text_size = theme.size.font_sm,
							line_height = LINE,
							color = theme.color.text_normal,
						},
					}
				),
			},
		},
		ui.show(function()
			return name_changed() or description_changed() or saved().pending
		end, kit.save_bar(reset, save)),
	}
end
