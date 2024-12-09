use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

const TOKEN_FILE: &str = "bluesky_tokens.json";
use crate::posts;

#[derive(Serialize, Deserialize)]
pub struct TokenData {
    pub access_jwt: String,
    pub refresh_jwt: String,
    pub did: String,
}

pub fn save_tokens(access_jwt: &str, refresh_jwt: &str, did: &str) {
    let token_data = TokenData {
        access_jwt: access_jwt.to_string(),
        refresh_jwt: refresh_jwt.to_string(),
        did: did.to_string(),
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

pub async fn refresh_access_token(refresh_jwt: &str) -> Option<String> {
    #[derive(Serialize)]
    struct RefreshRequest {
        refreshJwt: String,
    }

    #[derive(Deserialize)]
    struct RefreshResponse {
        accessJwt: String,
    }

    let client = Client::new();
    let refresh_data = RefreshRequest {
        refreshJwt: refresh_jwt.to_string(),
    };

    match client
        .post("https://bsky.social/xrpc/com.atproto.server.refreshSession")
        .json(&refresh_data)
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                let refresh_response: RefreshResponse = response.json().await.ok()?;
                Some(refresh_response.accessJwt)
            } else {
                println!("Failed to refresh access token");
                None
            }
        }
        Err(err) => {
            println!("Error refreshing token: {:?}", err);
            None
        }
    }
}

pub async fn authorize_bluesky(state: Arc<Mutex<posts::AppState>>) -> Option<String> {
    #[derive(Serialize)]
    struct BlueskyAuthRequest {
        identifier: String,
        password: String,
    }

    #[derive(Deserialize, Debug, Clone)]
    #[serde(rename_all = "camelCase")]
    struct BlueskyAuthResponse {
        access_jwt: String,
        refresh_jwt: String,
        handle: String,
        email: String,
        did: String,
    }

    let client = Client::new();
    let auth_data = BlueskyAuthRequest {
        identifier: env::var("BLUESKY_USERNAME").unwrap_or_else(|_| "invalid_username".to_string()),
        password: env::var("BLUESKY_PASSWORD").unwrap_or_else(|_| "invalid_password".to_string()),
    };

    match client
        .post("https://bsky.social/xrpc/com.atproto.server.createSession")
        .json(&auth_data)
        .send()
        .await
    {
        Ok(response) => {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            // println!("Response Status: {}", status);
            // println!("Response Body: {}", body);

            if status.is_success() {
                match serde_json::from_str::<BlueskyAuthResponse>(&body) {
                    Ok(auth_response) => {
                        println!("Successfully authenticated as {}", auth_response.handle);

                        // Clone required values to avoid borrowing issues
                        let token = auth_response.access_jwt.clone();
                        let refresh_token = auth_response.refresh_jwt.clone();
                        let did = auth_response.did.clone();

                        // Update the AppState
                        let mut state = state.lock().await;
                        state.bluesky_token = Some(token.clone());
                        state.did = Some(did.clone());

                        // Save tokens locally
                        save_tokens(&token, &refresh_token, &did);

                        Some(token)
                    }
                    Err(err) => {
                        println!("Deserialization error: {:?}", err);
                        None
                    }
                }
            } else {
                println!("Authentication failed: {}", body);
                None
            }
        }
        Err(err) => {
            println!("Error sending request: {:?}", err);
            None
        }
    }
}

pub async fn post_to_bluesky(token: &str, text: &str, user_did: &str) -> bool {
    use chrono::Utc;

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct BlueskyPost {
        repo: String,       // The DID of the user
        collection: String, // The type of record
        r#type: String,     // The schema type of the record
        record: Record,     // The actual content of the post
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Record {
        text: String,       // The post's text content
        created_at: String, // ISO 8601 timestamp
    }

    let client = Client::new();

    let post_data = BlueskyPost {
        repo: user_did.to_string(),
        collection: "app.bsky.feed.post".to_string(),
        r#type: "app.bsky.feed.post".to_string(),
        record: Record {
            text: text.to_string(),
            created_at: Utc::now().to_rfc3339(), // Generate the current timestamp in ISO 8601 format
        },
    };

    match client
        .post("https://bsky.social/xrpc/com.atproto.repo.createRecord")
        .bearer_auth(token)
        .json(&post_data)
        .send()
        .await
    {
        Ok(response) => {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            // println!("Post Response Status: {}", status);
            // println!("Post Response Body: {}", body);

            if status.is_success() {
                true
            } else {
                println!("Post failed: {}", body);
                false
            }
        }
        Err(err) => {
            println!("Error posting to Bluesky: {:?}", err);
            false
        }
    }
}
