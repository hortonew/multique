use dotenv::dotenv;
use eframe::egui;
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;
mod bluesky;
mod posts;

struct PostApp {
    state: Arc<Mutex<posts::AppState>>,
    rt: Arc<Runtime>,
}

impl PostApp {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(posts::AppState::default())),
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
                    state.twitter_authorized = true;
                }
            });

            // Mastodon Authorization UI
            ui.horizontal(|ui| {
                ui.label("Mastodon:");
                if state.mastodon_authorized {
                    ui.label("Authorized ✅");
                } else if ui.button("Authorize").clicked() {
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
                    let state_clone = Arc::clone(&self.state);
                    rt.spawn(async move {
                        if bluesky::authorize_bluesky(Arc::clone(&state_clone))
                            .await
                            .is_some()
                        {
                            let mut state = state_clone.lock().unwrap();
                            state.bluesky_authorized = true;
                        }
                    });
                }
            });

            ui.separator();

            // Post Input UI
            ui.text_edit_multiline(&mut state.post_text);

            // Post Button
            if ui.button("Post").clicked() {
                if let Some(token) = &state.bluesky_token {
                    if let Some(user_did) = &state.did {
                        let text = state.post_text.clone();
                        let rt = Arc::clone(&self.rt);
                        let state_clone = Arc::clone(&self.state);
                        let token = token.clone();
                        let user_did = user_did.clone();

                        rt.spawn(async move {
                            if bluesky::post_to_bluesky(&token, &text, &user_did).await {
                                println!("Post successful!");
                                // Clear the input box after a successful post
                                let mut state = state_clone.lock().unwrap();
                                state.post_text.clear();
                            } else {
                                println!("Failed to post to Bluesky.");
                            }
                        });
                    } else {
                        println!("No DID available for posting.");
                    }
                } else {
                    println!("Not authorized for Bluesky.");
                }
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
