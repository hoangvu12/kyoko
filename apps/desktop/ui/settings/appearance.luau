local theme = require("../theme")
local state = require("../state")
local icon = require("@components/icon")
local kit = require("./kit")

local SYSTEM = "system"
local FALLBACK = "midnight"

local function current(): string
	local found = kit.value("theme")
	return if type(found) == "string" then found else FALLBACK
end

local function built_in()
	return ui.computed(function()
		local found = {}
		for _, entry in state.themes.list:get() do
			if entry.source == "builtin" then
				table.insert(found, entry)
			end
		end
		return found
	end)
end

local swatch = ui.component(function(props: { entry: state.ThemeEntry })
	local entry = props.entry
	local function chosen(): boolean
		return current() == entry.id
	end
	return ui.button {
		id = "settings_theme_" .. entry.id,
		tooltip = entry.name,
		label = entry.name,
		role = "radio",
		checked = chosen,
		w = theme.size.theme_swatch_size,
		h = theme.size.theme_swatch_size,
		shrink = 0,
		p = 0,
		rounded = theme.size.radius_sm,
		cursor = "pointer",
		justify = "center",
		items = "center",
		bg = entry.swatch[3] or entry.swatch[1] or theme.color.bg_4,
		border = function()
			return if chosen() then theme.number(theme.size.space_2)() else theme.number(theme.size.border_thickness)()
		end,
		border_color = function()
			return if chosen() then theme.color.accent_2 else theme.color.border
		end,
		focus = { border_color = theme.color.accent_2 },
		on_click = function()
			kit.set("theme", entry.id)
		end,
		ui.show(chosen, icon { name = "check", size = theme.size.icon_size_md :: any, color = entry.swatch[5] or theme.color.text_3 }),
	}
end)

local function swatches()
	return ui.column {
		w_full = true,
		gap = theme.size.space_16,
		kit.caption("Default Themes"),
		ui.each(built_in(), function(entry: state.ThemeEntry)
			return swatch { entry = entry }
		end, {
			key = function(entry: state.ThemeEntry)
				return entry.id .. "|" .. table.concat(entry.swatch, ",")
			end,
			direction = "row",
			flex_wrap = true,
			gap = theme.size.space_10,
		}),
	}
end

return {
	headings = {
		{
			key = "theme",
			title = "Theme",
			rows = {
				{
					id = "settings_device_theme",
					title = "Same as Device Theme",
					note = "Match your device's light or dark mode.",
					keywords = "system dark light mode colour color",
					checked = function()
						return current() == SYSTEM
					end,
					on_change = function(on: any)
						kit.set("theme", if on then SYSTEM else FALLBACK)
					end,
				},
				{
					kind = "block",
					title = "Default Themes",
					keywords = "midnight dark light amoled colour color palette",
					build = swatches,
				},
			},
		},
		{
			key = "messages",
			title = "Messages",
			rows = {
				{
					key = "chat.inline_media",
					title = "Show images and videos",
					note = "When they are posted as links or uploaded to Discord.",
					keywords = "inline media attachments pictures",
				},
				{
					key = "chat.link_previews",
					title = "Show embeds and link previews",
					keywords = "embed unfurl url",
				},
				{
					key = "chat.show_reactions",
					title = "Show emoji reactions",
					keywords = "reactions",
				},
				{
					kind = "related",
					page = "accessibility",
					icon = "accessibility",
					title = "Accessibility",
					note = "Adjust the visual density of chat messages and UI",
				},
			},
		},
		{
			key = "scaling",
			title = "Scaling",
			rows = {
				{
					kind = "slider",
					key = "appearance.ui_scale",
					title = "Zoom Level",
					note = "Saved for a later version of Kyoko, which will scale the interface.",
					keywords = "dpi scale size zoom",
					min = 0.5,
					max = 2,
					step = 0.05,
					scale = 100,
					reset = 1,
				},
			},
		},
	},
}
