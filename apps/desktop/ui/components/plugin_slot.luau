local state = require("../state")

export type Props = {
	name: string,
	context: any?,
	direction: string?,
	gap: any?,
}

local function by_id(id: string): string
	return id
end

return function(props: Props)
	local direction = props.direction or "row"
	local ids = ui.computed(function()
		local slots = state.plugins.slots:get() or {}
		return slots[props.name] or {}
	end)
	return ui.show(function()
		return #ids:get() > 0
	end, ui.each(ids, function(id: string)
		return ui.slot(props.name, {
			from = id,
			context = props.context,
			direction = direction,
			gap = props.gap,
			items = "center",
			shrink = 0,
		})
	end, { key = by_id, direction = direction, gap = props.gap, items = "center", shrink = 0 }))
end
