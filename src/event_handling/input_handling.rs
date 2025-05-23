use crate::app_state::{App, InputMode};
use crate::art::{get_available_pixel_arts, ArtPixel, PixelArt};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use std::io;
use std::time::Duration;

impl App {
    pub async fn handle_events(&mut self) -> io::Result<()> {
        // Check if we need to fetch board on startup (when tokens were restored)
        if self.should_fetch_board_on_start {
            self.should_fetch_board_on_start = false; // Clear the flag
            self.trigger_board_fetch();
        }

        // Update blink state for queue previews
        self.update_blink_state();

        // Update cooldown status with current timer info
        self.update_cooldown_status();

        // Clean up old status messages
        self.cleanup_old_status_messages();

        // Check for completed board fetches
        if let Some(receiver) = &mut self.board_fetch_receiver {
            if let Ok(result) = receiver.try_recv() {
                self.handle_board_fetch_result(result);
            }
        }

        // Check for placement updates
        if let Some(receiver) = &mut self.placement_receiver {
            if let Ok(update) = receiver.try_recv() {
                self.handle_placement_update(update);
            }
        }

        // Check for queue processing updates
        if let Some(receiver) = &mut self.queue_receiver {
            if let Ok(update) = receiver.try_recv() {
                self.handle_queue_update(update);
            }
        }

        // Check for profile fetch results
        if let Some(receiver) = &mut self.profile_receiver {
            if let Ok(result) = receiver.try_recv() {
                self.handle_profile_fetch_result(result);
            }
        }

        let mut should_refresh_board = false;
        if self.input_mode == InputMode::None
            && self.initial_board_fetched
            && self.api_client.get_auth_cookie_preview().is_some()
            && !self.board_loading
        // Don't trigger refresh if already loading
        {
            if let Some(last_refresh) = self.last_board_refresh {
                if last_refresh.elapsed() >= Duration::from_secs(10) {
                    should_refresh_board = true;
                }
            }
        }

        if should_refresh_board {
            self.add_status_message("Auto-refreshing board...".to_string());
            self.trigger_board_fetch();
        }

        // Check for user input first - only process board loading if no input is pending
        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key_event) => {
                    if key_event.kind == KeyEventKind::Press {
                        self.handle_key_input(key_event.code).await?;
                    }
                }
                _ => { /* Other events */ }
            }
        } else {
            // No pending input events - all processing happens via async channels now
            // Board fetches are spawned as background tasks and results come via channels
        }
        Ok(())
    }

    async fn handle_key_input(&mut self, key_code: KeyCode) -> io::Result<()> {
        match self.input_mode {
            InputMode::EnterBaseUrl => {
                self.handle_base_url_input(key_code).await?;
            }
            InputMode::EnterCustomBaseUrlText => {
                self.handle_custom_base_url_input(key_code).await?;
            }
            InputMode::EnterAccessToken => {
                self.handle_access_token_input(key_code).await?;
            }
            InputMode::EnterRefreshToken => {
                self.handle_refresh_token_input(key_code).await?;
            }
            InputMode::None => {
                self.handle_main_input(key_code).await?;
            }
            InputMode::ArtEditor => {
                self.handle_art_editor_input(key_code).await?;
            }
            InputMode::ShowHelp => {
                self.handle_help_input(key_code);
            }
            InputMode::ShowProfile => {
                self.handle_profile_input(key_code);
            }
            InputMode::ArtEditorNewArtName => {
                self.handle_new_art_name_input(key_code);
            }
            InputMode::ArtSelection => {
                self.handle_art_selection_input(key_code);
            }
            InputMode::ArtQueue => {
                self.handle_queue_input(key_code).await?;
            }
        }
        Ok(())
    }

    async fn handle_base_url_input(&mut self, key_code: KeyCode) -> io::Result<()> {
        match key_code {
            KeyCode::Up => {
                if self.base_url_selection_index > 0 {
                    self.base_url_selection_index -= 1;
                }
            }
            KeyCode::Down => {
                if self.base_url_selection_index < self.base_url_options.len() - 1 {
                    self.base_url_selection_index += 1;
                }
            }
            KeyCode::Enter => {
                let selected_option = &self.base_url_options[self.base_url_selection_index];
                if selected_option == "Custom" {
                    self.input_mode = InputMode::EnterCustomBaseUrlText;
                    self.status_message = "Enter Custom API Base URL:".to_string();
                    self.input_buffer.clear();
                } else {
                    self.api_client.set_base_url(selected_option.clone());
                    self.input_mode = InputMode::EnterAccessToken;
                    self.status_message =
                        "Base URL set. Enter Access Token (or Enter to skip):".to_string();
                    self.input_buffer.clear();
                    // Save the base URL immediately
                    self.save_tokens();
                }
            }
            KeyCode::Char('q') => self.exit = true,
            _ => {}
        }
        Ok(())
    }

    async fn handle_custom_base_url_input(&mut self, key_code: KeyCode) -> io::Result<()> {
        match key_code {
            KeyCode::Enter => {
                let url = self.input_buffer.trim().to_string();
                if url.is_empty() || !(url.starts_with("http://") || url.starts_with("https://")) {
                    self.status_message = "Invalid URL. Must start with http:// or https://. Please re-enter Custom Base URL:".to_string();
                    self.input_buffer.clear();
                } else {
                    self.api_client.set_base_url(url);
                    self.input_mode = InputMode::EnterAccessToken;
                    self.status_message =
                        "Base URL set. Enter Access Token (or Enter to skip):".to_string();
                    self.input_buffer.clear();
                    // Save the base URL immediately
                    self.save_tokens();
                }
            }
            KeyCode::Esc => {
                self.input_mode = InputMode::EnterBaseUrl;
                self.status_message =
                    "Custom URL input cancelled. Select API Base URL:".to_string();
                self.input_buffer.clear();
            }
            KeyCode::Char(to_insert) => self.input_buffer.push(to_insert),
            KeyCode::Backspace => {
                self.input_buffer.pop();
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_access_token_input(&mut self, key_code: KeyCode) -> io::Result<()> {
        match key_code {
            KeyCode::Enter => {
                let token = self.input_buffer.trim().to_string();
                let current_refresh = self.api_client.get_refresh_token_clone();
                if !token.is_empty() {
                    self.api_client.set_tokens(Some(token), current_refresh);
                    self.status_message =
                        "Access Token set. Enter Refresh Token (or Enter to skip):".to_string();
                } else {
                    self.api_client.set_tokens(None, current_refresh);
                    self.status_message =
                        "Access Token skipped. Enter Refresh Token (or Enter to skip):".to_string();
                }
                self.input_mode = InputMode::EnterRefreshToken;
                self.input_buffer.clear();
                // Save tokens after setting access token
                self.save_tokens();
            }
            KeyCode::Esc => {
                self.input_mode = InputMode::EnterBaseUrl;
                self.status_message = "Token input cancelled. Select API Base URL:".to_string();
                self.input_buffer.clear();
            }
            KeyCode::Char(to_insert) => self.input_buffer.push(to_insert),
            KeyCode::Backspace => {
                self.input_buffer.pop();
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_refresh_token_input(&mut self, key_code: KeyCode) -> io::Result<()> {
        match key_code {
            KeyCode::Enter => {
                let refresh = self.input_buffer.trim().to_string();
                let current_access = self.api_client.get_access_token_clone();
                if !refresh.is_empty() {
                    self.api_client.set_tokens(current_access, Some(refresh));
                    self.status_message =
                        "Refresh Token set. Configuration complete. Fetching initial board..."
                            .to_string();
                } else {
                    self.api_client.set_tokens(current_access, None);
                    self.status_message =
                        "Refresh Token skipped. Configuration complete. Fetching initial board..."
                            .to_string();
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
                    "Refresh Token input cancelled. Re-enter Access Token:".to_string();
                self.input_buffer.clear();
            }
            KeyCode::Char(to_insert) => self.input_buffer.push(to_insert),
            KeyCode::Backspace => {
                self.input_buffer.pop();
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_main_input(&mut self, key_code: KeyCode) -> io::Result<()> {
        let mut art_moved = false;
        if self.loaded_art.is_some() {
            match key_code {
                KeyCode::Up => {
                    self.loaded_art.as_mut().unwrap().board_y =
                        self.loaded_art.as_mut().unwrap().board_y.saturating_sub(1);
                    art_moved = true;
                }
                KeyCode::Down => {
                    self.loaded_art.as_mut().unwrap().board_y =
                        self.loaded_art.as_mut().unwrap().board_y.saturating_add(1);
                    art_moved = true;
                }
                KeyCode::Left => {
                    self.loaded_art.as_mut().unwrap().board_x =
                        self.loaded_art.as_mut().unwrap().board_x.saturating_sub(1);
                    art_moved = true;
                }
                KeyCode::Right => {
                    self.loaded_art.as_mut().unwrap().board_x =
                        self.loaded_art.as_mut().unwrap().board_x.saturating_add(1);
                    art_moved = true;
                }
                KeyCode::Enter => {
                    // Add loaded art to queue and start processing
                    if let Some(art) = &self.loaded_art {
                        // Add art to queue at current position
                        self.add_art_to_queue(art.clone()).await;

                        // Start queue processing immediately
                        if !self.queue_processing {
                            self.trigger_queue_processing();
                        }
                    } else {
                        self.status_message = "No art loaded to place.".to_string();
                    }
                }
                KeyCode::Esc => {
                    if self.placement_in_progress {
                        // Cancel ongoing placement
                        self.placement_cancel_requested = true;
                        self.placement_in_progress = false;
                        self.placement_start = None;
                        self.placement_receiver = None;
                        self.status_message = "Art placement cancelled.".to_string();
                    } else {
                        // Cancel loaded art
                        self.loaded_art = None;
                        self.status_message =
                            "Loaded art cancelled. Board scroll re-enabled.".to_string();
                    }
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
            match key_code {
                KeyCode::Up => self.board_viewport_y = self.board_viewport_y.saturating_sub(10),
                KeyCode::Down => self.board_viewport_y = self.board_viewport_y.saturating_add(10),
                KeyCode::Left => self.board_viewport_x = self.board_viewport_x.saturating_sub(5),
                KeyCode::Right => self.board_viewport_x = self.board_viewport_x.saturating_add(5),
                KeyCode::Char('q') => self.exit = true,
                KeyCode::Char('c') => {
                    self.input_mode = InputMode::EnterAccessToken;
                    self.status_message = "Re-enter Access Token (current will be overwritten if new is provided, skip Refresh Token step if not needed):".to_string();
                    self.input_buffer.clear();
                }
                KeyCode::Char('r') => self.trigger_board_fetch(),
                KeyCode::Char('p') => self.trigger_profile_fetch(),
                KeyCode::Char('l') => {
                    // Open art selection to add more arts
                    self.available_pixel_arts = get_available_pixel_arts();
                    if !self.available_pixel_arts.is_empty() {
                        self.input_mode = InputMode::ArtSelection;
                        self.art_selection_index = 0;
                        self.status_message = format!(
                            "Select pixel art to load for positioning ({} available).",
                            self.available_pixel_arts.len()
                        );
                    } else {
                        self.status_message =
                            "No pixel arts available. Create some first with 'e'.".to_string();
                    }
                }
                KeyCode::Char('e') => {
                    // Start by asking for art name
                    self.input_mode = InputMode::ArtEditorNewArtName;
                    self.input_buffer.clear();
                    self.status_message = "Enter name for new pixel art:".to_string();
                }
                KeyCode::Char('?') => {
                    self.input_mode = InputMode::ShowHelp;
                    self.status_message = "Showing help. Press Esc or q to close.".to_string();
                }
                KeyCode::Char('i') => {
                    self.input_mode = InputMode::ShowProfile;
                    self.status_message =
                        "Showing user profile. Press Esc, q, or i to close.".to_string();
                }
                KeyCode::Char('w') => {
                    // Open work queue management
                    self.input_mode = InputMode::ArtQueue;
                    self.status_message =
                        "Work Queue Management. Use arrows to navigate, Enter to start processing."
                            .to_string();
                }
                _ => {}
            }
        }
        Ok(())
    }

    async fn handle_art_editor_input(&mut self, key_code: KeyCode) -> io::Result<()> {
        match key_code {
            KeyCode::Esc => {
                self.input_mode = InputMode::None;
                self.status_message = "Exited Pixel Art Editor. Changes not saved.".to_string();
            }
            KeyCode::Up => {
                self.art_editor_cursor_y = self.art_editor_cursor_y.saturating_sub(1).max(0);
            }
            KeyCode::Down => {
                self.art_editor_cursor_y = self
                    .art_editor_cursor_y
                    .saturating_add(1)
                    .min(self.art_editor_canvas_height as i32 - 1);
            }
            KeyCode::Left => {
                self.art_editor_cursor_x = self.art_editor_cursor_x.saturating_sub(1).max(0);
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
                        p.x != self.art_editor_cursor_x || p.y != self.art_editor_cursor_y
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
                    // Auto-save with the art's name instead of prompting for filename
                    if let Some(art) = &self.current_editing_art {
                        let filename = format!("{}.json", art.name);
                        self.save_current_art_to_file(filename).await;
                    }
                } else {
                    self.status_message = "No art to save.".to_string();
                }
            }
            KeyCode::Tab => {
                // Navigate to next color in palette
                if !self.colors.is_empty() {
                    self.art_editor_color_palette_index =
                        (self.art_editor_color_palette_index + 1) % self.colors.len();

                    // Update selected color to match palette index
                    if let Some(color) = self.colors.get(self.art_editor_color_palette_index) {
                        self.art_editor_selected_color_id = color.id;
                        let color_name = if color.name.trim().is_empty() {
                            format!("Color {}", color.id)
                        } else {
                            color.name.clone()
                        };
                        self.status_message = format!("Selected color: {}", color_name);
                    }
                }
            }
            KeyCode::BackTab => {
                // Navigate to previous color in palette
                if !self.colors.is_empty() {
                    self.art_editor_color_palette_index =
                        if self.art_editor_color_palette_index == 0 {
                            self.colors.len() - 1
                        } else {
                            self.art_editor_color_palette_index - 1
                        };

                    // Update selected color to match palette index
                    if let Some(color) = self.colors.get(self.art_editor_color_palette_index) {
                        self.art_editor_selected_color_id = color.id;
                        let color_name = if color.name.trim().is_empty() {
                            format!("Color {}", color.id)
                        } else {
                            color.name.clone()
                        };
                        self.status_message = format!("Selected color: {}", color_name);
                    }
                }
            }
            KeyCode::Backspace => {
                // No action needed for backspace in art editor
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_help_input(&mut self, key_code: KeyCode) {
        match key_code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => {
                self.input_mode = InputMode::None; // Or store and revert to previous mode
                self.status_message = "Help closed.".to_string();
            }
            _ => {}
        }
    }

    fn handle_profile_input(&mut self, key_code: KeyCode) {
        match key_code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('i') => {
                self.input_mode = InputMode::None;
                self.status_message = "Profile closed.".to_string();
            }
            _ => {}
        }
    }

    fn handle_new_art_name_input(&mut self, key_code: KeyCode) {
        match key_code {
            KeyCode::Enter => {
                let name = self.input_buffer.trim().to_string();
                if !name.is_empty() {
                    self.current_editing_art = Some(PixelArt {
                        name,
                        pixels: Vec::new(),
                        board_x: 0,
                        board_y: 0,
                    });
                    self.input_mode = InputMode::ArtEditor;
                    self.status_message = format!(
						"Entered Pixel Art Editor. Canvas: {}x{}. Arrows to move, Space to draw, Tab to change colors, s to save.",
						self.art_editor_canvas_width, self.art_editor_canvas_height
					);

                    // Initialize editor state
                    self.art_editor_cursor_x = 0;
                    self.art_editor_cursor_y = 0;
                    self.art_editor_selected_color_id = 1;

                    // Sync color palette index with selected color
                    if let Some(index) = self
                        .colors
                        .iter()
                        .position(|c| c.id == self.art_editor_selected_color_id)
                    {
                        self.art_editor_color_palette_index = index;
                    } else {
                        self.art_editor_color_palette_index = 0;
                        if let Some(first_color) = self.colors.first() {
                            self.art_editor_selected_color_id = first_color.id;
                        }
                    }
                } else {
                    self.status_message = "Name cannot be empty. Please enter a name.".to_string();
                }
            }
            KeyCode::Esc => {
                self.input_mode = InputMode::None;
                self.status_message = "New art name input cancelled.".to_string();
            }
            KeyCode::Char(to_insert) => self.input_buffer.push(to_insert),
            KeyCode::Backspace => {
                self.input_buffer.pop();
            }
            _ => {}
        }
    }

    fn handle_art_selection_input(&mut self, key_code: KeyCode) {
        match key_code {
            KeyCode::Up => {
                if self.art_selection_index > 0 {
                    self.art_selection_index -= 1;
                }
            }
            KeyCode::Down => {
                if self.art_selection_index < self.available_pixel_arts.len() - 1 {
                    self.art_selection_index += 1;
                }
            }
            KeyCode::Enter => {
                if let Some(selected_art) = self
                    .available_pixel_arts
                    .get(self.art_selection_index)
                    .cloned()
                {
                    // Load art for positioning
                    self.loaded_art = Some(selected_art.clone());
                    self.input_mode = InputMode::None;
                    self.status_message = format!(
                        "Loaded art: '{}'. Use arrows to position, Enter to add to queue.",
                        selected_art.name
                    );
                }
            }
            KeyCode::Esc => {
                self.input_mode = InputMode::None;
                self.status_message = "Art selection cancelled.".to_string();
            }
            KeyCode::Char('q') => self.exit = true,
            _ => {}
        }
    }

    async fn handle_queue_input(&mut self, key_code: KeyCode) -> io::Result<()> {
        match key_code {
            KeyCode::Up => {
                if self.queue_selection_index > 0 {
                    self.queue_selection_index -= 1;
                }
            }
            KeyCode::Down => {
                if self.queue_selection_index < self.art_queue.len().saturating_sub(1) {
                    self.queue_selection_index += 1;
                }
            }
            KeyCode::Char('u') | KeyCode::Char('k') => {
                // Move selected item up in priority
                if !self.art_queue.is_empty()
                    && self.queue_selection_index < self.art_queue.len()
                    && self.queue_selection_index > 0
                {
                    self.art_queue
                        .swap(self.queue_selection_index - 1, self.queue_selection_index);
                    self.queue_selection_index -= 1;
                    self.status_message = format!(
                        "Moved '{}' up in queue",
                        self.art_queue[self.queue_selection_index].art.name
                    );
                }
            }
            KeyCode::Char('j') | KeyCode::Char('n') => {
                // Move selected item down in priority
                if !self.art_queue.is_empty()
                    && self.queue_selection_index < self.art_queue.len().saturating_sub(1)
                {
                    self.art_queue
                        .swap(self.queue_selection_index, self.queue_selection_index + 1);
                    self.queue_selection_index += 1;
                    self.status_message = format!(
                        "Moved '{}' down in queue",
                        self.art_queue[self.queue_selection_index].art.name
                    );
                }
            }
            KeyCode::Enter => {
                // Start queue processing
                if !self.art_queue.is_empty() {
                    self.input_mode = InputMode::None;
                    self.trigger_queue_processing();
                } else {
                    self.status_message =
                        "Queue is empty. Press 'l' to add arts first.".to_string();
                }
            }
            KeyCode::Char('c') => {
                // Clear queue
                self.art_queue.clear();
                self.queue_selection_index = 0;
                self.status_message = "Queue cleared.".to_string();
            }
            KeyCode::Delete | KeyCode::Char('d') => {
                // Remove selected item from queue
                if !self.art_queue.is_empty() && self.queue_selection_index < self.art_queue.len() {
                    let removed_art = self.art_queue.remove(self.queue_selection_index);
                    self.status_message = format!("Removed '{}' from queue.", removed_art.art.name);
                    if self.queue_selection_index >= self.art_queue.len()
                        && !self.art_queue.is_empty()
                    {
                        self.queue_selection_index = self.art_queue.len() - 1;
                    }
                }
            }
            KeyCode::Char('1'..='5') => {
                // Set priority for selected item
                if !self.art_queue.is_empty() && self.queue_selection_index < self.art_queue.len() {
                    let priority = match key_code {
                        KeyCode::Char('1') => 1,
                        KeyCode::Char('2') => 2,
                        KeyCode::Char('3') => 3,
                        KeyCode::Char('4') => 4,
                        KeyCode::Char('5') => 5,
                        _ => 3, // Default priority
                    };
                    self.art_queue[self.queue_selection_index].priority = priority;
                    self.sort_queue_by_priority();
                    self.status_message = format!(
                        "Set priority {} for '{}'",
                        priority, self.art_queue[self.queue_selection_index].art.name
                    );
                }
            }
            KeyCode::Esc => {
                self.input_mode = InputMode::None;
                self.status_message = "Queue management closed.".to_string();
            }
            KeyCode::Char('l') => {
                // Open art selection to add more arts
                self.available_pixel_arts = get_available_pixel_arts();
                if !self.available_pixel_arts.is_empty() {
                    self.input_mode = InputMode::ArtSelection;
                    self.art_selection_index = 0;
                    self.status_message = format!(
                        "Select pixel art to load for positioning ({} available).",
                        self.available_pixel_arts.len()
                    );
                } else {
                    self.status_message =
                        "No pixel arts available. Create some first with 'e'.".to_string();
                }
            }
            _ => {}
        }
        Ok(())
    }
}
