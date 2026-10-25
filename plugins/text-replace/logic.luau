type Rule = { find: any, replace: any, only_if: any, pattern: any }

local function text_of(value: any): string
	return if type(value) == "string" then value else ""
end

local function replace_plain(text: string, find: string, replacement: string): string
	local parts = {}
	local position = 1
	while true do
		local start, finish = string.find(text, find, position, true)
		if not start then
			break
		end
		table.insert(parts, string.sub(text, position, start - 1))
		table.insert(parts, replacement)
		position = finish + 1
	end
	table.insert(parts, string.sub(text, position))
	return table.concat(parts)
end

local function apply(rule: Rule, text: string): string
	local find = text_of(rule.find)
	if find == "" then
		return text
	end
	local only_if = text_of(rule.only_if)
	if only_if ~= "" and not string.find(text, only_if, 1, true) then
		return text
	end
	local replacement = text_of(rule.replace)
	if rule.pattern == true then
		return (string.gsub(text, find, replacement))
	end
	return replace_plain(text, find, replacement)
end

local function rewrite(message: { content: string }): string?
	local rules = kyoko.settings.get("rules")
	if type(rules) ~= "table" then
		return nil
	end
	local text = message.content
	for _, rule in rules do
		if type(rule) == "table" then
			local applied, result = pcall(apply, rule, text)
			if applied and type(result) == "string" then
				text = result
			end
		end
	end
	return if text ~= message.content then text else nil
end

return {
	before_send = rewrite,
	before_edit = rewrite,
}
