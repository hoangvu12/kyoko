local M = {}

function M.reconnect()
	return ui.action("session.reconnect")
end

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

function M.open_hidden_channel(id: string)
	return ui.action("sidebar.open_hidden", id)
end

function M.toggle_category(id: string)
	return ui.action("sidebar.toggle_category", id)
end

function M.sidebar_action(name: string, id: string?)
	ui.invoke("sidebar." .. name, id or "")
end

function M.sidebar_send(name: string, payload: any)
	ui.invoke("sidebar." .. name, payload)
end

function M.channel_level(id: string): string
	local level = ui.call_sync("sidebar.channel_level", id)
	return if type(level) == "string" then level else "inherit"
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

function M.login_cancel_add()
	ui.invoke("login.cancel_add")
end

function M.login_qr_refresh()
	ui.invoke("login.qr_refresh")
end

function M.login_web_open()
	ui.invoke("login.web_open")
end

function M.login_web_cancel()
	ui.invoke("login.web_cancel")
end

function M.login_password_submit(login: string, password: string)
	ui.invoke("login.password_submit", { login = login, password = password })
end

function M.login_mfa_verify(code: string)
	ui.invoke("login.mfa_verify", code)
end

function M.login_mfa_method(method: string)
	ui.invoke("login.mfa_method", method)
end

function M.login_password_back()
	ui.invoke("login.password_back")
end

function M.login_mode(mode: string)
	ui.invoke("login.mode", mode)
end

function M.login_discard_unsaved()
	ui.invoke("login.discard_unsaved")
end

function M.sign_out()
	return ui.action("login.sign_out")
end

local function area_of(area: string?): string
	return area or "composer"
end

function M.composer_changed(text: string, caret: number, area: string?)
	ui.call_sync(area_of(area) .. ".changed", { text = text, caret = caret })
end

function M.composer_key(event: any, text: string, area: string?): boolean
	return ui.call_sync(area_of(area) .. ".key", {
		key = event.key,
		shift = event.shift,
		cmd = event.cmd,
		text = text,
	}) == true
end

function M.composer_choose(index: number, text: string, area: string?)
	ui.invoke(area_of(area) .. ".choose", { index = index, text = text })
end

function M.composer_add_files(paths: { string }, area: string?)
	ui.invoke(area_of(area) .. ".add_files", paths)
end

function M.composer_remove_file(id: string, area: string?)
	ui.invoke(area_of(area) .. ".remove_file", id)
end

function M.composer_cancel_reply(area: string?)
	ui.invoke(area_of(area) .. ".cancel_reply")
end

function M.composer_toggle_ping(area: string?)
	ui.invoke(area_of(area) .. ".toggle_ping")
end

function M.composer_cancel_edit(area: string?)
	ui.invoke(area_of(area) .. ".cancel_edit")
end

function M.composer_confirm(area: string?)
	ui.invoke(area_of(area) .. ".confirm")
end

function M.composer_retry(nonce: string, area: string?)
	ui.invoke(area_of(area) .. ".retry", nonce)
end

function M.composer_discard(nonce: string, area: string?)
	ui.invoke(area_of(area) .. ".discard", nonce)
end

function M.composer_dismiss(area: string?)
	ui.invoke(area_of(area) .. ".dismiss")
end

function M.composer_plus(at: { x: number, y: number }, area: string?)
	ui.invoke(area_of(area) .. ".plus", at)
end

function M.composer_spoiler(id: string, area: string?)
	ui.invoke(area_of(area) .. ".spoiler", id)
end

function M.composer_edit_file(id: string, area: string?)
	ui.invoke(area_of(area) .. ".edit_file", id)
end


function M.composer_command_section(index: number, area: string?)
	ui.invoke(area_of(area) .. ".command_section", index)
end

function M.composer_option_changed(index: number, value: string, area: string?)
	ui.invoke(area_of(area) .. ".option_changed", { index = index, value = value })
end

function M.composer_option_key(event: any, index: number, value: string, area: string?): boolean
	return ui.call_sync(area_of(area) .. ".option_key", {
		key = event.key,
		shift = event.shift,
		index = index,
		value = value,
	}) == true
end

function M.composer_option_focus(index: number, area: string?)
	ui.invoke(area_of(area) .. ".option_focus", index)
end

function M.composer_toggle_optional(area: string?)
	ui.invoke(area_of(area) .. ".toggle_optional")
end

function M.composer_add_optional(index: number, area: string?)
	ui.invoke(area_of(area) .. ".add_optional", index)
end

function M.composer_cancel_command(area: string?)
	ui.invoke(area_of(area) .. ".cancel_command")
end

function M.composer_choose_option(index: number, area: string?)
	ui.invoke(area_of(area) .. ".choose_option", index)
end

function M.composer_retry_choices(area: string?)
	ui.invoke(area_of(area) .. ".retry_choices")
end

function M.composer_option_files(paths: { string }, area: string?)
	ui.invoke(area_of(area) .. ".option_files", paths)
end

function M.composer_remove_option_file(index: number, area: string?)
	ui.invoke(area_of(area) .. ".remove_option_file", index)
end
function M.composer_backgrounds(text: string): { any }
	local result = ui.call_sync("composer.backgrounds", text)
	return if type(result) == "table" then result else {}
end

function M.composer_picker(tab: string, toolbar: any)
	ui.invoke("composer.picker", { tab = tab, toolbar = toolbar })
end

function M.composer_start_reply(message_id: string)
	ui.invoke("composer.start_reply", message_id)
end

function M.composer_start_edit(message_id: string)
	ui.invoke("composer.start_edit", message_id)
end

function M.member_scroll(offset: number, height: number, member: number, group: number)
	ui.call_sync("members.scroll", { offset = offset, height = height, member = member, group = group })
end

function M.open_member_profile(id: string, x: number, y: number)
	ui.invoke("members.open_profile", { id = id, x = x, y = y })
end

function M.open_status_menu(x: number, y: number)
	ui.invoke("user.status_menu", { x = x, y = y })
end

function M.open_settings(page: string?)
	ui.invoke("settings.open", page or "")
end

function M.close_settings()
	ui.invoke("settings.close")
end

function M.set_setting(key: string, value: any)
	ui.invoke("settings.set", { key = key, value = value })
end

function M.settings_action(name: string)
	ui.invoke("settings." .. name)
end

function M.share_activity(on: boolean)
	ui.invoke("settings.share_activity", on)
end

function M.run_search(query: string)
	ui.invoke("search.run", query)
end

function M.close_search()
	ui.invoke("search.close")
end

function M.more_results()
	ui.invoke("search.more")
end

function M.open_result(id: string)
	ui.invoke("search.open_result", id)
end

function M.result_menu(id: string, x: number, y: number)
	ui.invoke("search.menu", { id = id, x = x, y = y })
end

function M.plugin_action(name: string, argument: any?)
	ui.invoke("plugins." .. name, argument)
end

function M.theme_action(name: string, argument: any?)
	if name == "apply" then
		ui.invoke("themes.apply", argument)
	elseif name == "toggle" then
		ui.invoke("themes.toggle", argument)
	elseif name == "set_links" then
		ui.invoke("themes.set_links", argument)
	elseif name == "refresh" then
		ui.invoke("themes.refresh", argument)
	elseif name == "open_folder" then
		ui.invoke("themes.open_folder", argument)
	end
end

function M.open_threads(x: number, y: number)
	ui.invoke("threads.open_list", { x = x, y = y })
end

function M.open_pins(x: number, y: number)
	ui.invoke("threads.open_pins", { x = x, y = y })
end

function M.open_inbox(x: number, y: number)
	ui.invoke("inbox.open", { x = x, y = y })
end

function M.close_thread()
	ui.invoke("threads.close")
end

function M.join_thread(id: string)
	ui.invoke("threads.join", id)
end

function M.leave_thread(id: string)
	ui.invoke("threads.leave", id)
end

function M.archive_thread(id: string, archived: boolean)
	ui.invoke("threads.archive", { id = id, archived = archived })
end

function M.forum_new_post(title: string?)
	ui.invoke("forum.new_post", title or "")
end

function M.forum_open_post(id: string)
	ui.invoke("forum.open_post", id)
end

function M.forum_toggle_tag(id: string)
	ui.invoke("forum.toggle_tag", id)
end

function M.forum_sort(index: number)
	ui.invoke("forum.sort", index)
end

function M.forum_layout(gallery: boolean)
	ui.invoke("forum.layout", gallery)
end

function M.forum_more()
	ui.invoke("forum.more")
end

function M.create_thread(name: string, message: string, private: boolean)
	ui.invoke("threads.create", { name = name, message = message, private = private })
end

function M.forum_post(title: string, body: string, tags: { string })
	ui.invoke("forum.post", { title = title, body = body, tags = tags })
end

function M.forum_cancel_post()
	ui.invoke("forum.cancel_post")
end

function M.forum_attach()
	ui.invoke("forum.attach")
end

function M.forum_remove_file(index: number)
	ui.invoke("forum.remove_file", index)
end

function M.search_tab(tab: string)
	ui.invoke("search.tab", tab)
end

function M.search_page(page: number)
	ui.invoke("search.page", page)
end

function M.search_sort(oldest: boolean)
	ui.invoke("search.sort", oldest)
end

function M.member_menu(id: string, x: number, y: number)
	ui.invoke("members.menu", { id = id, x = x, y = y })
end

function M.open_server_settings(page: string?)
	ui.invoke("server.open", page or "")
end

function M.close_server_settings()
	ui.invoke("server.close")
end

function M.server_action(name: string, argument: any?)
	ui.invoke("server." .. name, argument)
end

function M.friends_tab(key: string)
	return ui.action("friends.select_tab", key)
end

function M.friends_filter(text: string)
	ui.invoke("friends.filter", text)
end

function M.add_friend(username: string)
	ui.invoke("friends.add", username)
end

function M.friends_act(name: string, id: string)
	ui.invoke("friends." .. name, id)
end

function M.open_friend_profile(id: string, x: number, y: number)
	ui.invoke("friends.open_profile", { id = id, x = x, y = y })
end

function M.open_full_profile(id: string)
	ui.invoke("dms.full_profile", id)
end

function M.open_dm_profile_menu(id: string, x: number, y: number)
	ui.invoke("dms.profile_more", { id = id, x = x, y = y })
end

function M.add_dm_profile_friend()
	ui.invoke("dms.add_friend", nil)
end

function M.composer_choose_result(index: number, area: string?)
	ui.invoke(area_of(area) .. ".choose_result", index)
end

function M.member_avatar_hover(name: string, inside: boolean)
	ui.call_sync("members.avatar_hover", { name = name, inside = inside })
end

return M
