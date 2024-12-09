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

pub async fn refresh_access_token(refresh_jwt: &str) -> Option<TokenData> {
    #[derive(Serialize)]
    struct RefreshRequest {
        refreshJwt: String,
    }

    #[derive(Deserialize)]
    struct RefreshResponse {
        accessJwt: String,
        refreshJwt: String,
        did: String,
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
                save_tokens(
                    &refresh_response.accessJwt,
                    &refresh_response.refreshJwt,
                    &refresh_response.did,
                );
                Some(TokenData {
                    access_jwt: refresh_response.accessJwt,
                    refresh_jwt: refresh_response.refreshJwt,
                    did: refresh_response.did,
                })
            } else {
                println!("Failed to refresh access token: {:?}", response.text().await);
                None
            }
        }
        Err(err) => {
            println!("Error refreshing token: {:?}", err);
            None
        }
    }
}

pub async fn authorize_bluesky(state: Arc<Mutex<posts::AppState>>) -> Option<TokenData> {
    let client = Client::new();
    let auth_data = create_auth_request();

    match send_auth_request(&client, &auth_data).await {
        Ok(response) if response.status().is_success() => process_auth_success(response, state).await,
        Ok(response) => {
            log_auth_error("Authorization failed", response).await;
            None
        }
        Err(err) => {
            println!("Error during authorization: {:?}", err);
            None
        }
    }
}

async fn process_auth_success(response: reqwest::Response, state: Arc<Mutex<posts::AppState>>) -> Option<TokenData> {
    if let Ok(auth_response) = response.json::<BlueskyAuthResponse>().await {
        save_tokens(
            &auth_response.access_jwt,
            &auth_response.refresh_jwt,
            &auth_response.did,
        );

        update_app_state(&state, &auth_response).await;

        Some(TokenData {
            access_jwt: auth_response.access_jwt,
            refresh_jwt: auth_response.refresh_jwt,
            did: auth_response.did,
        })
    } else {
        println!("Failed to parse authorization response.");
        None
    }
}

async fn update_app_state(state: &Arc<Mutex<posts::AppState>>, auth_response: &BlueskyAuthResponse) {
    let mut state = state.lock().await;
    state.bluesky_token = Some(auth_response.access_jwt.clone());
    state.did = Some(auth_response.did.clone());
    state.bluesky_authorized = true;
}

async fn log_auth_error(context: &str, response: reqwest::Response) {
    let status = response.status();
    let body = response
        .text()
        .await
        .unwrap_or_else(|_| "<Failed to read body>".to_string());
    println!("{}: Status: {}, Body: {}", context, status, body);
}

pub async fn reauthorize_bluesky() -> Option<TokenData> {
    let client = Client::new();
    let auth_data = create_auth_request();

    match send_auth_request(&client, &auth_data).await {
        Ok(response) => {
            if response.status().is_success() {
                if let Ok(auth_response) = response.json::<BlueskyAuthResponse>().await {
                    save_tokens(
                        &auth_response.access_jwt,
                        &auth_response.refresh_jwt,
                        &auth_response.did,
                    );

                    Some(TokenData {
                        access_jwt: auth_response.access_jwt,
                        refresh_jwt: auth_response.refresh_jwt,
                        did: auth_response.did,
                    })
                } else {
                    println!("Failed to parse reauthorization response.");
                    None
                }
            } else {
                println!("Reauthorization failed: {:?}", response.text().await);
                None
            }
        }
        Err(err) => {
            println!("Error during reauthorization: {:?}", err);
            None
        }
    }
}

pub async fn post_to_bluesky(token: &str, text: &str, user_did: &str) -> bool {
    let client = Client::new();
    let mut current_token = token.to_string();

    for _ in 0..2 {
        // Attempt to post
        if try_post(&client, &current_token, text, user_did).await {
            return true;
        }

        // If posting fails due to authentication, attempt to refresh or reauthorize
        if !refresh_or_reauthorize(&mut current_token).await {
            break;
        }
    }

    println!("All attempts to post to Bluesky failed.");
    false
}

async fn try_post(client: &Client, token: &str, text: &str, user_did: &str) -> bool {
    use chrono::Utc;

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Record {
        text: String,
        created_at: String,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct PostData {
        repo: String,
        collection: String,
        r#type: String,
        record: Record,
    }

    let post_data = PostData {
        repo: user_did.to_string(),
        collection: "app.bsky.feed.post".to_string(),
        r#type: "app.bsky.feed.post".to_string(),
        record: Record {
            text: text.to_string(),
            created_at: Utc::now().to_rfc3339(),
        },
    };

    match client
        .post("https://bsky.social/xrpc/com.atproto.repo.createRecord")
        .bearer_auth(token)
        .json(&post_data)
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => true,
        Ok(response) => {
            println!(
                "Post failed with status {}: {:?}",
                response.status(),
                response.text().await
            );
            false
        }
        Err(err) => {
            println!("Error posting to Bluesky: {:?}", err);
            false
        }
    }
}

async fn refresh_or_reauthorize(current_token: &mut String) -> bool {
    if let Some(tokens) = load_tokens() {
        if let Some(new_tokens) = refresh_access_token(&tokens.refresh_jwt).await {
            *current_token = new_tokens.access_jwt;
            return true;
        }
        if let Some(new_tokens) = reauthorize_bluesky().await {
            *current_token = new_tokens.access_jwt;
            return true;
        }
    }
    println!("Failed to refresh or reauthorize token.");
    false
}

fn create_auth_request() -> BlueskyAuthRequest {
    BlueskyAuthRequest {
        identifier: env::var("BLUESKY_USERNAME").unwrap_or_else(|_| "invalid_username".to_string()),
        password: env::var("BLUESKY_PASSWORD").unwrap_or_else(|_| "invalid_password".to_string()),
    }
}

async fn send_auth_request(
    client: &Client,
    auth_data: &BlueskyAuthRequest,
) -> Result<reqwest::Response, reqwest::Error> {
    client
        .post("https://bsky.social/xrpc/com.atproto.server.createSession")
        .json(auth_data)
        .send()
        .await
}

#[derive(Serialize)]
struct BlueskyAuthRequest {
    identifier: String,
    password: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BlueskyAuthResponse {
    access_jwt: String,
    refresh_jwt: String,
    did: String,
}
