local theme = require("../theme")
local state = require("../state")
local widgets = require("./widgets")
local icon_button = require("@components/icon_button")

local function hosts(): { string }
	local found = (state.settings.values:get() or {})["trusted_link_hosts"]
	return if type(found) == "table" then found else {}
end

local function forget(host: string)
	local kept = {}
	for _, item in hosts() do
		if item ~= host then
			table.insert(kept, item)
		end
	end
	widgets.set("trusted_link_hosts", kept)
end

local function trusted_list()
	local list = ui.computed(function()
		return hosts()
	end)
	return ui.column {
		w_full = true,
		ui.show(function()
			return #hosts() == 0
		end, ui.div { py = theme.size.space_12, widgets.hint("No trusted sites yet. Choose \"Always trust\" when you open a link.") }),
		ui.each(list, function(host: string)
			return widgets.row(host, nil, icon_button {
				name = "close",
				tooltip = "Stop trusting " .. host,
				on_click = function()
					forget(host)
				end,
			})
		end),
	}
end

return function()
	return ui.column {
		w_full = true,
		gap = theme.size.space_24,
		widgets.placeholder("shield", "Privacy & Safety settings are managed in Discord's web app."),
		ui.column {
			w_full = true,
			widgets.heading("Safety Locks"),
			widgets.toggle_row(
				"safety_locks",
				"Hold risky actions",
				"Kyoko holds actions that look automated to Discord, such as messaging strangers or posting in empty channels, until you confirm. Turning this off is at your own risk. Applies after a restart."
			),
		},
		ui.column {
			w_full = true,
			widgets.heading("Links"),
			widgets.toggle_row(
				"chat.link_warning",
				"Warn before opening links",
				"Ask before opening a link to a site you have not trusted."
			),
			ui.div { pt = theme.size.space_16, widgets.heading("Trusted Sites") },
			trusted_list(),
		},
	}
end
