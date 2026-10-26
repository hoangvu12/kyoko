local state = require("../state")
local actions = require("../actions")
local kit = require("./kit")

local function shares_activity(): boolean
	local found = state.settings.account:get()
	return found == nil or (found :: any).shares_activity ~= false
end

return {
	headings = {
		{
			key = "data-privacy",
			title = "Data & Privacy",
			keywords = "privacy safety data dms filter",
			rows = {
				kit.web_note("Privacy & Safety settings, such as who can message you and how your data is used, are managed in Discord's web app."),
			},
		},
		{
			key = "activity-privacy",
			title = "Activity Privacy",
			rows = {
				{
					kind = "toggle",
					id = "settings_share_activity",
					title = "Share my activity",
					note = "Let people see what you're playing or listening to in Discord. Kyoko itself shares no activity; this is your account's setting.",
					keywords = "activity status game playing presence rich",
					checked = shares_activity,
					on_change = function(on: any)
						actions.share_activity(on == true)
					end,
				},
			},
		},
	},
}
