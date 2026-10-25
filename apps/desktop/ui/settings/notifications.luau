local widgets = require("./widgets")
local actions = require("../actions")

return function()
	return ui.column {
		w_full = true,
		widgets.heading("Desktop Notifications"),
		widgets.toggle_row(
			"notifications.enabled",
			"Enable Desktop Notifications",
			"Show a notification when you are mentioned or get a direct message."
		),
		widgets.toggle_row("notifications.sounds", "Notification Sounds", "Play a sound with each notification."),
		widgets.toggle_row(
			"notifications.show_content",
			"Show Message Content",
			"Include the start of the message in the notification."
		),
		widgets.row(
			"Test Notification",
			widgets.note("notifications.test"),
			widgets.button {
				text = "Send Test",
				small = true,
				on_click = function()
					actions.settings_action("test_notification")
				end,
			}
		),
	}
end
