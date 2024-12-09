use dotenv::dotenv;
use eframe::egui;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::Mutex;

mod bluesky;
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
        if let Some(_bearer_token) = twitter::load_bearer_token() {
            let mut state_guard = futures::executor::block_on(state.lock());
            state_guard.twitter_authorized = true;
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
                let state = futures::executor::block_on(state_clone.lock());
                if state.twitter_authorized {
                    ui.label("Authorized ✅");
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

                            if twitter::authorize_twitter(state_clone.clone(), &code)
                                .await
                                .is_some()
                            {
                                let mut state = state_clone.lock().await;
                                state.twitter_authorized = true;
                            }
                        }
                    });
                }
            });

            // Bluesky Authorization UI
            ui.horizontal(|ui| {
                ui.label("Bluesky:");
                let state = futures::executor::block_on(state_clone.lock());
                if state.bluesky_authorized {
                    ui.label("Authorized ✅");
                } else if ui.button("Authorize").clicked() {
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

                    let text = state.post_text.clone();

                    // Post to Twitter if authorized
                    if state.twitter_authorized {
                        if let Some(bearer_token) = twitter::load_bearer_token() {
                            if twitter::post_to_twitter(&bearer_token, &text).await {
                                println!("Posted to Twitter successfully!");
                            } else {
                                println!("Failed to post to Twitter.");
                            }
                        }
                    }

                    // Post to Bluesky if authorized
                    if let Some(token) = &state.bluesky_token {
                        if let Some(user_did) = &state.did {
                            let token = token.clone();
                            let user_did = user_did.clone();

                            if bluesky::post_to_bluesky(&token, &text, &user_did).await {
                                println!("Posted to Bluesky successfully!");
                            } else {
                                println!("Failed to post to Bluesky.");
                            }
                        }
                    }

                    // Clear the input box after posting
                    state.post_text.clear();
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
