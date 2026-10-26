export type Row = {
	title: string?,
	note: any?,
	keywords: string?,
}

export type Heading = {
	key: string,
	title: string?,
	keywords: string?,
	rows: { Row }?,
}

export type Searchable = {
	title: string,
	keywords: (string | () -> string)?,
	headings: { Heading }?,
}

local M = {}

M.query = ui.state("query", "")

local function words(query: string): { string }
	local found = {}
	for word in string.gmatch(string.lower(query), "%S+") do
		table.insert(found, word)
	end
	return found
end

local function has_all(text: string, wanted: { string }): boolean
	local lowered = string.lower(text)
	for _, word in wanted do
		if not string.find(lowered, word, 1, true) then
			return false
		end
	end
	return true
end

local function text_of(value: any): string
	if type(value) == "string" then
		return value
	elseif type(value) == "function" then
		local ok, found = pcall(value)
		return if ok and type(found) == "string" then found else ""
	end
	return ""
end

function M.active(): boolean
	return #words(M.query:get()) > 0
end

function M.row_matches(row: Row, query: string): boolean
	local wanted = words(query)
	if #wanted == 0 then
		return false
	end
	return has_all(table.concat({ row.title or "", text_of(row.note), row.keywords or "" }, " "), wanted)
end

function M.heading_matches(heading: Heading, query: string): boolean
	local wanted = words(query)
	if #wanted == 0 then
		return false
	end
	if has_all((heading.title or "") .. " " .. (heading.keywords or ""), wanted) then
		return true
	end
	for _, row in heading.rows or {} do
		if M.row_matches(row, query) then
			return true
		end
	end
	return false
end

function M.page_matches(page: Searchable, query: string): boolean
	local wanted = words(query)
	if #wanted == 0 then
		return true
	end
	if has_all(page.title .. " " .. text_of(page.keywords), wanted) then
		return true
	end
	for _, heading in page.headings or {} do
		if M.heading_matches(heading, query) then
			return true
		end
	end
	return false
end

function M.highlighted(row: Row): () -> boolean
	return function()
		return M.row_matches(row, M.query:get())
	end
end

return M
