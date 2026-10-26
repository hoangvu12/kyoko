type Emoji = { key: string, favorite: number? }
type Request = { query: string, emojis: { Emoji } }

local function favorites_first(request: Request): { string }
	local favorites: { Emoji } = {}
	for _, emoji in request.emojis do
		if emoji.favorite then
			table.insert(favorites, emoji)
		end
	end
	table.sort(favorites, function(left: Emoji, right: Emoji): boolean
		return (left.favorite :: number) < (right.favorite :: number)
	end)
	local order: { string } = {}
	for _, emoji in favorites do
		table.insert(order, emoji.key)
	end
	return order
end

return {
	rank_emoji = favorites_first,
}
