use dotenv::dotenv;
use eframe::egui;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::Mutex;

mod bluesky;
mod posts;

fn is_token_valid(token: &str) -> bool {
    !token.is_empty()
}

struct PostApp {
    state: Arc<Mutex<posts::AppState>>,
    rt: Arc<Runtime>,
}

impl PostApp {
    fn new() -> Self {
        let state = Arc::new(Mutex::new(posts::AppState::default()));

        // Load tokens from storage
        if let Some(tokens) = bluesky::load_tokens() {
            let mut state_guard = futures::executor::block_on(state.lock());
            state_guard.bluesky_token = Some(tokens.access_jwt);
            state_guard.did = Some(tokens.did);
            state_guard.bluesky_authorized = true; // Mark Bluesky as authorized
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
            ui.heading("Multi-Platform Poster");

            // Twitter Authorization UI
            ui.horizontal(|ui| {
                ui.label("Twitter:");
                let mut state = futures::executor::block_on(state_clone.lock());
                if state.twitter_authorized {
                    ui.label("Authorized ✅");
                } else if ui.button("Authorize").clicked() {
                    state.twitter_authorized = true;
                }
            });

            // Mastodon Authorization UI
            ui.horizontal(|ui| {
                ui.label("Mastodon:");
                let mut state = futures::executor::block_on(state_clone.lock());
                if state.mastodon_authorized {
                    ui.label("Authorized ✅");
                } else if ui.button("Authorize").clicked() {
                    state.mastodon_authorized = true;
                }
            });

            // Bluesky Authorization UI
            ui.horizontal(|ui| {
                ui.label("Bluesky:");
                let mut state = futures::executor::block_on(state_clone.lock());
                if state.bluesky_authorized {
                    ui.label("Authorized ✅");
                } else {
                    ui.add_enabled_ui(!state.bluesky_authorized, |ui| {
                        if ui.button("Authorize").clicked() {
                            let rt = Arc::clone(&self.rt);
                            let state_clone = Arc::clone(&self.state);
                            rt.spawn(async move {
                                if bluesky::authorize_bluesky(state_clone.clone())
                                    .await
                                    .is_some()
                                {
                                    let mut state = state_clone.lock().await;
                                    state.bluesky_authorized = true;
                                }
                            });
                        }
                    });
                }
            });

            ui.separator();

            // Post Input UI
            {
                let mut state = futures::executor::block_on(state_clone.lock());
                ui.text_edit_multiline(&mut state.post_text);
            }

            // Post Button
            if ui.button("Post").clicked() {
                let state = Arc::clone(&self.state);
                let rt = Arc::clone(&self.rt);

                rt.spawn(async move {
                    let mut state = state.lock().await;

                    if let Some(token) = &state.bluesky_token {
                        if let Some(user_did) = &state.did {
                            let text = state.post_text.clone();
                            let token = token.clone();
                            let user_did = user_did.clone();

                            let final_token = if !is_token_valid(&token) {
                                if let Some(refresh_token) = &state.bluesky_token {
                                    let new_token = {
                                        if let Some(new_token) =
                                            bluesky::refresh_access_token(refresh_token).await
                                        {
                                            bluesky::save_tokens(
                                                &new_token,
                                                refresh_token,
                                                &user_did,
                                            );
                                            new_token
                                        } else {
                                            println!("Failed to refresh token");
                                            return;
                                        }
                                    };
                                    state.bluesky_token = Some(new_token.clone());
                                    new_token
                                } else {
                                    println!("No refresh token available");
                                    return;
                                }
                            } else {
                                token
                            };

                            if bluesky::post_to_bluesky(&final_token, &text, &user_did).await {
                                println!("Post successful!");
                                state.post_text.clear();
                            } else {
                                println!("Failed to post to Bluesky.");
                            }
                        } else {
                            println!("No DID available for posting.");
                        }
                    } else {
                        println!("Not authorized for Bluesky.");
                    }
                });
            }
        });
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
