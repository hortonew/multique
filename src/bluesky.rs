use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::{Arc, Mutex};

use crate::posts;

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
                        let did = auth_response.did.clone();

                        // Update the posts::AppState
                        let mut state = state.lock().unwrap();
                        state.bluesky_token = Some(token.clone());
                        state.did = Some(did);

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
