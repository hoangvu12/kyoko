local theme = require("../theme")
local widgets = require("./widgets")

local LOCALES = {
	{ label = "English (US)", value = "en-US" },
}

return function()
	return ui.column {
		w_full = true,
		gap = theme.size.space_8,
		widgets.heading("Language"),
		widgets.setting_radios("locale", LOCALES),
		ui.div {
			pt = theme.size.space_8,
			widgets.hint("More languages are coming. A change of language applies after a restart."),
		},
	}
end
