local theme = require("../theme")

local MAX_DISPLAYED = 999

export type Props = {
	count: () -> number,
	ring: string?,
}

local function label(count: number): string
	if count > MAX_DISPLAYED then
		return MAX_DISPLAYED .. "+"
	end
	return tostring(count)
end

return function(props: Props)
	local ringed = props.ring ~= nil
	return ui.row {
		shrink = 0,
		items = "center",
		justify = "center",
		min_w = if ringed then 20 else 16,
		h = if ringed then 20 else 16,
		px = 4,
		rounded = 10,
		bg = theme.color.mention_badge,
		border = if ringed then 2 else 0,
		border_color = props.ring,
		ui.text {
			function()
				return label(props.count())
			end,
			text_size = theme.size.font_tiny,
			weight = theme.weight.bold,
			line_height = 16,
			color = theme.color.text_on_brand,
		},
	}
end
