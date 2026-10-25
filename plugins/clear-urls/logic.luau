local rules = require("./rules")

local TRAILING = "[%.,;:!%?'\"]"

local function host_matches(host: string, pattern: string): boolean
	if string.sub(pattern, -2) == ".*" then
		local name = string.sub(pattern, 1, -3)
		local label = string.match(host, "^(.-)%.[%a]+$") or ""
		label = string.match(label, "^(.-)%.co$") or string.match(label, "^(.-)%.com$") or label
		return label == name or string.sub(label, -(#name + 1)) == "." .. name
	end
	return host == pattern or string.sub(host, -(#pattern + 1)) == "." .. pattern
end

local function applies(rule: rules.Rule, host: string): boolean
	if #rule.hosts == 0 then
		return true
	end
	for _, pattern in rule.hosts do
		if host_matches(host, pattern) then
			return true
		end
	end
	return false
end

local function listed(rule: rules.Rule, key: string): boolean
	for _, name in rule.exact do
		if key == name then
			return true
		end
	end
	for _, prefix in rule.prefixes do
		if string.sub(key, 1, #prefix) == prefix then
			return true
		end
	end
	return false
end

local function active_rules(host: string, referrals: boolean): { rules.Rule }
	local found = { rules.everywhere }
	for _, rule in rules.sites do
		if applies(rule, host) then
			table.insert(found, rule)
		end
	end
	if referrals then
		for _, rule in rules.referrals do
			if applies(rule, host) then
				table.insert(found, rule)
			end
		end
	end
	return found
end

local function removable(active: { rules.Rule }, key: string): boolean
	local lowered = string.lower(key)
	for _, rule in active do
		if listed(rule, lowered) then
			return true
		end
	end
	return false
end

local function clean_path(active: { rules.Rule }, path: string): string
	for _, rule in active do
		for _, pattern in rule.paths or {} do
			path = string.gsub(path, pattern, "")
		end
	end
	return path
end

local function clean_url(url: string, referrals: boolean): string
	local scheme, authority, rest = string.match(url, "^(https?://)([^/%?#]+)(.*)$")
	if not scheme then
		return url
	end
	local host = string.lower(string.gsub(string.gsub(authority, "^.*@", ""), ":%d+$", ""))
	local active = active_rules(host, referrals)
	local path = string.match(rest, "^[^%?#]*") or ""
	local after = string.sub(rest, #path + 1)
	local query = ""
	local fragment = ""
	if string.sub(after, 1, 1) == "?" then
		query = string.match(after, "^%?([^#]*)") or ""
		fragment = string.sub(after, #query + 2)
	else
		fragment = after
	end
	local kept = {}
	for pair in string.gmatch(query, "[^&]+") do
		local key = string.match(pair, "^([^=]*)") or pair
		if not removable(active, key) then
			table.insert(kept, pair)
		end
	end
	local cleaned_path = clean_path(active, path)
	if cleaned_path == "" and path ~= "" then
		cleaned_path = "/"
	end
	local cleaned = scheme .. authority .. cleaned_path
	if #kept > 0 then
		cleaned ..= "?" .. table.concat(kept, "&")
	end
	return cleaned .. fragment
end

local function split_trailing(url: string): (string, string)
	local tail = ""
	while #url > 0 do
		local last = string.sub(url, -1)
		local opens = select(2, string.gsub(url, "%(", ""))
		local closes = select(2, string.gsub(url, "%)", ""))
		if string.match(last, TRAILING) or (last == ")" and closes > opens) then
			tail = last .. tail
			url = string.sub(url, 1, -2)
		else
			break
		end
	end
	return url, tail
end

local function clean_prose(text: string, referrals: boolean): string
	return (string.gsub(text, "https?://[^%s<>]+", function(found: string): string
		local url, tail = split_trailing(found)
		return clean_url(url, referrals) .. tail
	end))
end

local function clean_text(text: string, referrals: boolean): string
	local parts = {}
	local position = 1
	while position <= #text do
		local start = string.find(text, "`", position, true)
		if not start then
			table.insert(parts, clean_prose(string.sub(text, position), referrals))
			break
		end
		table.insert(parts, clean_prose(string.sub(text, position, start - 1), referrals))
		local fence = if string.sub(text, start, start + 2) == "```" then "```" else "`"
		local finish = string.find(text, fence, start + #fence, true)
		if not finish then
			table.insert(parts, string.sub(text, start))
			break
		end
		table.insert(parts, string.sub(text, start, finish + #fence - 1))
		position = finish + #fence
	end
	return table.concat(parts)
end

local function rewrite(message: { content: string }): string?
	local referrals = kyoko.settings.get("referrals") ~= false
	local cleaned = clean_text(message.content, referrals)
	return if cleaned ~= message.content then cleaned else nil
end

return {
	before_send = rewrite,
	before_edit = rewrite,
}
