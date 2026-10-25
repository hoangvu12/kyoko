export type Rule = { hosts: { string }, exact: { string }, prefixes: { string }, paths: { string }? }

local everywhere: Rule = {
	hosts = {},
	exact = {
		"fbclid",
		"gclid",
		"gclsrc",
		"dclid",
		"gbraid",
		"wbraid",
		"msclkid",
		"yclid",
		"twclid",
		"ttclid",
		"li_fat_id",
		"mc_cid",
		"mc_eid",
		"mkt_tok",
		"igshid",
		"igsh",
		"_ga",
		"_gl",
		"_hsenc",
		"_hsmi",
		"hsctatracking",
		"oly_anon_id",
		"oly_enc_id",
		"vero_id",
		"vero_conv",
		"_openstat",
		"rb_clickid",
		"s_cid",
		"wickedid",
		"srsltid",
	},
	prefixes = { "utm_", "__hs", "pk_campaign", "pk_kwd", "pk_source", "pk_medium" },
}

local sites: { Rule } = {
	{ hosts = { "youtube.com", "youtu.be", "music.youtube.com" }, exact = { "si", "pp" }, prefixes = {} },
	{ hosts = { "open.spotify.com", "spotify.link" }, exact = { "si", "context", "nd" }, prefixes = {} },
	{ hosts = { "twitter.com", "x.com" }, exact = { "s", "t", "ref_src", "ref_url" }, prefixes = {} },
	{ hosts = { "instagram.com" }, exact = { "img_index" }, prefixes = {} },
	{ hosts = { "facebook.com", "fb.watch" }, exact = { "mibextid", "__tn__", "rdid", "share_url" }, prefixes = { "__cft__" } },
	{
		hosts = { "tiktok.com" },
		exact = { "is_from_webapp", "sender_device", "is_copy_url", "_r", "_t", "web_id" },
		prefixes = { "share_" },
	},
	{ hosts = { "reddit.com" }, exact = { "share_id", "rdt", "correlation_id", "ref_source" }, prefixes = {} },
	{ hosts = { "linkedin.com" }, exact = { "trk", "trackingid", "lipi", "refid", "midtoken", "midsig" }, prefixes = {} },
	{
		hosts = { "amazon.*" },
		exact = { "ref", "ref_", "_encoding", "content-id", "crid", "sprefix", "qid", "dib", "dib_tag", "smid" },
		prefixes = { "pf_rd_", "pd_rd_" },
		paths = { "/ref=[^/]*" },
	},
	{ hosts = { "aliexpress.com", "aliexpress.us" }, exact = { "spm", "scm", "pvid", "gatewayadapt" }, prefixes = { "algo_" } },
	{ hosts = { "ebay.com", "ebay.co.uk", "ebay.de" }, exact = { "_trkparms", "_trksid", "itmmeta", "mkevt", "mkcid", "mkrid" }, prefixes = {} },
	{ hosts = { "google.com" }, exact = { "ved", "ei", "sca_esv", "sxsrf", "gs_lcp", "gs_lp", "uact", "sclient", "bih", "biw", "rlz" }, prefixes = {} },
	{ hosts = { "bing.com" }, exact = { "form", "cvid", "qs", "sk", "sp", "sc", "ghc", "pq" }, prefixes = {} },
	{ hosts = { "medium.com" }, exact = { "source" }, prefixes = {} },
}

local referrals: { Rule } = {
	{ hosts = { "amazon.*" }, exact = { "tag", "linkcode", "linkid", "ascsubtag", "creative", "creativeasin", "camp" }, prefixes = {} },
	{ hosts = { "aliexpress.com", "aliexpress.us" }, exact = {}, prefixes = { "aff_" } },
	{ hosts = { "ebay.com", "ebay.co.uk", "ebay.de" }, exact = { "campid", "customid", "toolid" }, prefixes = {} },
}

return {
	everywhere = everywhere,
	sites = sites,
	referrals = referrals,
}
