local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local kit = require("./kit")

local NAME_LIMIT = 100
local DESCRIPTION_LINES = 3

local name = ui.state("name", "")
local description = ui.state("description", "")
local picking = false

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

local function dirty(): boolean
	return name_changed() or description_changed() or saved().pending
end

local function reset()
	local current = saved()
	name:set(current.name)
	description:set(current.description)
	actions.server_action("reset_icon")
end

local function save()
	actions.server_action("save_overview", {
		name = if name_changed() then wanted_name() else nil,
		description = if description_changed() then trimmed(description:get()) else nil,
	})
end

local function pick_icon()
	if picking then
		return
	end
	picking = true
	ui.task(function()
		local ok, files = pcall(function()
			return ui.open_files({ title = "Upload Image", multiple = false }):await()
		end)
		picking = false
		if ok and files[1] then
			actions.server_action("icon", files[1].path)
		end
	end)
end

local function initials(text: string): string
	local letters = {}
	for word in string.gmatch(text, "%S+") do
		table.insert(letters, string.sub(word, 1, 1))
	end
	return table.concat(letters)
end

local function tile()
	local side = theme.number(theme.size.avatar_size_lg)() + theme.number(theme.size.space_20)()
	return ui.row {
		id = "server_overview_icon",
		w = side,
		h = side,
		shrink = 0,
		rounded = theme.size.radius_lg,
		overflow = "hidden",
		justify = "center",
		items = "center",
		bg = theme.color.text_1,
		ui.presence(function()
			local picture = saved().icon
			return if picture ~= "" then picture else nil
		end, function(picture: string)
			return ui.image { src = picture, w = side, h = side, fit = "cover" }
		end),
		ui.show(function()
			return saved().icon == ""
		end, kit.text(function()
			return initials(saved().name)
		end, theme.size.font_size_xl, theme.size.line_height_xl, theme.color.accent_5, {
			weight = theme.weight.bold,
		})),
	}
end

local function follow_saved()
	local seen: state.ServerOverview? = nil
	ui.effect(function()
		local current = state.server.overview:get()
		if current and (seen == nil or seen.name ~= current.name or seen.description ~= current.description) then
			seen = current
			name:set(current.name)
			description:set(current.description)
		end
	end)
end

local function build()
	follow_saved()
	return ui.column {
		w_full = true,
		kit.title("Server Overview"),
		ui.row {
			w_full = true,
			mt = theme.size.settings_row_gap,
			gap = theme.size.space_24,
			items = "start",
			tile(),
			ui.column {
				grow = 1,
				basis = 0,
				min_w = 0,
				gap = theme.size.space_8,
				items = "start",
				kit.hint("We recommend an image of at least 512x512 for the server."),
				kit.button { id = "server_upload_icon", text = "Upload Image", on_click = pick_icon },
			},
			ui.column {
				grow = 1,
				basis = 0,
				min_w = 0,
				kit.field("Server Name", kit.input(name, "", { id = "server_overview_name", label = "Server Name" })),
			},
		},
		kit.divider(),
		kit.label("Server Description", "Tell people what this server is about."),
		ui.div {
			w_full = true,
			mt = theme.size.space_12,
			kit.textarea(description, "Tell people what this server is about", DESCRIPTION_LINES, {
				id = "server_overview_description",
				label = "Server Description",
			}),
		},
	}
end

return {
	keywords = "server overview profile name icon image upload description",
	build = build,
	save_bar = function()
		return ui.show(dirty, kit.save_bar(reset, save))
	end,
}
