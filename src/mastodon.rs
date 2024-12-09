use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

const TOKEN_FILE: &str = "mastodon_tokens.json";
const API_BASE_URL: &str = "https://fosstodon.org";
const OAUTH_BASE_URL: &str = "https://fosstodon.org/oauth";

#[derive(Serialize, Deserialize)]
pub struct TokenData {
    pub access_token: String,
}

pub fn save_tokens(access_token: &str) {
    let token_data = TokenData {
        access_token: access_token.to_string(),
    };
    let json = serde_json::to_string(&token_data).expect("Failed to serialize token data");
    fs::write(TOKEN_FILE, json).expect("Failed to write token file");
}

pub fn load_tokens() -> Option<TokenData> {
    if Path::new(TOKEN_FILE).exists() {
        let json = fs::read_to_string(TOKEN_FILE).expect("Failed to read token file");
        serde_json::from_str(&json).ok()
    } else {
        None
    }
}

/// Generates the Mastodon OAuth 2.0 authorization URL.
pub async fn generate_auth_url(client_id: &str) -> String {
    format!(
        "{}/authorize?response_type=code&client_id={}&redirect_uri=urn:ietf:wg:oauth:2.0:oob&scope=write:statuses",
        OAUTH_BASE_URL, client_id
    )
}

/// Exchanges an authorization code for an access token.
pub async fn authorize_mastodon(client_id: &str, client_secret: &str, code: &str) -> Option<String> {
    #[derive(Serialize)]
    struct TokenRequest {
        grant_type: String,
        client_id: String,
        client_secret: String,
        redirect_uri: String,
        code: String,
    }

    #[derive(Deserialize)]
    struct TokenResponse {
        access_token: String,
    }

    let client = Client::new();
    let token_request = TokenRequest {
        grant_type: "authorization_code".to_string(),
        client_id: client_id.to_string(),
        client_secret: client_secret.to_string(),
        redirect_uri: "urn:ietf:wg:oauth:2.0:oob".to_string(),
        code: code.to_string(),
    };

    match client
        .post(format!("{}/token", OAUTH_BASE_URL))
        .form(&token_request)
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                if let Ok(token_response) = response.json::<TokenResponse>().await {
                    Some(token_response.access_token)
                } else {
                    println!("Failed to parse token response.");
                    None
                }
            } else {
                println!(
                    "Failed to authorize Mastodon: {}",
                    response.text().await.unwrap_or_default()
                );
                None
            }
        }
        Err(err) => {
            println!("Error sending request: {:?}", err);
            None
        }
    }
}

/// Posts a status (toot) to Mastodon.
pub async fn post_to_mastodon(token: &str, status: &str) -> bool {
    #[derive(Serialize)]
    struct StatusPost {
        status: String,
    }

    let client = Client::new();
    let post_data = StatusPost {
        status: status.to_string(),
    };

    match client
        .post(format!("{}/api/v1/statuses", API_BASE_URL))
        .bearer_auth(token)
        .json(&post_data)
        .send()
        .await
    {
        Ok(response) => {
            let status_code = response.status();
            let body = response.text().await.unwrap_or_default();
            if status_code.is_success() {
                println!("Posted to Mastodon successfully!");
                true
            } else {
                println!("Failed to post to Mastodon: {}", body);
                false
            }
        }
        Err(err) => {
            println!("Error posting to Mastodon: {:?}", err);
            false
        }
    }
}
