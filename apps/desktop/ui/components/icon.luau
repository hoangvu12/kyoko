export type Props = {
	name: string,
	size: number?,
	color: any?,
}

return function(props: Props)
	local size = props.size or 20
	return ui.svg {
		src = "icons/" .. props.name .. ".svg",
		w = size,
		h = size,
		shrink = 0,
		tint = props.color or "currentColor",
	}
end
