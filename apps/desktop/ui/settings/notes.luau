local kit = require("./kit")

local M = {}

function M.page(key: string, title: string, note: string, keywords: string)
	return {
		headings = {
			{
				key = key,
				title = title,
				keywords = keywords,
				rows = { kit.web_note(note) },
			},
		},
	}
end

return M
