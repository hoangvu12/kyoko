local theme = require("../theme")

export type Props = {
	drop: string,
	target: string,
	inset: number,
	on_drop: (payload: any, drop: string, target: string) -> (),
}

return ui.component(function(props: Props)
	local over = ui.use_state(false)
	return ui.div {
		absolute = true,
		top = -8,
		left = props.inset,
		w = 48,
		h = 8,
		rounded = 4,
		bg = function()
			return if over:get() then theme.color.brand else "#00000000"
		end,
		on_drag_over = function()
			over:set(true)
		end,
		on_drag_leave = function()
			over:set(false)
		end,
		on_drop = function(payload: any)
			over:set(false)
			props.on_drop(payload, props.drop, props.target)
		end,
	}
end)
