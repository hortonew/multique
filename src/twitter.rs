use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use url::Url;

const TOKEN_FILE: &str = "twitter_tokens.json";

use crate::posts;

#[derive(Serialize, Deserialize)]
pub struct TokenData {
    pub access_token: String,
    pub refresh_token: Option<String>,
}

pub fn save_tokens(access_token: &str, refresh_token: Option<&str>) {
    let token_data = TokenData {
        access_token: access_token.to_string(),
        refresh_token: refresh_token.map(|rt| rt.to_string()),
    };
    let json = serde_json::to_string(&token_data).expect("Failed to serialize token data");
    fs::write(TOKEN_FILE, json).expect("Failed to write token file");
}

pub fn load_bearer_token() -> Option<String> {
    if let Some(tokens) = load_tokens() {
        Some(tokens.access_token)
    } else {
        None
    }
}

fn load_tokens() -> Option<TokenData> {
    if Path::new(TOKEN_FILE).exists() {
        let json = fs::read_to_string(TOKEN_FILE).expect("Failed to read token file");
        serde_json::from_str(&json).ok()
    } else {
        None
    }
}

/// Generates the Twitter OAuth 2.0 authorization URL.
pub async fn generate_auth_url() -> Option<String> {
    let client_id = env::var("TWITTER_CLIENT_ID").expect("TWITTER_CLIENT_ID not set");
    let redirect_uri = env::var("TWITTER_REDIRECT_URI").expect("TWITTER_REDIRECT_URI not set");

    let mut url = Url::parse("https://twitter.com/i/oauth2/authorize").unwrap();
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", &client_id)
        .append_pair("redirect_uri", &redirect_uri)
        .append_pair("scope", "tweet.read tweet.write users.read")
        .append_pair("state", "state")
        .append_pair("code_challenge", "challenge")
        .append_pair("code_challenge_method", "plain");

    Some(url.to_string())
}

/// Authorizes Twitter using the provided authorization code and saves the tokens.
pub async fn authorize_twitter(
    state: Arc<Mutex<posts::AppState>>,
    authorization_code: &str,
) -> Option<String> {
    #[derive(Serialize)]
    struct TokenRequest {
        code: String,
        grant_type: String,
        client_id: String,
        redirect_uri: String,
        code_verifier: String,
    }

    #[derive(Deserialize)]
    struct TokenResponse {
        access_token: String,
        refresh_token: Option<String>,
    }

    let client_id = env::var("TWITTER_CLIENT_ID").expect("TWITTER_CLIENT_ID not set");
    let redirect_uri = env::var("TWITTER_REDIRECT_URI").expect("TWITTER_REDIRECT_URI not set");

    let client = Client::new();
    let token_request = TokenRequest {
        code: authorization_code.to_string(),
        grant_type: "authorization_code".to_string(),
        client_id: client_id.clone(),
        redirect_uri: redirect_uri.clone(),
        code_verifier: "challenge".to_string(), // This must match the `code_challenge` value in `generate_auth_url`.
    };

    match client
        .post("https://api.twitter.com/2/oauth2/token")
        .form(&token_request)
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                if let Ok(token_response) = response.json::<TokenResponse>().await {
                    let access_token = token_response.access_token.clone();
                    let refresh_token = token_response.refresh_token.clone();

                    // Save the tokens locally
                    save_tokens(&access_token, refresh_token.as_deref());

                    // Update the AppState
                    let mut state_guard = state.lock().await;
                    state_guard.twitter_authorized = true;

                    Some(access_token)
                } else {
                    println!("Failed to parse token response.");
                    None
                }
            } else {
                println!("Failed to authorize Twitter: {:?}", response.text().await);
                None
            }
        }
        Err(err) => {
            println!("Error sending request: {:?}", err);
            None
        }
    }
}

pub async fn post_to_twitter(token: &str, text: &str) -> bool {
    #[derive(Serialize)]
    struct TwitterPost {
        text: String,
    }

    let client = Client::new();
    let post_data = TwitterPost {
        text: text.to_string(),
    };

    match client
        .post("https://api.twitter.com/2/tweets")
        .bearer_auth(token)
        .json(&post_data)
        .send()
        .await
    {
        Ok(response) => {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            if status.is_success() {
                true
            } else {
                println!("Failed to post to Twitter: {}", body);
                false
            }
        }
        Err(err) => {
            println!("Error posting to Twitter: {:?}", err);
            false
        }
    }
}
