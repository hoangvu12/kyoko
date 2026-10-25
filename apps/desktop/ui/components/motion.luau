local state = require("../state")

local M = {}

function M.reduced(): boolean
	local values = state.settings.values:get()
	local chosen = type(values) == "table" and values["accessibility.reduced_motion"] == true
	return chosen or ui.prefers.reduced_motion:get()
end

function M.duration(ms: number): number
	return if M.reduced() then 1 else ms
end

return M
