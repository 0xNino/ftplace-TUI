use crate::api_client::ApiError;
use crate::app_state::{App, InputMode};
use crate::art::{load_default_pixel_art, ArtPixel, PixelArt};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use std::fs::File;
use std::io;
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};

impl App {
    pub async fn handle_events(&mut self) -> io::Result<()> {
        // Check if we need to fetch board on startup (when tokens were restored)
        if self.should_fetch_board_on_start {
            self.should_fetch_board_on_start = false; // Clear the flag
            self.fetch_board_data().await;
        }

        let mut should_refresh_board = false;
        if self.input_mode == InputMode::None
            && self.initial_board_fetched
            && self.api_client.get_auth_cookie_preview().is_some()
        {
            if let Some(last_refresh) = self.last_board_refresh {
                if last_refresh.elapsed() >= Duration::from_secs(10) {
                    should_refresh_board = true;
                }
            }
        }

        if should_refresh_board {
            self.status_message = "Auto-refreshing board...".to_string();
            self.fetch_board_data().await;
        }

        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key_event) => {
                    if key_event.kind == KeyEventKind::Press {
                        match self.input_mode {
                            InputMode::EnterBaseUrl => {
                                match key_event.code {
                                    KeyCode::Up => {
                                        if self.base_url_selection_index > 0 {
                                            self.base_url_selection_index -= 1;
                                        }
                                    }
                                    KeyCode::Down => {
                                        if self.base_url_selection_index
                                            < self.base_url_options.len() - 1
                                        {
                                            self.base_url_selection_index += 1;
                                        }
                                    }
                                    KeyCode::Enter => {
                                        let selected_option =
                                            &self.base_url_options[self.base_url_selection_index];
                                        if selected_option == "Custom" {
                                            self.input_mode = InputMode::EnterCustomBaseUrlText;
                                            self.status_message =
                                                "Enter Custom API Base URL:".to_string();
                                            self.input_buffer.clear();
                                        } else {
                                            self.api_client.set_base_url(selected_option.clone());
                                            self.input_mode = InputMode::EnterAccessToken;
                                            self.status_message = "Base URL set. Enter Access Token (or Enter to skip):".to_string();
                                            self.input_buffer.clear();
                                            // Save the base URL immediately
                                            self.save_tokens();
                                        }
                                    }
                                    KeyCode::Char('q') => self.exit = true,
                                    _ => {}
                                }
                            }
                            InputMode::EnterCustomBaseUrlText => {
                                match key_event.code {
                                    KeyCode::Enter => {
                                        let url = self.input_buffer.trim().to_string();
                                        if url.is_empty()
                                            || !(url.starts_with("http://")
                                                || url.starts_with("https://"))
                                        {
                                            self.status_message = "Invalid URL. Must start with http:// or https://. Please re-enter Custom Base URL:".to_string();
                                            self.input_buffer.clear();
                                        } else {
                                            self.api_client.set_base_url(url);
                                            self.input_mode = InputMode::EnterAccessToken;
                                            self.status_message = "Base URL set. Enter Access Token (or Enter to skip):".to_string();
                                            self.input_buffer.clear();
                                            // Save the base URL immediately
                                            self.save_tokens();
                                        }
                                    }
                                    KeyCode::Esc => {
                                        self.input_mode = InputMode::EnterBaseUrl;
                                        self.status_message =
                                            "Custom URL input cancelled. Select API Base URL:"
                                                .to_string();
                                        self.input_buffer.clear();
                                    }
                                    KeyCode::Char(to_insert) => self.input_buffer.push(to_insert),
                                    KeyCode::Backspace => {
                                        self.input_buffer.pop();
                                    }
                                    _ => {}
                                }
                            }
                            InputMode::EnterAccessToken => match key_event.code {
                                KeyCode::Enter => {
                                    let token = self.input_buffer.trim().to_string();
                                    let current_refresh = self.api_client.get_refresh_token_clone();
                                    if !token.is_empty() {
                                        self.api_client.set_tokens(Some(token), current_refresh);
                                        self.status_message = "Access Token set. Enter Refresh Token (or Enter to skip):".to_string();
                                    } else {
                                        self.api_client.set_tokens(None, current_refresh);
                                        self.status_message = "Access Token skipped. Enter Refresh Token (or Enter to skip):".to_string();
                                    }
                                    self.input_mode = InputMode::EnterRefreshToken;
                                    self.input_buffer.clear();
                                    // Save tokens after setting access token
                                    self.save_tokens();
                                }
                                KeyCode::Esc => {
                                    self.input_mode = InputMode::EnterBaseUrl;
                                    self.status_message =
                                        "Token input cancelled. Select API Base URL:".to_string();
                                    self.input_buffer.clear();
                                }
                                KeyCode::Char(to_insert) => self.input_buffer.push(to_insert),
                                KeyCode::Backspace => {
                                    self.input_buffer.pop();
                                }
                                _ => {}
                            },
                            InputMode::EnterRefreshToken => match key_event.code {
                                KeyCode::Enter => {
                                    let refresh = self.input_buffer.trim().to_string();
                                    let current_access = self.api_client.get_access_token_clone();
                                    if !refresh.is_empty() {
                                        self.api_client.set_tokens(current_access, Some(refresh));
                                        self.status_message = "Refresh Token set. Configuration complete. Fetching initial board...".to_string();
                                    } else {
                                        self.api_client.set_tokens(current_access, None);
                                        self.status_message = "Refresh Token skipped. Configuration complete. Fetching initial board...".to_string();
                                    }
                                    self.input_mode = InputMode::ShowHelp;
                                    self.input_buffer.clear();
                                    if !self.initial_board_fetched {
                                        self.fetch_board_data().await;
                                    }
                                    // Save tokens after setting refresh token (final step)
                                    self.save_tokens();
                                }
                                KeyCode::Esc => {
                                    self.input_mode = InputMode::EnterAccessToken;
                                    self.status_message =
                                        "Refresh Token input cancelled. Re-enter Access Token:"
                                            .to_string();
                                    self.input_buffer.clear();
                                }
                                KeyCode::Char(to_insert) => self.input_buffer.push(to_insert),
                                KeyCode::Backspace => {
                                    self.input_buffer.pop();
                                }
                                _ => {}
                            },
                            InputMode::None => {
                                let mut art_moved = false;
                                if self.loaded_art.is_some() {
                                    match key_event.code {
                                        KeyCode::Up => {
                                            self.loaded_art.as_mut().unwrap().board_y = self
                                                .loaded_art
                                                .as_mut()
                                                .unwrap()
                                                .board_y
                                                .saturating_sub(1);
                                            art_moved = true;
                                        }
                                        KeyCode::Down => {
                                            self.loaded_art.as_mut().unwrap().board_y = self
                                                .loaded_art
                                                .as_mut()
                                                .unwrap()
                                                .board_y
                                                .saturating_add(1);
                                            art_moved = true;
                                        }
                                        KeyCode::Left => {
                                            self.loaded_art.as_mut().unwrap().board_x = self
                                                .loaded_art
                                                .as_mut()
                                                .unwrap()
                                                .board_x
                                                .saturating_sub(1);
                                            art_moved = true;
                                        }
                                        KeyCode::Right => {
                                            self.loaded_art.as_mut().unwrap().board_x = self
                                                .loaded_art
                                                .as_mut()
                                                .unwrap()
                                                .board_x
                                                .saturating_add(1);
                                            art_moved = true;
                                        }
                                        KeyCode::Enter => {
                                            self.place_loaded_art().await;
                                        }
                                        _ => {}
                                    }
                                    if art_moved {
                                        let art = self.loaded_art.as_ref().unwrap();
                                        self.status_message = format!(
                                            "Art '{}' at ({}, {}). Press Enter to place.",
                                            art.name, art.board_x, art.board_y
                                        );
                                    }
                                }

                                if !art_moved {
                                    match key_event.code {
                                        KeyCode::Up => {
                                            self.board_viewport_y =
                                                self.board_viewport_y.saturating_sub(10)
                                        }
                                        KeyCode::Down => {
                                            self.board_viewport_y =
                                                self.board_viewport_y.saturating_add(10)
                                        }
                                        KeyCode::Left => {
                                            self.board_viewport_x =
                                                self.board_viewport_x.saturating_sub(5)
                                        }
                                        KeyCode::Right => {
                                            self.board_viewport_x =
                                                self.board_viewport_x.saturating_add(5)
                                        }
                                        KeyCode::Char('q') => self.exit = true,
                                        KeyCode::Char('c') => {
                                            self.input_mode = InputMode::EnterAccessToken;
                                            self.status_message = "Re-enter Access Token (current will be overwritten if new is provided, skip Refresh Token step if not needed):".to_string();
                                            self.input_buffer.clear();
                                        }
                                        KeyCode::Char('r') => self.fetch_board_data().await,
                                        KeyCode::Char('p') => self.fetch_profile_data().await,
                                        KeyCode::Char('l') => {
                                            self.loaded_art = Some(load_default_pixel_art());
                                            if let Some(art) = &self.loaded_art {
                                                self.status_message = format!(
                                                    "Loaded art: '{}'. Use arrow keys to move. Board scroll disabled.",
                                                    art.name
                                                );
                                            } else {
                                                self.status_message =
                                                    "Failed to load art.".to_string();
                                            }
                                        }
                                        KeyCode::Char('e') => {
                                            self.input_mode = InputMode::ArtEditor;
                                            if self.current_editing_art.is_none() {
                                                self.current_editing_art = Some(PixelArt {
                                                    name: "NewArt".to_string(),
                                                    pixels: Vec::new(),
                                                    board_x: 0,
                                                    board_y: 0,
                                                });
                                                self.art_editor_cursor_x = 0;
                                                self.art_editor_cursor_y = 0;
                                                self.art_editor_selected_color_id = 1;
                                            }
                                            self.status_message = format!(
                                                "Entered Pixel Art Editor. Canvas: {}x{}. Arrows to move, Space to draw, s to save.",
                                                self.art_editor_canvas_width, self.art_editor_canvas_height
                                            );
                                        }
                                        KeyCode::Char('?') => {
                                            self.input_mode = InputMode::ShowHelp;
                                            self.status_message =
                                                "Showing help. Press Esc or q to close."
                                                    .to_string();
                                        }
                                        KeyCode::Char('i') => {
                                            self.input_mode = InputMode::ShowProfile;
                                            self.status_message =
                                                "Showing user profile. Press Esc, q, or i to close."
                                                    .to_string();
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            InputMode::ArtEditor => match key_event.code {
                                KeyCode::Esc => {
                                    self.input_mode = InputMode::None;
                                    self.status_message =
                                        "Exited Pixel Art Editor. Changes not saved.".to_string();
                                }
                                KeyCode::Up => {
                                    self.art_editor_cursor_y =
                                        self.art_editor_cursor_y.saturating_sub(1).max(0);
                                }
                                KeyCode::Down => {
                                    self.art_editor_cursor_y = self
                                        .art_editor_cursor_y
                                        .saturating_add(1)
                                        .min(self.art_editor_canvas_height as i32 - 1);
                                }
                                KeyCode::Left => {
                                    self.art_editor_cursor_x =
                                        self.art_editor_cursor_x.saturating_sub(1).max(0);
                                }
                                KeyCode::Right => {
                                    self.art_editor_cursor_x = self
                                        .art_editor_cursor_x
                                        .saturating_add(1)
                                        .min(self.art_editor_canvas_width as i32 - 1);
                                }
                                KeyCode::Char(' ') => {
                                    if let Some(art) = &mut self.current_editing_art {
                                        art.pixels.retain(|p| {
                                            p.x != self.art_editor_cursor_x
                                                || p.y != self.art_editor_cursor_y
                                        });
                                        art.pixels.push(ArtPixel {
                                            x: self.art_editor_cursor_x,
                                            y: self.art_editor_cursor_y,
                                            color_id: self.art_editor_selected_color_id,
                                        });
                                        self.status_message = format!(
                                            "Drew pixel at ({}, {}) with color {}.",
                                            self.art_editor_cursor_x,
                                            self.art_editor_cursor_y,
                                            self.art_editor_selected_color_id
                                        );
                                    }
                                }
                                KeyCode::Char('s') => {
                                    if self.current_editing_art.is_some() {
                                        self.input_mode = InputMode::ArtEditorFileName;
                                        self.art_editor_filename_buffer.clear();
                                        self.status_message = "Enter filename to save art (e.g., my_art.json). Press Enter to save, Esc to cancel.".to_string();
                                    } else {
                                        self.status_message = "No art to save.".to_string();
                                    }
                                }
                                KeyCode::Backspace => {
                                    self.art_editor_filename_buffer.pop();
                                }
                                _ => {}
                            },
                            InputMode::ArtEditorFileName => match key_event.code {
                                KeyCode::Enter => {
                                    let filename =
                                        self.art_editor_filename_buffer.trim().to_string();
                                    if !filename.is_empty() {
                                        self.save_current_art_to_file(filename).await;
                                        self.input_mode = InputMode::ArtEditor;
                                    } else {
                                        self.status_message = "Filename cannot be empty. Press Esc to cancel or type a name.".to_string();
                                    }
                                }
                                KeyCode::Esc => {
                                    self.input_mode = InputMode::ArtEditor;
                                    self.status_message = "Save art cancelled.".to_string();
                                    self.art_editor_filename_buffer.clear();
                                }
                                KeyCode::Char(to_insert) => {
                                    self.art_editor_filename_buffer.push(to_insert);
                                }
                                KeyCode::Backspace => {
                                    self.art_editor_filename_buffer.pop();
                                }
                                _ => {}
                            },
                            InputMode::ShowHelp => match key_event.code {
                                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => {
                                    self.input_mode = InputMode::None; // Or store and revert to previous mode
                                    self.status_message = "Help closed.".to_string();
                                }
                                _ => {}
                            },
                            InputMode::ShowProfile => match key_event.code {
                                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('i') => {
                                    self.input_mode = InputMode::None;
                                    self.status_message = "Profile closed.".to_string();
                                }
                                _ => {}
                            },
                        }
                    }
                }
                _ => { /* Other events */ }
            }
        }
        Ok(())
    }

    async fn fetch_board_data(&mut self) {
        self.status_message = "Fetching board data...".to_string();
        match self.api_client.get_board().await {
            Ok(board_response) => {
                self.board = board_response.board;
                self.colors = board_response.colors;
                self.status_message = format!(
                    "Board data fetched. {} colors. Board size: {}x{}. Arrows to scroll.",
                    self.colors.len(),
                    self.board.len(),
                    if self.board.is_empty() {
                        0
                    } else {
                        self.board[0].len()
                    }
                );
                self.last_board_refresh = Some(Instant::now());
                if !self.initial_board_fetched {
                    self.initial_board_fetched = true;
                }
                // Save tokens in case they were refreshed during the API call
                self.save_tokens();
            }
            Err(e) => {
                match e {
                    ApiError::Unauthorized => {
                        self.status_message = "Session expired or cookie invalid. Auto-refresh paused. Enter new tokens or restart.".to_string();
                        self.api_client.clear_tokens();
                        // Clear saved tokens when session expires
                        self.clear_saved_tokens();
                    }
                    _ => {
                        // Use enhanced error display for API errors
                        self.handle_api_error_with_enhanced_display("Error fetching board", &e)
                            .await;
                        self.status_message.push_str(" Try 'r' to refresh.");
                    }
                }
                self.last_board_refresh = Some(Instant::now());
            }
        }
    }

    async fn fetch_profile_data(&mut self) {
        if self.api_client.get_auth_cookie_preview().is_none() {
            self.status_message =
                "Cannot fetch profile: Access Token not set. Please enter it.".to_string();
            self.input_mode = InputMode::EnterAccessToken;
            self.input_buffer.clear();
            return;
        }
        self.status_message = "Fetching profile data...".to_string();
        match self.api_client.get_profile().await {
            Ok(profile_response) => {
                let info = profile_response.user_infos;
                self.status_message = format!(
                    "Profile: {}, Pixels: {}, Cooldown: {}s, User Timers: {}",
                    info.username.as_deref().unwrap_or("N/A"),
                    info.pixel_buffer,
                    info.pixel_timer,
                    info.timers.as_ref().map_or(0, |v| v.len())
                );
                self.user_info = Some(info);
                // Save tokens in case they were refreshed during the API call
                self.save_tokens();
            }
            Err(e) => {
                self.user_info = None;
                match e {
                    ApiError::Unauthorized => {
                        self.status_message = "Error fetching profile: Unauthorized. Access Token might be invalid or expired. Try 'c' to update.".to_string();
                    }
                    _ => {
                        // Use enhanced error display for API errors
                        self.handle_api_error_with_enhanced_display("Error fetching profile", &e)
                            .await;
                    }
                }
            }
        }
    }

    async fn place_loaded_art(&mut self) {
        if self.loaded_art.is_none() {
            self.status_message = "No art loaded to place.".to_string();
            return;
        }
        if self.api_client.get_auth_cookie_preview().is_none() {
            self.status_message =
                "Cannot place pixels: Access Token not set. Use --access-token CLI arg or 'c' to set token.".to_string();
            return;
        }

        let art_to_place = self.loaded_art.clone().unwrap();
        let total_pixels = art_to_place.pixels.len();
        self.status_message = format!(
            "Starting to place art '{}' ({} pixels)...",
            art_to_place.name, total_pixels
        );

        for (index, art_pixel) in art_to_place.pixels.iter().enumerate() {
            let abs_x = art_to_place.board_x + art_pixel.x;
            let abs_y = art_to_place.board_y + art_pixel.y;

            self.status_message = format!(
                "Placing pixel {}/{} ('{}') at ({},{}) with color_id {}...",
                index + 1,
                total_pixels,
                art_to_place.name,
                abs_x,
                abs_y,
                art_pixel.color_id
            );

            if let Some(u_info) = &self.user_info {
                if u_info.pixel_buffer <= 0 && u_info.pixel_timer > 0 {
                    self.status_message = format!(
                        "Cooldown active: waiting {}s before placing pixel {}/{}.",
                        u_info.pixel_timer,
                        index + 1,
                        total_pixels
                    );
                    tokio::time::sleep(Duration::from_secs(u_info.pixel_timer as u64)).await;
                }
            }

            match self
                .api_client
                .place_pixel(abs_x, abs_y, art_pixel.color_id)
                .await
            {
                Ok(response) => {
                    self.status_message = format!(
                        "Pixel {}/{} placed at ({},{}). Next CD: {}s, Buf: {}. User Timers: {}.",
                        index + 1,
                        total_pixels,
                        abs_x,
                        abs_y,
                        response.user_infos.pixel_timer,
                        response.user_infos.pixel_buffer,
                        response.user_infos.timers.as_ref().map_or(0, |v| v.len())
                    );
                    self.user_info = Some(response.user_infos);
                }
                Err(e) => {
                    // Use enhanced error display that utilizes timers and interval fields
                    let base_message = format!(
                        "Error placing pixel {}/{} at ({},{})",
                        index + 1,
                        total_pixels,
                        abs_x,
                        abs_y
                    );

                    // Check if it's unauthorized first for token clearing
                    match &e {
                        ApiError::Unauthorized
                        | ApiError::ApiErrorResponse {
                            status: reqwest::StatusCode::UNAUTHORIZED,
                            ..
                        } => {
                            self.api_client.clear_tokens();
                            self.status_message = format!(
                                "{}: Authentication failed. Tokens cleared. Please restart with valid tokens. Halting placement.",
                                base_message
                            );
                            return;
                        }
                        _ => {}
                    }

                    // Use enhanced error display for all other errors
                    self.handle_api_error_with_enhanced_display(&base_message, &e)
                        .await;

                    // For rate limiting errors, don't halt placement - let enhanced display handle it
                    if let ApiError::ApiErrorResponse { status, .. } = &e {
                        if status == &reqwest::StatusCode::TOO_MANY_REQUESTS
                            || status.as_u16() == 425
                            || status.as_u16() == 420
                        {
                            // Enhanced display already updated user timers, just refresh board
                            self.fetch_board_data().await;
                        }
                    }

                    return;
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        self.status_message = format!(
            "Finished placing art '{}'. Refreshing board...",
            art_to_place.name
        );
        self.fetch_board_data().await;
    }

    async fn save_current_art_to_file(&mut self, filename: String) {
        if let Some(art) = &self.current_editing_art {
            let art_with_name = PixelArt {
                name: art.name.clone(),
                pixels: art.pixels.clone(),
                board_x: 0,
                board_y: 0,
            };
            match serde_json::to_string_pretty(&art_with_name) {
                Ok(json_data) => {
                    let dir_path = Path::new("pixel_arts");
                    if !dir_path.exists() {
                        if let Err(e) = std::fs::create_dir_all(dir_path) {
                            self.status_message =
                                format!("Error creating directory pixel_arts: {}", e);
                            return;
                        }
                    }
                    let file_path = dir_path.join(if filename.ends_with(".json") {
                        filename
                    } else {
                        format!("{}.json", filename)
                    });
                    match File::create(&file_path) {
                        Ok(mut file) => {
                            if let Err(e) = file.write_all(json_data.as_bytes()) {
                                self.status_message =
                                    format!("Error writing to file {}: {}", file_path.display(), e);
                            } else {
                                self.status_message =
                                    format!("Art saved to {}", file_path.display());
                            }
                        }
                        Err(e) => {
                            self.status_message =
                                format!("Error creating file {}: {}", file_path.display(), e);
                        }
                    }
                }
                Err(e) => {
                    self.status_message = format!("Error serializing art to JSON: {}", e);
                }
            }
        } else {
            self.status_message = "No current art to save.".to_string();
        }
    }

    /// Save current tokens and base URL to persistent storage
    fn save_tokens(&mut self) {
        let token_data = crate::token_storage::TokenData {
            access_token: self.api_client.get_access_token_clone(),
            refresh_token: self.api_client.get_refresh_token_clone(),
            base_url: Some(self.api_client.get_base_url()),
        };

        if let Err(e) = self.token_storage.save(&token_data) {
            eprintln!("Warning: Could not save tokens: {}", e);
        }
    }

    /// Clear saved tokens from persistent storage
    fn clear_saved_tokens(&mut self) {
        if let Err(e) = self.token_storage.clear() {
            eprintln!("Warning: Could not clear saved tokens: {}", e);
        }
    }

    /// Check if tokens were refreshed and save them if needed
    async fn check_and_save_refreshed_tokens(&mut self) {
        // This will be called after API operations that might refresh tokens
        self.save_tokens();
    }

    /// Enhanced error message formatting that utilizes timers and interval from ApiErrorResponse
    fn format_enhanced_error_message(
        &self,
        base_message: &str,
        status: &reqwest::StatusCode,
        error_response: &crate::api_client::ApiErrorResponse,
    ) -> String {
        let mut enhanced_message = format!("{}: {}", base_message, error_response.message);

        // Add timer information if available
        if let Some(timers) = &error_response.timers {
            if !timers.is_empty() {
                enhanced_message.push_str(" | Active Timers: ");
                let timer_strings: Vec<String> = timers
                    .iter()
                    .enumerate()
                    .map(|(i, timer)| {
                        let time_remaining = (*timer as f64 / 1000.0)
                            - (chrono::Utc::now().timestamp_millis() as f64 / 1000.0);
                        if time_remaining > 0.0 {
                            format!("T{}({:.1}s)", i + 1, time_remaining)
                        } else {
                            format!("T{}(expired)", i + 1)
                        }
                    })
                    .collect();
                enhanced_message.push_str(&timer_strings.join(", "));
            }
        }

        // Add interval information for cooldown errors
        if let Some(interval) = error_response.interval {
            let interval_seconds = interval as f64 / 1000.0;
            enhanced_message.push_str(&format!(" | Retry Interval: {:.1}s", interval_seconds));
        }

        // Add specific guidance based on status code
        match status.as_u16() {
            420 => enhanced_message.push_str(" | Status: Enhance Your Hype (cooldown active)"),
            425 => enhanced_message.push_str(" | Status: Too Early (rate limited)"),
            429 => enhanced_message.push_str(" | Status: Too Many Requests (rate limited)"),
            _ => {}
        }

        enhanced_message
    }

    /// Enhanced error handling for API operations that uses the ApiErrorResponse fields
    async fn handle_api_error_with_enhanced_display(
        &mut self,
        base_message: &str,
        error: &crate::api_client::ApiError,
    ) {
        match error {
            crate::api_client::ApiError::ApiErrorResponse {
                status,
                error_response,
            } => {
                let enhanced_message =
                    self.format_enhanced_error_message(base_message, status, error_response);

                // For rate limiting errors, update user info with new timers if available
                if *status == reqwest::StatusCode::TOO_MANY_REQUESTS
                    || status.as_u16() == 425
                    || status.as_u16() == 420
                {
                    if let Some(timers) = &error_response.timers {
                        if let Some(user_info) = &mut self.user_info {
                            user_info.timers = Some(timers.clone());
                        }
                    }
                }

                self.status_message = enhanced_message;
            }
            crate::api_client::ApiError::Unauthorized => {
                self.status_message = format!(
                    "{}: Unauthorized access. Please check your tokens.",
                    base_message
                );
                self.api_client.clear_tokens();
            }
            _ => {
                self.status_message = format!("{}: {:?}", base_message, error);
            }
        }
    }
}
