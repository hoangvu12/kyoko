use wreq::header::OrigHeaderMap;

pub const API_ORDER: [&str; 24] = [
	"content-length",
	"sec-ch-ua-platform",
	"authorization",
	"x-installation-id",
	"x-debug-options",
	"sec-ch-ua",
	"sec-ch-ua-mobile",
	"x-discord-timezone",
	"x-context-properties",
	"x-super-properties",
	"x-audit-log-reason",
	"x-discord-locale",
	"user-agent",
	"content-type",
	"accept",
	"origin",
	"sec-fetch-site",
	"sec-fetch-mode",
	"sec-fetch-dest",
	"referer",
	"accept-encoding",
	"accept-language",
	"cookie",
	"priority",
];

pub const IMAGE_ORDER: [&str; 14] = [
	"sec-ch-ua-platform",
	"user-agent",
	"sec-ch-ua",
	"sec-ch-ua-mobile",
	"accept",
	"sec-fetch-site",
	"sec-fetch-mode",
	"sec-fetch-dest",
	"sec-fetch-storage-access",
	"referer",
	"accept-encoding",
	"accept-language",
	"range",
	"priority",
];

pub const UPGRADE_ORDER: [&str; 11] = [
	"Host",
	"Connection",
	"Pragma",
	"Cache-Control",
	"User-Agent",
	"Upgrade",
	"Origin",
	"Sec-WebSocket-Version",
	"Accept-Encoding",
	"Accept-Language",
	"Sec-WebSocket-Key",
];

pub(crate) fn orig_headers(order: &[&'static str]) -> OrigHeaderMap {
	let mut map = OrigHeaderMap::with_capacity(order.len());
	for name in order {
		map.insert(*name);
	}
	map
}
