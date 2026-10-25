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

function M.composer_changed(text: string, caret: number)
	ui.call_sync("composer.changed", { text = text, caret = caret })
end

function M.composer_key(event: any, text: string): boolean
	return ui.call_sync("composer.key", {
		key = event.key,
		shift = event.shift,
		cmd = event.cmd,
		text = text,
	}) == true
end

function M.composer_choose(index: number, text: string)
	ui.invoke("composer.choose", { index = index, text = text })
end

function M.composer_add_files(paths: { string })
	ui.invoke("composer.add_files", paths)
end

function M.composer_remove_file(id: string)
	ui.invoke("composer.remove_file", id)
end

function M.composer_cancel_reply()
	ui.invoke("composer.cancel_reply")
end

function M.composer_toggle_ping()
	ui.invoke("composer.toggle_ping")
end

function M.composer_cancel_edit()
	ui.invoke("composer.cancel_edit")
end

function M.composer_confirm()
	ui.invoke("composer.confirm")
end

function M.composer_retry(nonce: string)
	ui.invoke("composer.retry", nonce)
end

function M.composer_discard(nonce: string)
	ui.invoke("composer.discard", nonce)
end

function M.composer_dismiss()
	ui.invoke("composer.dismiss")
end

function M.composer_start_reply(message_id: string)
	ui.invoke("composer.start_reply", message_id)
end

function M.composer_start_edit(message_id: string)
	ui.invoke("composer.start_edit", message_id)
end

return M
