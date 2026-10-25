local M = {}

function M.select_guild(id: string)
	return ui.action("rail.select_guild", id)
end

function M.select_channel(id: string)
	return ui.action("sidebar.select_channel", id)
end

function M.minimize_window()
	return ui.action("window.minimize")
end

function M.maximize_window()
	return ui.action("window.maximize")
end

function M.close_window()
	return ui.action("window.close")
end

function M.report_slot(name: string, element: any)
	if element.x == nil or element.y == nil then
		return
	end
	ui.call_sync("frame.slot", { name = name, x = element.x, y = element.y, w = element.width, h = element.height })
end

function M.media_visible(names: { string })
	ui.call_sync("media.visible", names)
end

function M.media_hover(name: string)
	ui.call_sync("media.hover", name)
end

function M.media_reduced_motion(system: boolean)
	ui.call_sync("media.reduced_motion", system)
end

return M
