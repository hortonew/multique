

## Implement a new app in main.rs

Load token

```rust
mod new;

// make sure new_authorized gets added to posts.rs

// Load new tokens
if let Some(_new_token) = new::load_bearer_token() {
    let mut state_guard = futures::executor::block_on(state.lock());
    state_guard.new_authorized = true;
}
```

Add to UI

```rust
// new Authorization UI
ui.horizontal(|ui| {
    ui.label("New:");
    let state = futures::executor::block_on(state_clone.lock());
    if state.new_authorized {
        ui.label("Authorized ✅");
    } else if ui.button("Authorize").clicked() {
        let rt = Arc::clone(&self.rt);
        let state_clone = Arc::clone(&self.state);
        rt.spawn(async move {
            if let Some(auth_url) = new::generate_auth_url().await {
                println!("Authorize your app at: {}", auth_url);

                println!("Enter the authorization code:");
                let mut input_code = String::new();
                std::io::stdin().read_line(&mut input_code).unwrap();
                let code = input_code.trim().to_string();

                if new::authorize_new(state_clone.clone(), &code)
                    .await
                    .is_some()
                {
                    let mut state = state_clone.lock().await;
                    state.new_authorized = true;
                }
            }
        });
    }
});
```

Post if authorized

```rust
// Post to new if authorized
if state.new_authorized {
    if let Some(new_token) = new::load_bearer_token() {
        if new::post_to_new(&new_token, &text).await {
            println!("Posted to new successfully!");
        } else {
            println!("Failed to post to new.");
        }
    }
}
```