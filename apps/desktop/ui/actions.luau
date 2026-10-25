local M = {}

function M.select_guild(id: string)
	return ui.action("rail.select_guild", id)
end

function M.select_home()
	return ui.action("rail.select_home")
end

function M.toggle_folder(id: string)
	return ui.action("rail.toggle_folder", id)
end

function M.add_server()
	return ui.action("rail.add_server")
end

function M.discover()
	return ui.action("rail.discover")
end

function M.rail_drop(payload: any, drop: string, target: string)
	ui.invoke("rail.drop", { guild = payload.guild, folder = payload.folder, drop = drop, target = target })
end

function M.select_channel(id: string)
	return ui.action("sidebar.select_channel", id)
end

function M.toggle_category(id: string)
	return ui.action("sidebar.toggle_category", id)
end

function M.sidebar_action(name: string, id: string?)
	ui.invoke("sidebar." .. name, id or "")
end

function M.select_dm(id: string)
	return ui.action("dms.select", id)
end

function M.close_dm(id: string)
	ui.invoke("dms.close", id)
end

function M.show_home_page(page: string)
	return ui.action("home.show_page", page)
end

function M.dm_action(name: string)
	return ui.action("dms." .. name)
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

function M.login_acknowledge()
	ui.invoke("login.acknowledge")
end

function M.login_submit(token: string)
	ui.invoke("login.submit", token)
end

function M.login_continue()
	ui.invoke("login.continue")
end

function M.sign_out()
	return ui.action("login.sign_out")
end

return M
