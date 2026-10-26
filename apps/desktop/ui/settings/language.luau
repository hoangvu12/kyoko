local LOCALES = {
	{ label = "English (US)", value = "en-US" },
}

return {
	headings = {
		{
			key = "language",
			title = "Language",
			rows = {
				{
					kind = "radios",
					key = "locale",
					title = "Language",
					note = "More languages are coming. A change of language applies after a restart.",
					keywords = "locale translation english",
					options = LOCALES,
				},
			},
		},
		{
			key = "time",
			title = "Time",
			rows = {
				{
					key = "chat.time_24h",
					title = "24-hour time",
					note = "Show times as 14:30 instead of 2:30 PM.",
					keywords = "clock timestamp hours",
				},
			},
		},
	},
}
