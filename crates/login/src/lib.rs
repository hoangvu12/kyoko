mod error;
pub mod persist;
mod qr;
mod token_login;
mod web_login;

pub use error::{LoginError, Result};
pub use qr::{QrConfig, QrEvent, QrLogin, QrSession, REMOTE_AUTH_URL, Scanner};
pub use token_login::TokenLogin;
pub use web_login::{CARRIED_COOKIES, DISCORD_LOGIN_URL, carried_cookie, token_from_page, token_script};
