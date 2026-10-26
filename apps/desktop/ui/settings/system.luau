local kit = require("./kit")

return {
	headings = {
		{
			key = "startup",
			title = "Startup",
			rows = {
				{
					key = "startup.open_on_login",
					title = "Open Kyoko on Login",
					note = kit.note_or("startup.open_on_login", "Start Kyoko when you sign in to your computer."),
					keywords = "startup boot windows autostart",
				},
				{
					key = "window.minimize_to_tray",
					title = "Minimize to Tray",
					note = "Closing the window keeps Kyoko running in the tray.",
					keywords = "tray background close",
				},
				{
					key = "integration.discord_links",
					title = "Open Discord Links",
					note = kit.note_or("integration.discord_links", "Open discord:// links in Kyoko."),
					keywords = "protocol handler discord://",
				},
			},
		},
	},
}
