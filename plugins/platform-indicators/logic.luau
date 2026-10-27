type User = { bot: boolean, platforms: { [string]: string }? }

local PLATFORMS = {
	{ key = "desktop", icon = "monitor.svg", label = "Desktop" },
	{ key = "mobile", icon = "smartphone.svg", label = "Mobile" },
	{ key = "web", icon = "globe.svg", label = "Web" },
}

local STATUSES = {
	online = { color = "$kyoko.status_online", label = "Online" },
	idle = { color = "$kyoko.status_idle", label = "Idle" },
	dnd = { color = "$kyoko.status_dnd", label = "Do Not Disturb" },
	offline = { color = "$kyoko.status_offline", label = "Offline" },
}

local function mark(platform: any, status: string)
	local shown = STATUSES[status] or STATUSES.offline
	local color = shown.color
	if platform.key == "mobile" and kyoko.settings.get("color_mobile") == false then
		color = "$kyoko.text_4"
	end
	return { icon = platform.icon, color = color, tooltip = `{shown.label} on {platform.label}` }
end

local function marks(platforms: { [string]: string })
	local list = {}
	for _, platform in PLATFORMS do
		local status = platforms[platform.key]
		if status ~= nil then
			table.insert(list, mark(platform, status))
		end
	end
	return list
end

local function shown(key: string, list: { any })
	if kyoko.settings.get(key) == false then
		return nil
	end
	return list
end

return {
	decorate_user = function(user: User)
		if user.bot or user.platforms == nil then
			return nil
		end
		local list = marks(user.platforms)
		if #list == 0 then
			return nil
		end
		return {
			members = shown("members", list),
			messages = shown("messages", list),
			profiles = shown("profiles", list),
		}
	end,
}
