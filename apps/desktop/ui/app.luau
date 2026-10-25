local actions = require("./actions")
local pages = require("@settings/pages")
local frame = require("@shell/frame")

pages.add_section({
	key = "kyoko",
	title = "Kyoko",
	pages = {
		{ key = "plugins", title = "Plugins", icon = "plugins", build = require("@settings/plugins") },
		{ key = "themes", title = "Themes", icon = "themes", build = require("@settings/themes") },
	},
})

ui.effect(function()
	actions.media_reduced_motion(ui.prefers.reduced_motion:get())
end)

return frame()
