local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local kit = require("./kit")

local NAME_LIMIT = 100
local TOPIC_LIMIT = 1024
local TOPIC_LINES = 3
local QUALITY_AUTO = 1
local QUALITY_FULL = 2

local name = ui.state("channel_name", "")
local topic = ui.state("channel_topic", "")
local slowmode = ui.state("channel_slowmode", 1)
local visibility = ui.state("channel_visibility", "default")
local nsfw = ui.state("channel_nsfw", false)
local announcement = ui.state("channel_announcement", false)
local archive = ui.state("channel_archive", 1)
local bitrate = ui.state("channel_bitrate", 64)
local user_limit = ui.state("channel_user_limit", 0)
local video_quality = ui.state("channel_video_quality", QUALITY_AUTO)
local region = ui.state("channel_region", 1)
local category = ui.state("channel_category", 0)

local function saved(): state.ChannelOverview?
	return state.channel.overview:get()
end

local function trimmed(text: string): string
	return (string.gsub(text, "^%s*(.-)%s*$", "%1"))
end

local function wanted_name(): string
	return string.sub(trimmed(name:get()), 1, NAME_LIMIT)
end

local function wanted_topic(): string
	return string.sub(trimmed(topic:get()), 1, TOPIC_LIMIT)
end

local function load(current: state.ChannelOverview)
	name:set(current.name)
	topic:set(current.topic)
	slowmode:set(current.slowmode)
	visibility:set(current.visibility)
	nsfw:set(current.nsfw)
	announcement:set(current.announcement)
	archive:set(current.archive)
	bitrate:set(current.bitrate)
	user_limit:set(current.user_limit)
	video_quality:set(current.video_quality)
	region:set(current.region)
	category:set(current.category)
end

local function changes(): { [string]: any }
	local current = saved()
	local found: { [string]: any } = {}
	if not current then
		return found
	end
	local renamed = wanted_name()
	if renamed ~= "" and renamed ~= current.name then
		found.name = renamed
	end
	if current.has_topic and wanted_topic() ~= current.topic then
		found.topic = wanted_topic()
	end
	if slowmode:get() ~= current.slowmode then
		found.slowmode = slowmode:get()
	end
	if current.audio then
		if visibility:get() ~= current.visibility then
			found.visibility = visibility:get()
		end
		if bitrate:get() ~= current.bitrate then
			found.bitrate = bitrate:get()
		end
		if user_limit:get() ~= current.user_limit then
			found.user_limit = user_limit:get()
		end
		if video_quality:get() ~= current.video_quality then
			found.video_quality = video_quality:get()
		end
		if region:get() ~= current.region then
			found.region = region:get()
		end
	elseif nsfw:get() ~= current.nsfw then
		found.nsfw = nsfw:get()
	end
	if current.announces and announcement:get() ~= current.announcement then
		found.announcement = announcement:get()
	end
	if current.archives and archive:get() ~= current.archive then
		found.archive = archive:get()
	end
	if category:get() ~= current.category and category:get() > 0 then
		found.category = category:get()
	end
	return found
end

local function dirty(): boolean
	return next(changes()) ~= nil
end

local function reset()
	local current = saved()
	if current then
		load(current)
	end
end

local function save()
	local found = changes()
	if next(found) ~= nil then
		actions.channel_action("save", found)
	end
end

local function signature(current: state.ChannelOverview): string
	return table.concat({
		current.id,
		current.name,
		current.topic,
		tostring(current.slowmode),
		current.visibility,
		tostring(current.nsfw),
		tostring(current.announcement),
		tostring(current.archive),
		tostring(current.bitrate),
		tostring(current.user_limit),
		tostring(current.video_quality),
		tostring(current.region),
		tostring(current.category),
		tostring(#current.regions),
	}, "|")
end

local function follow_saved()
	local seen = ""
	ui.effect(function()
		local current = saved()
		if current then
			local now = signature(current)
			if now ~= seen then
				seen = now
				load(current)
			end
		end
	end)
end

local function block(title: string, content: any, note: any?)
	return ui.column {
		w_full = true,
		gap = theme.size.space_8,
		kit.caption(title),
		content,
		if note then kit.hint(note) else nil,
	}
end

local function switch_line(id: string, title: string, note: string, value: any)
	return ui.div {
		id = id,
		w_full = true,
		kit.line(
			title,
			note,
			kit.switch {
				label = title,
				checked = function()
					return value:get()
				end,
				on_change = function(checked: boolean)
					value:set(checked)
				end,
			}
		),
	}
end

local function choices(key: string): { string }
	local all = state.channel.choices:get() :: any
	return all[key] or {}
end

local function select(id: string, labels: { string }, value: any)
	return kit.select {
		id = id,
		labels = labels,
		current = function()
			return value:get()
		end,
		pick = function(index: number)
			value:set(index)
		end,
	}
end

local function name_block()
	return block(
		"Channel Name",
		kit.input(name, "", { id = "channel_overview_name", label = "Channel Name" })
	)
end

local function topic_block()
	return block(
		"Channel Topic",
		kit.textarea(topic, "Let everyone know how to use this channel!", TOPIC_LINES, {
			id = "channel_overview_topic",
			label = "Channel Topic",
		})
	)
end

local function slowmode_block()
	return block(
		"Slowmode",
		select("channel_overview_slowmode", choices("slowmode"), slowmode),
		"Members will be restricted to sending one message and creating one thread per this interval, unless they have the Bypass Slowmode permission."
	)
end

local function visibility_block()
	return block(
		"Content Visibility",
		kit.radios({
			{ label = "Default", value = "default", note = "Channel content is always visible." },
			{
				label = "Spoiler Channel",
				value = "spoiler",
				note = "Mark this channel as containing spoilers, so that plot twists, heavy topics, or other sensitive discussions are hidden until users choose to view.",
			},
			{
				label = "Age-Restricted Channel",
				value = "age",
				note = "Users will need to confirm they are of over the legal age to view in the content in this channel. Age-restricted channels are exempt from the explicit content filter.",
			},
		}, function()
			return visibility:get()
		end, function(value: any)
			visibility:set(value)
		end, "channel_visibility")
	)
end

local function bitrate_block(current: state.ChannelOverview)
	return block(
		"Bitrate",
		kit.range {
			id = "channel_overview_bitrate",
			label = "Bitrate",
			value = bitrate,
			min = current.bitrate_min,
			max = current.bitrate_max,
			marks = {
				tostring(current.bitrate_min) .. "kbps",
				tostring(current.bitrate_max) .. "kbps",
			},
		},
		function()
			return "ALL THE BITS! Going above 64 kbps may adversely affect people on poor connections. Now "
				.. tostring(bitrate:get())
				.. " kbps."
		end
	)
end

local function quality_block()
	return block(
		"Video Quality",
		kit.radios({
			{ label = "Auto", value = QUALITY_AUTO },
			{ label = "720p", value = QUALITY_FULL },
		}, function()
			return video_quality:get()
		end, function(value: any)
			video_quality:set(value)
		end, "channel_video_quality"),
		"Sets camera video quality for all channel participants. Choose Auto for optimal performance."
	)
end

local function limit_block(current: state.ChannelOverview)
	return block(
		"User Limit",
		kit.range {
			id = "channel_overview_user_limit",
			label = "User Limit",
			value = user_limit,
			min = 0,
			max = current.user_limit_max,
			marks = { "∞", tostring(current.user_limit_max) },
		},
		function()
			local limit = user_limit:get()
			local shown = if limit == 0 then "no limit" else tostring(limit) .. " users"
			return "Limits the number of users that can connect to this voice channel: " .. shown .. "."
		end
	)
end

local function region_block(current: state.ChannelOverview)
	return block(
		"Region Override",
		select("channel_overview_region", current.regions, region),
		"Everyone in this channel will connect to this region. Automatic picks the best region when someone joins."
	)
end

local function archive_block()
	return block(
		"Hide After Inactivity",
		select("channel_overview_archive", choices("archive"), archive),
		"New threads will stop showing in the channel list after this period of inactivity."
	)
end

local function category_block(current: state.ChannelOverview)
	local labels = { "No Category" }
	for _, title in current.categories do
		table.insert(labels, title)
	end
	return block("Category", kit.select {
		id = "channel_overview_category",
		labels = labels,
		current = function()
			return category:get() + 1
		end,
		pick = function(index: number)
			category:set(index - 1)
		end,
	})
end

local function form(current: state.ChannelOverview)
	local parts: { any } = { kit.title("Overview"), name_block() }
	if current.has_topic then
		table.insert(parts, topic_block())
	end
	table.insert(parts, slowmode_block())
	if current.audio then
		table.insert(parts, visibility_block())
	else
		table.insert(
			parts,
			switch_line(
				"channel_overview_nsfw",
				"Age-Restricted Channel",
				"Users will need to confirm they are of over the legal age to view the content in this channel. Age-restricted channels are exempt from the explicit content filter.",
				nsfw
			)
		)
	end
	if current.announces then
		table.insert(
			parts,
			switch_line(
				"channel_overview_announcement",
				"Announcement Channel",
				"Post messages that reach servers outside your own. Users can opt in to follow this channel, so select posts they publish here will appear directly in their own servers.",
				announcement
			)
		)
	end
	table.insert(parts, kit.divider())
	if current.audio then
		table.insert(parts, bitrate_block(current))
		table.insert(parts, quality_block())
		table.insert(parts, limit_block(current))
		table.insert(parts, region_block(current))
	end
	if current.archives then
		table.insert(parts, archive_block())
	end
	if #current.categories > 0 then
		table.insert(parts, category_block(current))
	end
	return ui.column {
		id = "channel_overview",
		w_full = true,
		gap = theme.size.space_24,
		table.unpack(parts),
	}
end

local function build()
	follow_saved()
	return ui.presence(function()
		local current = saved()
		return if current then current.id .. "|" .. tostring(current.audio) .. "|" .. tostring(#current.regions) else nil
	end, function()
		local current = saved()
		return if current then form(current) else ui.div {}
	end)
end

return {
	keywords = "channel overview name topic slowmode bitrate region",
	build = build,
	save_bar = function()
		return ui.show(dirty, kit.save_bar(reset, save))
	end,
}
