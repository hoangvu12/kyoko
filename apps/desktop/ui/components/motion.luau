local state = require("../state")
local theme = require("../theme")

local HOVER_SLIDE = 200
local HOVER_EASE = "inOutSine"

local M = {}

function M.reduced(): boolean
	local values = state.settings.values:get()
	local chosen = type(values) == "table" and values["accessibility.reduced_motion"] == true
	return chosen or ui.prefers.reduced_motion:get()
end

function M.duration(ms: number): number
	return if M.reduced() then 1 else ms
end

function M.moving(hovered: any): any
	return ui.computed(function()
		return hovered:get() and not M.reduced()
	end)
end

function M.slide(hovered: any, rest: string | number, shift: string): any
	local from = if type(rest) == "number" then rest else theme.number(rest)()
	return ui.tween(M.moving(hovered), {
		from = from,
		to = from + theme.number(shift)(),
		duration = HOVER_SLIDE,
		ease = HOVER_EASE,
	})
end

return M
