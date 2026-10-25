type Message = { id: string, channel_id: string, guild_id: string? }

local DM = 1
local GROUP_DM = 3

local pinged: { [string]: string } = {}
local account: string? = nil

local function after(id: string, other: string): boolean
	if #id ~= #other then
		return #id > #other
	end
	return id > other
end

local function limited(message: Message): boolean
	if message.guild_id then
		return kyoko.settings.get("servers") == true
	end
	local which = kyoko.settings.get("channels")
	if which ~= "direct" and which ~= "group" then
		return true
	end
	local channel = kyoko.state.channel(message.channel_id)
	local kind = if channel then channel.type else nil
	return kind == (if which == "direct" then DM else GROUP_DM)
end

local function read_since(channel_id: string, id: string): boolean
	local read = kyoko.state.read_state(channel_id)
	local last = if read then read.last_message_id else nil
	return last ~= nil and not after(id, last)
end

return {
	allow_notification = function(message: Message): boolean
		if not limited(message) then
			return true
		end
		local previous = pinged[message.channel_id]
		if previous and not read_since(message.channel_id, previous) then
			return false
		end
		pinged[message.channel_id] = message.id
		return true
	end,
	on_event = function(event: { type: string, user_id: string? })
		if event.type == "ready" then
			if event.user_id ~= account then
				pinged = {}
			end
			account = event.user_id
		end
	end,
}
