#[derive(Default)]
pub struct AppState {
    pub twitter_authorized: bool,
    pub mastodon_authorized: bool,
    pub bluesky_authorized: bool,
    pub post_text: String,
    pub bluesky_token: Option<String>,
    pub did: Option<String>,
}
