use dotenv::dotenv;
use eframe::egui;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;

#[derive(Default)]
struct AppState {
    twitter_authorized: bool,
    mastodon_authorized: bool,
    bluesky_authorized: bool,
    post_text: String,
    bluesky_token: Option<String>,
}

struct PostApp {
    state: Arc<Mutex<AppState>>,
    rt: Arc<Runtime>,
}

impl PostApp {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(AppState::default())),
            rt: Arc::new(Runtime::new().unwrap()),
        }
    }
}

impl eframe::App for PostApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut state = self.state.lock().unwrap();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Multi-Platform Poster");

            // Twitter Authorization UI
            ui.horizontal(|ui| {
                ui.label("Twitter:");
                if state.twitter_authorized {
                    ui.label("Authorized ✅");
                } else if ui.button("Authorize").clicked() {
                    // Placeholder for Twitter authorization logic
                    state.twitter_authorized = true;
                }
            });

            // Mastodon Authorization UI
            ui.horizontal(|ui| {
                ui.label("Mastodon:");
                if state.mastodon_authorized {
                    ui.label("Authorized ✅");
                } else if ui.button("Authorize").clicked() {
                    // Placeholder for Mastodon authorization logic
                    state.mastodon_authorized = true;
                }
            });

            // Bluesky Authorization UI
            ui.horizontal(|ui| {
                ui.label("Bluesky:");
                if state.bluesky_authorized {
                    ui.label("Authorized ✅");
                } else if ui.button("Authorize").clicked() {
                    let rt = Arc::clone(&self.rt);
                    let state = self.state.clone();
                    rt.spawn(async move {
                        if let Some(token) = authorize_bluesky().await {
                            let mut state = state.lock().unwrap();
                            state.bluesky_authorized = true;
                            state.bluesky_token = Some(token);
                        }
                    });
                }
            });

            ui.separator();

            // Post Input UI
            ui.add_enabled_ui(
                state.twitter_authorized || state.mastodon_authorized || state.bluesky_authorized,
                |ui| {
                    ui.text_edit_multiline(&mut state.post_text);
                },
            );

            // Post Button
            if ui.button("Post").clicked() {
                let text = state.post_text.clone();
                let token = state.bluesky_token.clone();
                if let Some(token) = token {
                    let rt = self.rt.clone();
                    rt.spawn(async move {
                        if post_to_bluesky(&token, &text).await {
                            println!("Post successful!");
                        } else {
                            println!("Failed to post to Bluesky.");
                        }
                    });
                }
            }
        });
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct BlueskyAuthResponse {
    access_jwt: String,
    refresh_jwt: String,
    handle: String,
    email: String,
}

async fn authorize_bluesky() -> Option<String> {
    #[derive(Serialize)]
    struct BlueskyAuthRequest {
        identifier: String,
        password: String,
    }

    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    struct BlueskyAuthResponse {
        access_jwt: String,
        refresh_jwt: String,
        handle: String,
        email: String,
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
            println!("Response Status: {}", status);
            println!("Response Body: {}", body);

            if status.is_success() {
                match serde_json::from_str::<BlueskyAuthResponse>(&body) {
                    Ok(auth_response) => {
                        println!("Successfully authenticated as {}", auth_response.handle);
                        Some(auth_response.access_jwt)
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

async fn post_to_bluesky(token: &str, text: &str) -> bool {
    #[derive(Serialize)]
    struct BlueskyPost {
        text: String,
    }

    let client = Client::new();
    let post_data = BlueskyPost {
        text: text.to_string(),
    };

    match client
        .post("https://bsky.social/xrpc/com.atproto.repo.createRecord")
        .bearer_auth(token)
        .json(&post_data)
        .send()
        .await
    {
        Ok(response) => response.status().is_success(),
        Err(err) => {
            println!("Error posting to Bluesky: {:?}", err);
            false
        }
    }
}

fn main() -> Result<(), eframe::Error> {
    dotenv().ok();
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Multi-Platform Poster",
        options,
        Box::new(|_cc| Ok(Box::new(PostApp::new()))),
    )
}
