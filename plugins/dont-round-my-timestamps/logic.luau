type Time = { past: boolean, unit: string, value: number }

return {
	format_relative = function(time: Time)
		local amount = math.floor(time.value)
		local span = if amount == 1 then `1 {time.unit}` else `{amount} {time.unit}s`
		if time.past then
			return `{span} ago`
		end
		return `in {span}`
	end,
}
