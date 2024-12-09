use dotenv::dotenv;
use eframe::egui;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::Mutex;

mod bluesky;
mod linkedin;
mod mastodon;
mod posts;
mod twitter;

struct PostApp {
    state: Arc<Mutex<posts::AppState>>,
    rt: Arc<Runtime>,
}

impl PostApp {
    fn new() -> Self {
        let state = Arc::new(Mutex::new(posts::AppState::default()));

        // Load Bluesky tokens from storage
        if let Some(tokens) = bluesky::load_tokens() {
            let mut state_guard = futures::executor::block_on(state.lock());
            state_guard.bluesky_token = Some(tokens.access_jwt);
            state_guard.did = Some(tokens.did);
            state_guard.bluesky_authorized = true; // Mark Bluesky as authorized
        }

        // Load Bearer Token for Twitter
        if twitter::load_bearer_token().is_some() {
            let mut state_guard = futures::executor::block_on(state.lock());
            state_guard.twitter_authorized = true;
        }

        // Load LinkedIn tokens
        if linkedin::load_bearer_token().is_some() {
            let mut state_guard = futures::executor::block_on(state.lock());
            state_guard.linkedin_authorized = true;
        }

        // Load Mastodon Access Token
        if mastodon::load_tokens().is_some() {
            let mut state_guard = futures::executor::block_on(state.lock());
            state_guard.mastodon_authorized = true;
        }

        Self {
            state,
            rt: Arc::new(Runtime::new().unwrap()),
        }
    }
}

impl eframe::App for PostApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let state_clone = Arc::clone(&self.state);

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("🌟 Multique - Multi-Platform Poster");

            // Platform Authorization Section
            ui.group(|ui| {
                ui.label("Platform Authorizations:");

                // Twitter Authorization
                ui.horizontal(|ui| {
                    ui.label("🐦 Twitter / X:");
                    let state = futures::executor::block_on(state_clone.lock());
                    if state.twitter_authorized {
                        ui.colored_label(egui::Color32::GREEN, "Authorized ✅");
                    } else if ui.button("Authorize").clicked() {
                        let rt = Arc::clone(&self.rt);
                        let state_clone = Arc::clone(&self.state);
                        rt.spawn(async move {
                            if let Some(auth_url) = twitter::generate_auth_url().await {
                                println!("Authorize your app at: {}", auth_url);

                                println!("Enter the authorization code:");
                                let mut input_code = String::new();
                                std::io::stdin().read_line(&mut input_code).unwrap();
                                let code = input_code.trim().to_string();

                                if twitter::authorize_twitter(state_clone.clone(), &code).await.is_some() {
                                    let mut state = state_clone.lock().await;
                                    state.twitter_authorized = true;
                                }
                            }
                        });
                    }
                });

                // Bluesky Authorization
                ui.horizontal(|ui| {
                    ui.label("☁️ Bluesky:");
                    let state = futures::executor::block_on(state_clone.lock());
                    if state.bluesky_authorized {
                        ui.colored_label(egui::Color32::GREEN, "Authorized ✅");
                    } else if ui.button("Authorize").clicked() {
                        let rt = Arc::clone(&self.rt);
                        let state_clone = Arc::clone(&self.state);
                        rt.spawn(async move {
                            if bluesky::authorize_bluesky(state_clone.clone()).await.is_some() {
                                let mut state = state_clone.lock().await;
                                state.bluesky_authorized = true;
                            }
                        });
                    }
                });

                // Mastodon Authorization
                ui.horizontal(|ui| {
                    ui.label("🐘 Mastodon:");
                    let state = futures::executor::block_on(state_clone.lock());
                    if state.mastodon_authorized {
                        ui.colored_label(egui::Color32::GREEN, "Authorized ✅");
                    } else if ui.button("Authorize").clicked() {
                        let rt = Arc::clone(&self.rt);
                        let state_clone = Arc::clone(&self.state);
                        rt.spawn(async move {
                            let client_id =
                                std::env::var("MASTODON_CLIENT_ID").expect("MASTODON_CLIENT_ID not set in .env");
                            let client_secret = std::env::var("MASTODON_CLIENT_SECRET")
                                .expect("MASTODON_CLIENT_SECRET not set in .env");

                            let authorization_url = mastodon::generate_auth_url(&client_id).await;
                            println!("Authorize your app at: {}", authorization_url);

                            println!("Enter the authorization code:");
                            let mut input_code = String::new();
                            std::io::stdin().read_line(&mut input_code).unwrap();
                            let code = input_code.trim().to_string();

                            if let Some(access_token) =
                                mastodon::authorize_mastodon(&client_id, &client_secret, &code).await
                            {
                                mastodon::save_tokens(&access_token);
                                let mut state = state_clone.lock().await;
                                state.mastodon_authorized = true;
                            }
                        });
                    }
                });

                // LinkedIn Authorization
                ui.horizontal(|ui| {
                    ui.label("🔗 LinkedIn:");
                    let state = futures::executor::block_on(state_clone.lock());
                    if state.linkedin_authorized {
                        ui.colored_label(egui::Color32::GREEN, "Authorized ✅");
                    } else if ui.button("Authorize").clicked() {
                        let rt = Arc::clone(&self.rt);
                        let state_clone = Arc::clone(&self.state);
                        rt.spawn(async move {
                            if let Some(auth_url) = linkedin::generate_auth_url().await {
                                println!("Authorize your app at: {}", auth_url);

                                println!("Enter the authorization code:");
                                let mut input_code = String::new();
                                std::io::stdin().read_line(&mut input_code).unwrap();
                                let code = input_code.trim().to_string();

                                if linkedin::authorize_linkedin(state_clone.clone(), &code).await.is_some() {
                                    let mut state = state_clone.lock().await;
                                    state.linkedin_authorized = true;
                                }
                            }
                        });
                    }
                });
            });

            ui.separator();

            // Compose and Post Section
            ui.group(|ui| {
                ui.label("Compose your message:");
                {
                    let mut state = futures::executor::block_on(state_clone.lock());
                    ui.text_edit_multiline(&mut state.post_text);
                }

                if ui
                    .add(egui::Button::new("📤 Post").fill(egui::Color32::DARK_GRAY))
                    .clicked()
                {
                    let state = Arc::clone(&self.state);
                    let rt = Arc::clone(&self.rt);

                    rt.spawn(async move {
                        let mut state = state.lock().await;
                        let text = state.post_text.clone();

                        // Post to Twitter
                        if state.twitter_authorized {
                            if let Some(bearer_token) = twitter::load_bearer_token() {
                                if twitter::post_to_twitter(&bearer_token, &text).await {
                                    println!("Posted to Twitter successfully!");
                                } else {
                                    println!("Failed to post to Twitter.");
                                }
                            }
                        }

                        // Post to Bluesky
                        if let Some(token) = &state.bluesky_token {
                            if let Some(user_did) = &state.did {
                                if bluesky::post_to_bluesky(token, &text, user_did).await {
                                    println!("Posted to Bluesky successfully!");
                                } else {
                                    println!("Failed to post to Bluesky.");
                                }
                            }
                        }

                        // Post to Mastodon
                        if state.mastodon_authorized {
                            if let Some(token_data) = mastodon::load_tokens() {
                                if mastodon::post_to_mastodon(&token_data.access_token, &text).await {
                                    println!("Posted to Mastodon successfully!");
                                } else {
                                    println!("Failed to post to Mastodon.");
                                }
                            }
                        }

                        // Post to LinkedIn
                        if state.linkedin_authorized {
                            if let Some(linkedin_token) = linkedin::load_bearer_token() {
                                if linkedin::post_to_linkedin(&linkedin_token, &text).await {
                                    println!("Posted to LinkedIn successfully!");
                                } else {
                                    println!("Failed to post to LinkedIn.");
                                }
                            }
                        }

                        state.post_text.clear(); // Clear input after posting
                    });
                }
            });
        });
    }
}

fn main() -> Result<(), eframe::Error> {
    dotenv().ok();
    let options = eframe::NativeOptions::default();
    eframe::run_native("Multique", options, Box::new(|_cc| Ok(Box::new(PostApp::new()))))
}
