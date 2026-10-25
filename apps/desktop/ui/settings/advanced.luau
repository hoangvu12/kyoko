local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local widgets = require("./widgets")

local function note_or(key: string, fallback: string): () -> string
	local note = widgets.note(key)
	return function()
		local found = note()
		return if found ~= "" then found else fallback
	end
end

return function()
	return ui.column {
		w_full = true,
		gap = theme.size.space_16,
		ui.column {
			w_full = true,
			widgets.heading("Startup"),
			widgets.toggle_row(
				"startup.open_on_login",
				"Open Kyoko on Login",
				note_or("startup.open_on_login", "Start Kyoko when you sign in to your computer.")
			),
			widgets.toggle_row(
				"window.minimize_to_tray",
				"Minimize to Tray",
				"Closing the window keeps Kyoko running in the tray."
			),
			widgets.toggle_row(
				"integration.discord_links",
				"Open Discord Links",
				note_or("integration.discord_links", "Open discord:// links in Kyoko.")
			),
		},
		ui.column {
			w_full = true,
			widgets.heading("Developer"),
			widgets.toggle_row("developer.developer_mode", "Developer Mode", "Show Copy ID in menus everywhere."),
			widgets.row(
				"Renderer",
				"Chosen by the graphics backend at launch.",
				widgets.label("wgpu", theme.size.font_md, theme.color.text_muted, { literal = true })
			),
		},
		ui.column {
			w_full = true,
			widgets.heading("Data"),
			widgets.row(
				"Data Folder",
				function()
					return state.settings.about:get().data_dir
				end,
				widgets.button {
					text = "Open Folder",
					small = true,
					on_click = function()
						actions.settings_action("open_data_folder")
					end,
				}
			),
		},
		ui.column {
			w_full = true,
			widgets.heading("GIF Usage", theme.size.space_8),
			ui.row {
				gap = theme.size.space_12,
				items = "center",
				widgets.toggle("advanced.share_gif_usage", "Tell Discord which GIFs I pick"),
				widgets.label("Tell Discord which GIFs I pick", theme.size.font_sm, theme.color.text_normal, { literal = true }),
			},
			widgets.hint("Sends the GIF you choose back to Discord to improve search results. Off by default."),
		},
	}
end
