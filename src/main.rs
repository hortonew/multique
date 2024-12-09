use dotenv::dotenv;
use eframe::egui;
use std::collections::HashMap;
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
    platform_checkboxes: HashMap<&'static str, bool>, // Added checkboxes state
}

impl PostApp {
    fn new() -> Self {
        let state = Arc::new(Mutex::new(posts::AppState::default()));
        let rt = Arc::new(Runtime::new().unwrap());

        let platform_checkboxes = HashMap::from([
            ("Twitter", true),
            ("Bluesky", true),
            ("Mastodon", true),
            ("LinkedIn", false),
        ]);

        // Load Bluesky tokens and validate
        if let Some(tokens) = bluesky::load_tokens() {
            let mut state_guard = futures::executor::block_on(state.lock());
            state_guard.bluesky_token = Some(tokens.access_jwt.clone());
            state_guard.did = Some(tokens.did.clone());
            state_guard.bluesky_authorized = true; // Assume authorized for now

            // Validate and refresh token asynchronously
            let state_clone = Arc::clone(&state);
            rt.spawn(async move {
                if let Some(new_tokens) = bluesky::refresh_access_token(&tokens.refresh_jwt).await {
                    let mut state = state_clone.lock().await;
                    state.bluesky_token = Some(new_tokens.access_jwt);
                    state.did = Some(new_tokens.did);
                    println!("Bluesky token refreshed successfully.");
                } else {
                    println!("Bluesky token refresh failed. Attempting reauthorization...");
                    if let Some(new_tokens) = bluesky::reauthorize_bluesky().await {
                        let mut state = state_clone.lock().await;
                        state.bluesky_token = Some(new_tokens.access_jwt);
                        state.did = Some(new_tokens.did);
                        println!("Bluesky reauthorized successfully.");
                    } else {
                        let mut state = state_clone.lock().await;
                        state.bluesky_authorized = false;
                        println!("Failed to refresh or reauthorize Bluesky.");
                    }
                }
            });
        }

        // Load tokens for other platforms
        if twitter::load_bearer_token().is_some() {
            let mut state_guard = futures::executor::block_on(state.lock());
            state_guard.twitter_authorized = true;
        }

        if linkedin::load_bearer_token().is_some() {
            let mut state_guard = futures::executor::block_on(state.lock());
            state_guard.linkedin_authorized = true;
        }

        if mastodon::load_tokens().is_some() {
            let mut state_guard = futures::executor::block_on(state.lock());
            state_guard.mastodon_authorized = true;
        }

        Self {
            state,
            rt,
            platform_checkboxes,
        }
    }
}

impl eframe::App for PostApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let available_width = ctx.available_rect().width();
        let main_section_width = available_width * 0.6;
        let side_panel_width = available_width * 0.4;
        let state_clone = Arc::clone(&self.state);
        egui::SidePanel::right("note_panel").exact_width(side_panel_width).show(ctx, |ui| {
            ui.heading(egui::RichText::new("📝 Instructions").color(egui::Color32::GREEN));
            ui.label("1. Authorize the platforms you want to use.\n2. Check the boxes for the platforms you want to post to.\n3. Write your message and click 'Post.'\n\nKeep posts to under 5 every 15 minutes to avoid rate limiting.");

            ui.add_space(20.0);

            ui.heading(
                egui::RichText::new("📋 It will post to:")
                    .color(egui::Color32::GREEN),
            );
            let state = futures::executor::block_on(state_clone.lock());

            for (platform, checked) in &self.platform_checkboxes {
                let is_authorized = match *platform {
                    "Twitter" => state.twitter_authorized,
                    "Bluesky" => state.bluesky_authorized,
                    "Mastodon" => state.mastodon_authorized,
                    "LinkedIn" => state.linkedin_authorized,
                    _ => false,
                };

                if *checked && is_authorized {
                    ui.label(format!("- {}", platform));
                }
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.set_min_width(main_section_width);
            ui.heading("🌟 Multique - Post to all the platforms!");

            ui.add_space(20.0);
            // Platform Selection Section
            ui.group(|ui| {
                ui.set_min_width(400.0);
                ui.label("Platforms:");

                render_platform_checkbox(
                    ui,
                    "🐦 Twitter / X:",
                    "Twitter",
                    &mut self.platform_checkboxes,
                    |state| state.twitter_authorized,
                    || {
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
                    },
                    state_clone.clone(),
                );

                render_platform_checkbox(
                    ui,
                    "☁️ Bluesky:",
                    "Bluesky",
                    &mut self.platform_checkboxes,
                    |state| state.bluesky_authorized,
                    || {
                        let rt = Arc::clone(&self.rt);
                        let state_clone = Arc::clone(&self.state);
                        rt.spawn(async move {
                            if bluesky::authorize_bluesky(state_clone.clone()).await.is_some() {
                                let mut state = state_clone.lock().await;
                                state.bluesky_authorized = true;
                            }
                        });
                    },
                    state_clone.clone(),
                );

                render_platform_checkbox(
                    ui,
                    "🐘 Mastodon:",
                    "Mastodon",
                    &mut self.platform_checkboxes,
                    |state| state.mastodon_authorized,
                    || {
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
                    },
                    state_clone.clone(),
                );

                render_platform_checkbox(
                    ui,
                    "🔗 LinkedIn:",
                    "LinkedIn",
                    &mut self.platform_checkboxes,
                    |state| state.linkedin_authorized,
                    || {
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
                    },
                    state_clone.clone(),
                );
            });

            ui.add_space(20.0);

            // Compose and Post Section
            ui.group(|ui| {
                ui.set_min_width(400.0);
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
                    let selected_platforms = self.platform_checkboxes.clone();

                    rt.spawn(async move {
                        let mut state = state.lock().await;
                        let text = state.post_text.clone();

                        // Post only to platforms that are authorized and selected
                        if *selected_platforms.get("Twitter").unwrap_or(&false) && state.twitter_authorized {
                            if let Some(bearer_token) = twitter::load_bearer_token() {
                                if twitter::post_to_twitter(&bearer_token, &text).await {
                                    println!("Posted to Twitter successfully!");
                                } else {
                                    println!("Failed to post to Twitter.");
                                }
                            }
                        }

                        if *selected_platforms.get("Bluesky").unwrap_or(&false) && state.bluesky_authorized {
                            if let Some(token) = state.bluesky_token.clone() {
                                if let Some(user_did) = state.did.clone() {
                                    if bluesky::post_to_bluesky(&token, &text, &user_did).await {
                                        println!("Posted to Bluesky successfully!");
                                    } else {
                                        println!("Failed to post to Bluesky.");
                                    }
                                }
                            }
                        }

                        if *selected_platforms.get("Mastodon").unwrap_or(&false) && state.mastodon_authorized {
                            if let Some(token_data) = mastodon::load_tokens() {
                                if mastodon::post_to_mastodon(&token_data.access_token, &text).await {
                                    println!("Posted to Mastodon successfully!");
                                } else {
                                    println!("Failed to post to Mastodon.");
                                }
                            }
                        }

                        if *selected_platforms.get("LinkedIn").unwrap_or(&false) && state.linkedin_authorized {
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

/// Helper function to render a platform's checkbox and authorization status
fn render_platform_checkbox<F, G>(
    ui: &mut egui::Ui,
    label: &str,
    platform_key: &'static str,
    platform_checkboxes: &mut HashMap<&'static str, bool>,
    is_authorized: F,
    authorize_action: G,
    state_clone: Arc<Mutex<posts::AppState>>,
) where
    F: FnOnce(&posts::AppState) -> bool,
    G: FnOnce(),
{
    ui.horizontal(|ui| {
        ui.label(label);

        // Check authorization status
        let state = futures::executor::block_on(state_clone.lock());
        if is_authorized(&state) {
            ui.colored_label(egui::Color32::GREEN, "Authorized ✅");
            // Show checkbox only if authorized
            if let Some(checked) = platform_checkboxes.get_mut(platform_key) {
                ui.checkbox(checked, "");
            }
        } else {
            ui.colored_label(egui::Color32::RED, "Not Authorized ❌");
            // Show the "Authorize" button for unauthorized platforms
            if ui.button("Authorize").clicked() {
                authorize_action();
            }
        }
    });
}

fn main() -> Result<(), eframe::Error> {
    dotenv().ok();
    let options = eframe::NativeOptions::default();
    eframe::run_native("Multique", options, Box::new(|_cc| Ok(Box::new(PostApp::new()))))
}
