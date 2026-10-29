use protocol::{Embed, EmbedAuthor, EmbedKind, EmbedMedia, EmbedProvider};

pub const YOUTUBE_LINK: &str = "https://www.youtube.com/watch?v=jNQXAC9IVRw&t=5s";
pub const VIMEO_LINK: &str = "https://vimeo.com/76979871";
pub const SPOTIFY_LINK: &str = "https://open.spotify.com/track/4cOdK2wGLETKBW3PvgPWqT";
pub const SOUNDCLOUD_LINK: &str = "https://soundcloud.com/forss/flickermood";
pub const LINKS: [&str; 4] = [YOUTUBE_LINK, VIMEO_LINK, SPOTIFY_LINK, SOUNDCLOUD_LINK];
const SOUNDCLOUD_TRACK: u64 = 293;
const PROXY: &str = "https://images-ext-1.discordapp.net/external/mock/https/";

fn media(url: &str, width: i64, height: i64) -> EmbedMedia {
	EmbedMedia {
		url: Some(url.to_owned()),
		proxy_url: url.strip_prefix("https://").map(|rest| format!("{PROXY}{rest}")),
		width: Some(width),
		height: Some(height),
		..EmbedMedia::default()
	}
}

fn provider(name: &str, url: &str) -> Option<EmbedProvider> {
	Some(EmbedProvider {
		name: Some(name.to_owned()),
		url: Some(url.to_owned()),
	})
}

pub fn embed(link: &str) -> Option<Embed> {
	let rest = link.strip_prefix("https://")?;
	let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
	let host = host.trim_start_matches("www.");
	let path = path.split(['?', '#']).next().unwrap_or_default();
	let query = rest.split_once('?').map(|(_, query)| query).unwrap_or_default();
	match host {
		"youtube.com" | "youtu.be" => {
			let id = if host == "youtu.be" {
				path.to_owned()
			} else {
				query
					.split('&')
					.find_map(|pair| pair.strip_prefix("v="))?
					.to_owned()
			};
			Some(Embed {
				kind: Some(EmbedKind::Video),
				title: Some("Me at the zoo".to_owned()),
				url: Some(link.to_owned()),
				color: Some(0xFF0000),
				provider: provider("YouTube", "https://www.youtube.com"),
				author: Some(EmbedAuthor {
					name: "jawed".to_owned(),
					url: Some("https://www.youtube.com/channel/UC4QobU6STFB0P71PMvOGN5A".to_owned()),
					..EmbedAuthor::default()
				}),
				thumbnail: Some(media(
					&format!("https://i.ytimg.com/vi/{id}/hqdefault.jpg"),
					480,
					360,
				)),
				video: Some(media(&format!("https://www.youtube.com/embed/{id}"), 1280, 720)),
				..Embed::default()
			})
		}
		"vimeo.com" => Some(Embed {
			kind: Some(EmbedKind::Video),
			title: Some("The New Vimeo Player (You Know, For Videos)".to_owned()),
			description: Some(
				"It may look (mostly) the same on the surface, but under the hood it is all new.".to_owned(),
			),
			url: Some(link.to_owned()),
			color: Some(0x00ADEF),
			provider: provider("Vimeo", "https://vimeo.com/"),
			author: Some(EmbedAuthor {
				name: "Vimeo".to_owned(),
				url: Some("https://vimeo.com/staff".to_owned()),
				..EmbedAuthor::default()
			}),
			thumbnail: Some(media(
				&format!("https://i.vimeocdn.com/video/{path}-d_1280x720"),
				1280,
				720,
			)),
			video: Some(media(
				&format!("https://player.vimeo.com/video/{path}"),
				1280,
				720,
			)),
			..Embed::default()
		}),
		"open.spotify.com" => Some(Embed {
			kind: Some(EmbedKind::Link),
			title: Some("Never Gonna Give You Up".to_owned()),
			description: Some("Rick Astley \u{b7} Song \u{b7} 1987".to_owned()),
			url: Some(link.to_owned()),
			color: Some(0x1ED760),
			provider: provider("Spotify", "https://open.spotify.com"),
			thumbnail: Some(media("https://i.scdn.co/image/ab67616d0000b273mock", 640, 640)),
			..Embed::default()
		}),
		"soundcloud.com" => Some(Embed {
			kind: Some(EmbedKind::Video),
			title: Some("Flickermood by Forss".to_owned()),
			description: Some("From the Soulhack album".to_owned()),
			url: Some(link.to_owned()),
			color: Some(0xFF5500),
			provider: provider("SoundCloud", "https://soundcloud.com"),
			author: Some(EmbedAuthor {
				name: "Forss".to_owned(),
				url: Some("https://soundcloud.com/forss".to_owned()),
				..EmbedAuthor::default()
			}),
			thumbnail: Some(media(
				"https://i1.sndcdn.com/artworks-mock-t500x500.jpg",
				500,
				500,
			)),
			video: Some(media(
				&format!(
					"https://w.soundcloud.com/player/?url=https%3A%2F%2Fapi.soundcloud.com%2Ftracks%2F{SOUNDCLOUD_TRACK}&auto_play=false&show_artwork=true"
				),
				435,
				400,
			)),
			..Embed::default()
		}),
		_ => None,
	}
}
