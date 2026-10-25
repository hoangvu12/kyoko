export type Props = {
	name: string | () -> string,
	size: number?,
	color: any?,
}

local function source(name: string | () -> string): any
	if type(name) == "function" then
		return function()
			return "icons/" .. name() .. ".svg"
		end
	end
	return "icons/" .. name .. ".svg"
end

return function(props: Props)
	local size = props.size or 20
	return ui.svg {
		src = source(props.name),
		w = size,
		h = size,
		shrink = 0,
		tint = props.color or "currentColor",
	}
end
