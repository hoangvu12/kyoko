local actions = require("./actions")
local frame = require("@shell/frame")

ui.effect(function()
	actions.media_reduced_motion(ui.prefers.reduced_motion:get())
end)

return frame()
