local state = require("../state")
local actions = require("../actions")
local kit = require("./kit")

return {
	headings = {
		{
			key = "data",
			title = "Data",
			rows = {
				{
					kind = "custom",
					title = "Data Folder",
					note = function()
						return state.settings.about:get().data_dir
					end,
					keywords = "folder files cache storage disk directory",
					control = function()
						return kit.button {
							text = "Open Folder",
							on_click = function()
								actions.settings_action("open_data_folder")
							end,
						}
					end,
				},
			},
		},
	},
}
