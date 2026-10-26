local actions = require("../actions")
local kit = require("./kit")

return {
	headings = {
		{
			key = "overview",
			title = "Overview",
			rows = {
				{
					key = "notifications.enabled",
					title = "Enable Desktop Notifications",
					note = "Show a notification when you are mentioned or get a direct message. For one server or channel, right-click it and choose Notification Settings.",
					keywords = "alerts toast windows",
				},
				{
					key = "notifications.show_content",
					title = "Show Message Content",
					note = "Include the start of the message in the notification.",
					keywords = "preview text",
				},
				{
					kind = "custom",
					title = "Test Notification",
					note = kit.note_of("notifications.test"),
					keywords = "try send",
					control = function()
						return kit.button {
							text = "Send Test",
							on_click = function()
								actions.settings_action("test_notification")
							end,
						}
					end,
				},
			},
		},
		{
			key = "sounds",
			title = "Sounds",
			rows = {
				{
					key = "notifications.sounds",
					title = "Notification Sounds",
					note = "Play a sound with each notification.",
					keywords = "audio ding mute",
				},
			},
		},
	},
}
