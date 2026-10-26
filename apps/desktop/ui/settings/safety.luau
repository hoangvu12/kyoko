local theme = require("../theme")
local state = require("../state")
local kit = require("./kit")
local icon_button = require("@components/icon_button")

local function hosts(): { string }
	local found = (state.settings.values:get() or {})["trusted_link_hosts"]
	return if type(found) == "table" then found else {}
end

local function forget(host: string)
	local kept = {}
	for _, item in hosts() do
		if item ~= host then
			table.insert(kept, item)
		end
	end
	kit.set("trusted_link_hosts", kept)
end

local function trusted_list()
	local list = ui.computed(function()
		return hosts()
	end)
	return ui.column {
		w_full = true,
		gap = theme.size.space_8,
		kit.label("Trusted Sites", "Links to these sites open without asking."),
		ui.show(function()
			return #hosts() == 0
		end, kit.hint("No trusted sites yet. Choose \"Always trust\" when you open a link.")),
		ui.each(list, function(host: string)
			return ui.row {
				w_full = true,
				items = "center",
				gap = theme.size.space_12,
				px = theme.size.space_12,
				py = theme.size.space_4,
				rounded = theme.size.radius_sm,
				bg = theme.color.bg_3,
				kit.text(host, theme.size.font_size_md, theme.size.line_height_label, theme.color.text_3, {
					grow = 1,
					min_w = 0,
					truncate = true,
				}),
				icon_button {
					name = "close",
					tooltip = "Stop trusting " .. host,
					on_click = function()
						forget(host)
					end,
				},
			}
		end, { w_full = true, gap = theme.size.space_8 }),
	}
end

return {
	headings = {
		{
			key = "safety-locks",
			title = "Safety Locks",
			rows = {
				{
					key = "safety_locks",
					title = "Hold risky actions",
					note = "Kyoko holds actions that look automated to Discord, such as messaging strangers or posting in empty channels, until you confirm. Turning this off is at your own risk. Applies after a restart.",
					keywords = "safety lock automation confirm",
				},
			},
		},
		{
			key = "links",
			title = "Links",
			rows = {
				{
					key = "chat.link_warning",
					title = "Warn before opening links",
					note = "Ask before opening a link to a site you have not trusted.",
					keywords = "url browser external",
				},
				{
					kind = "block",
					title = "Trusted Sites",
					keywords = "trust domains hosts allow",
					build = trusted_list,
				},
			},
		},
		{
			key = "gif-usage",
			title = "GIF Usage",
			rows = {
				{
					key = "advanced.share_gif_usage",
					title = "Tell Discord which GIFs I pick",
					note = "Sends the GIF you choose back to Discord to improve search results. Off by default.",
					keywords = "gif search privacy analytics",
				},
			},
		},
	},
}
