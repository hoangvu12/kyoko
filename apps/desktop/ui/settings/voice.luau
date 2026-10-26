local kit = require("./kit")

return {
	headings = {
		{
			key = "voice",
			title = "Voice & Video",
			rows = {
				{
					kind = "block",
					title = "Kyoko has no voice calls",
					keywords = "voice video microphone camera call input output",
					build = function()
						return kit.notice("Kyoko has no voice calls, so there is nothing to set up here.", "headphones")
					end,
				},
			},
		},
	},
}
