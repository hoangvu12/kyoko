local kit = require("./kit")

return {
	headings = {
		{
			key = "developer",
			title = "Developer",
			rows = {
				{
					key = "developer.developer_mode",
					title = "Developer Mode",
					note = "Show Copy ID in menus everywhere.",
					keywords = "id snowflake copy debug",
				},
				{
					kind = "custom",
					title = "Renderer",
					note = "Chosen by the graphics backend at launch.",
					keywords = "gpu graphics wgpu",
					control = function()
						return kit.shown_value("wgpu")
					end,
				},
			},
		},
		{
			key = "credits",
			title = "Credits",
			rows = {
				{
					kind = "block",
					title = "Credits",
					keywords = "license licence notices third party twemoji gemoji",
					build = function()
						return kit.hint(
							"Emoji graphics are Twemoji by Twitter, Jason Sofonia, Justine De Caires and contributors (jdecked/twemoji), licensed under CC-BY 4.0. Emoji names come from GitHub's gemoji (MIT). Every notice is in THIRD_PARTY_NOTICES.md."
						)
					end,
				},
			},
		},
	},
}
