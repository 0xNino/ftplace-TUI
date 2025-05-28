use crate::app_state::{App, InputMode};
use crate::art::{get_available_pixel_arts, ArtPixel, PixelArt};
use crossterm::event::{
    self, Event, KeyCode, KeyEventKind, MouseButton, MouseEvent, MouseEventKind,
};
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
        if (self.input_mode == InputMode::None || self.input_mode == InputMode::ShowStatusLog)
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
            self.trigger_board_fetch();
        }

        // Check for user input first - only process board loading if no input is pending
        // Use shorter timeout when status log is open for more responsive updates
        let poll_timeout = if self.input_mode == InputMode::ShowStatusLog {
            Duration::from_millis(16) // ~60 FPS updates when status log is open
        } else {
            Duration::from_millis(50)
        };

        // Batch character input for better performance during paste operations
        let mut char_batch = String::new();
        let mut last_key_code = None;

        // Collect all pending character inputs
        while event::poll(Duration::from_millis(0))? {
            match event::read()? {
                Event::Key(key_event) => {
                    if key_event.kind == KeyEventKind::Press {
                        match key_event.code {
                            KeyCode::Char(c)
                                if matches!(
                                    self.input_mode,
                                    InputMode::EnterCustomBaseUrlText
                                        | InputMode::EnterAccessToken
                                        | InputMode::EnterRefreshToken
                                        | InputMode::ArtEditorNewArtName
                                ) =>
                            {
                                char_batch.push(c);
                            }
                            _ => {
                                // Process any batched characters first
                                if !char_batch.is_empty() {
                                    self.input_buffer.push_str(&char_batch);
                                    char_batch.clear();
                                }
                                // Then process the non-character key
                                self.handle_key_input(key_event.code).await?;
                                return Ok(()); // Exit early to render UI
                            }
                        }
                        last_key_code = Some(key_event.code);
                    }
                }
                _ => { /* Other events */ }
            }
        }

        // Process any remaining batched characters
        if !char_batch.is_empty() {
            self.input_buffer.push_str(&char_batch);
        }

        // If we only had character input and no other keys, we still processed input
        if last_key_code.is_some() && char_batch.is_empty() {
            // All input was processed above
        } else if event::poll(poll_timeout)? {
            match event::read()? {
                Event::Key(key_event) => {
                    if key_event.kind == KeyEventKind::Press {
                        self.handle_key_input(key_event.code).await?;
                    }
                }
                Event::Mouse(mouse_event) => {
                    self.handle_mouse_input(mouse_event).await?;
                    return Ok(()); // Exit early to render UI after mouse input
                }
                _ => { /* Other events */ }
            }
        } else {
            // No pending input events - all processing happens via async channels now
            // Board fetches are spawned as background tasks and results come via channels
        }
        Ok(())
    }

    async fn handle_mouse_input(&mut self, mouse_event: MouseEvent) -> io::Result<()> {
        // Only handle mouse events in main mode
        if self.input_mode != InputMode::None {
            return Ok(());
        }

        match mouse_event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some((board_x, board_y, board_width, board_height)) = self.board_area_bounds
                {
                    let mouse_x = mouse_event.column;
                    let mouse_y = mouse_event.row;

                    // Check if click is within board area
                    if mouse_x >= board_x
                        && mouse_x < board_x + board_width
                        && mouse_y >= board_y
                        && mouse_y < board_y + board_height
                    {
                        // Convert screen coordinates to board pixel coordinates
                        let screen_cell_x = mouse_x - board_x;
                        let screen_cell_y = mouse_y - board_y;

                        // Each screen cell represents 2 vertical pixels (due to half-block rendering)
                        let board_pixel_x = self.board_viewport_x as i32 + screen_cell_x as i32;
                        let board_pixel_y =
                            self.board_viewport_y as i32 + (screen_cell_y as i32 * 2);

                        if let Some(art) = &mut self.loaded_art {
                            // Get art dimensions to center it under the mouse cursor
                            let art_dimensions = crate::art::get_art_dimensions(art);
                            let art_center_offset_x = art_dimensions.0 / 2;
                            let art_center_offset_y = art_dimensions.1 / 2;

                            // Position art so its center is under the mouse cursor
                            art.board_x = board_pixel_x - art_center_offset_x;
                            art.board_y = board_pixel_y - art_center_offset_y;

                            self.status_message = format!(
                                "Art '{}' centered at ({}, {}) via mouse. Press Enter to place.",
                                art.name, art.board_x, art.board_y
                            );
                        } else {
                            // No art loaded - show coordinates for reference
                            self.status_message = format!(
                                "Clicked at board position ({}, {}). Load art with 'l' to place here.",
                                board_pixel_x, board_pixel_y
                            );
                        }
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                // Scroll up - move viewport up
                self.board_viewport_y = self.board_viewport_y.saturating_sub(15);
                self.status_message = format!(
                    "Scrolled up. Viewport at ({}, {})",
                    self.board_viewport_x, self.board_viewport_y
                );
            }
            MouseEventKind::ScrollDown => {
                // Scroll down - move viewport down
                self.board_viewport_y = self.board_viewport_y.saturating_add(15);
                self.status_message = format!(
                    "Scrolled down. Viewport at ({}, {})",
                    self.board_viewport_x, self.board_viewport_y
                );
            }
            MouseEventKind::ScrollLeft => {
                // Scroll left - move viewport right (natural scrolling)
                self.board_viewport_x = self.board_viewport_x.saturating_add(15);
                self.status_message = format!(
                    "Scrolled left. Viewport at ({}, {})",
                    self.board_viewport_x, self.board_viewport_y
                );
            }
            MouseEventKind::ScrollRight => {
                // Scroll right - move viewport left (natural scrolling)
                self.board_viewport_x = self.board_viewport_x.saturating_sub(15);
                self.status_message = format!(
                    "Scrolled right. Viewport at ({}, {})",
                    self.board_viewport_x, self.board_viewport_y
                );
            }
            _ => {} // Ignore other mouse events
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
            InputMode::ShowStatusLog => {
                self.handle_status_log_input(key_code);
            }

            InputMode::EnterShareMessage => {
                self.handle_share_message_input(key_code);
            }
            InputMode::EnterShareString => {
                self.handle_share_string_input(key_code);
            }
            InputMode::ShareSelection => {
                self.handle_share_selection_input(key_code);
            }
            InputMode::ArtEditorNewArtName => {
                self.handle_new_art_name_input(key_code);
            }
            InputMode::ArtSelection => {
                self.handle_art_selection_input(key_code);
            }
            InputMode::ArtPreview => {
                self.handle_art_preview_input(key_code);
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
                        let art_name = art.name.clone();
                        let art_position = (art.board_x, art.board_y);

                        // Add art to queue at current position
                        self.add_art_to_queue(art.clone()).await;

                        // Clear loaded art so user exits positioning mode
                        self.loaded_art = None;

                        // Start queue processing immediately
                        if !self.queue_processing {
                            self.trigger_queue_processing();
                        }

                        self.status_message = format!(
                            "Added '{}' to queue at ({}, {}). Queue processing started.",
                            art_name, art_position.0, art_position.1
                        );
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
                KeyCode::Up => self.board_viewport_y = self.board_viewport_y.saturating_sub(25),
                KeyCode::Down => self.board_viewport_y = self.board_viewport_y.saturating_add(25),
                KeyCode::Left => self.board_viewport_x = self.board_viewport_x.saturating_sub(15),
                KeyCode::Right => self.board_viewport_x = self.board_viewport_x.saturating_add(15),
                KeyCode::Esc => {
                    if self.queue_processing {
                        self.cancel_queue_processing();
                    }
                }
                KeyCode::Char('q') => self.exit = true,
                KeyCode::Char('c') => {
                    self.input_mode = InputMode::EnterAccessToken;
                    self.status_message = "Re-enter Access Token (current will be overwritten if new is provided, skip Refresh Token step if not needed):".to_string();
                    self.input_buffer.clear();
                }
                KeyCode::Char('b') => {
                    self.input_mode = InputMode::EnterBaseUrl;
                    self.status_message = "Select API Base URL or choose Custom:".to_string();
                    self.base_url_selection_index = 0; // Reset selection to first option
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
                KeyCode::Char('h') => {
                    self.input_mode = InputMode::ShowStatusLog;
                    self.status_message =
                        "Showing status log history. Press Esc, q, or h to close.".to_string();
                }
                KeyCode::Char('w') => {
                    // Open work queue management
                    self.input_mode = InputMode::ArtQueue;
                    // Recalculate queue totals to ensure pixel counts are up-to-date
                    self.recalculate_queue_totals();
                    // Center viewport on the first queue item if queue is not empty
                    if !self.art_queue.is_empty() {
                        self.center_viewport_on_selected_queue_item();
                    }
                    self.status_message =
                        "Work Queue Management. Use arrows to navigate, Enter to start processing."
                            .to_string();
                }
                KeyCode::Char('s') => {
                    // Toggle pause/resume for selected queue item
                    self.toggle_selected_queue_item_pause();
                }
                KeyCode::Char('x') => {
                    // Share current loaded art with coordinates
                    if let Some(art) = &self.loaded_art {
                        self.start_art_sharing(art.clone(), art.board_x, art.board_y);
                    } else {
                        self.status_message =
                            "No art loaded to share. Load art first with 'l'.".to_string();
                    }
                }
                KeyCode::Char('v') => {
                    // View/import shared arts
                    self.open_share_selection();
                }
                KeyCode::Char('z') => {
                    // Enter share string for quick coordinate sharing
                    self.input_mode = InputMode::EnterShareString;
                    self.input_buffer.clear();
                    self.status_message =
                        "Enter share string (ftplace-share: NAME at (X, Y)):".to_string();
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
                    art.pattern.retain(|p| {
                        p.x != self.art_editor_cursor_x || p.y != self.art_editor_cursor_y
                    });
                    art.pattern.push(ArtPixel {
                        x: self.art_editor_cursor_x,
                        y: self.art_editor_cursor_y,
                        color: self.art_editor_selected_color_id,
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

    fn handle_status_log_input(&mut self, key_code: KeyCode) {
        match key_code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('h') => {
                self.input_mode = InputMode::None;
                self.status_message = "Status log closed.".to_string();
            }
            KeyCode::Char('r') => {
                // Allow board refresh while status log is open
                self.trigger_board_fetch();
            }
            KeyCode::Char('p') => {
                // Allow profile fetch while status log is open
                self.trigger_profile_fetch();
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
                        width: 0,
                        height: 0,
                        pattern: Vec::new(),
                        board_x: 0,
                        board_y: 0,
                        description: None,
                        author: None,
                        created_at: Some(chrono::Utc::now().to_rfc3339()),
                        tags: None,
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
                    let mut art_to_load = selected_art.clone();

                    // Center the art in the current viewport
                    if let Some((_, _, board_width, board_height)) = self.board_area_bounds {
                        // Calculate viewport center in board coordinates
                        let viewport_center_x =
                            self.board_viewport_x as i32 + (board_width as i32 / 2);
                        let viewport_center_y =
                            self.board_viewport_y as i32 + (board_height as i32); // *2 because half-blocks

                        // Get art dimensions to center it properly
                        let art_dimensions = crate::art::get_art_dimensions(&art_to_load);
                        let art_center_offset_x = art_dimensions.0 / 2;
                        let art_center_offset_y = art_dimensions.1 / 2;

                        // Position art so its center aligns with viewport center
                        art_to_load.board_x = viewport_center_x - art_center_offset_x;
                        art_to_load.board_y = viewport_center_y - art_center_offset_y;
                    } else {
                        // Fallback: center in current viewport using viewport coordinates
                        art_to_load.board_x = self.board_viewport_x as i32 + 25; // Rough center estimate
                        art_to_load.board_y = self.board_viewport_y as i32 + 15;
                    }

                    // Load art for positioning
                    self.loaded_art = Some(art_to_load.clone());
                    self.input_mode = InputMode::None;
                    self.status_message = format!(
                        "Loaded art: '{}' at ({}, {}). Use arrows to position, Enter to add to queue.",
                        art_to_load.name, art_to_load.board_x, art_to_load.board_y
                    );
                }
            }
            KeyCode::Char(' ') => {
                // Show full-screen preview of selected art
                if let Some(selected_art) = self
                    .available_pixel_arts
                    .get(self.art_selection_index)
                    .cloned()
                {
                    self.art_preview_art = Some(selected_art);
                    self.input_mode = InputMode::ArtPreview;
                    self.status_message =
                        "Full-screen art preview. Press Esc to return to selection.".to_string();
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

    fn handle_art_preview_input(&mut self, key_code: KeyCode) {
        match key_code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.art_preview_art = None;
                self.input_mode = InputMode::ArtSelection;
                self.status_message = "Returned to art selection.".to_string();
            }
            KeyCode::Enter => {
                // Load the previewed art for positioning
                if let Some(art) = &self.art_preview_art {
                    let mut art_to_load = art.clone();

                    // Center the art in the current viewport
                    if let Some((_, _, board_width, board_height)) = self.board_area_bounds {
                        // Calculate viewport center in board coordinates
                        let viewport_center_x =
                            self.board_viewport_x as i32 + (board_width as i32 / 2);
                        let viewport_center_y =
                            self.board_viewport_y as i32 + (board_height as i32); // *2 because half-blocks

                        // Get art dimensions to center it properly
                        let art_dimensions = crate::art::get_art_dimensions(&art_to_load);
                        let art_center_offset_x = art_dimensions.0 / 2;
                        let art_center_offset_y = art_dimensions.1 / 2;

                        // Position art so its center aligns with viewport center
                        art_to_load.board_x = viewport_center_x - art_center_offset_x;
                        art_to_load.board_y = viewport_center_y - art_center_offset_y;
                    } else {
                        // Fallback: center in current viewport using viewport coordinates
                        art_to_load.board_x = self.board_viewport_x as i32 + 25; // Rough center estimate
                        art_to_load.board_y = self.board_viewport_y as i32 + 15;
                    }

                    // Load art for positioning
                    self.loaded_art = Some(art_to_load.clone());
                    self.art_preview_art = None;
                    self.input_mode = InputMode::None;
                    self.status_message = format!(
                        "Loaded art: '{}' at ({}, {}). Use arrows to position, Enter to add to queue.",
                        art_to_load.name, art_to_load.board_x, art_to_load.board_y
                    );
                }
            }
            _ => {}
        }
    }

    async fn handle_queue_input(&mut self, key_code: KeyCode) -> io::Result<()> {
        match key_code {
            KeyCode::Up => {
                if self.queue_selection_index > 0 {
                    self.queue_selection_index -= 1;
                    // Center viewport on the newly selected queue item
                    self.center_viewport_on_selected_queue_item();
                }
            }
            KeyCode::Down => {
                if self.queue_selection_index < self.art_queue.len().saturating_sub(1) {
                    self.queue_selection_index += 1;
                    // Center viewport on the newly selected queue item
                    self.center_viewport_on_selected_queue_item();
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
                    let _ = self.save_queue(); // Auto-save after reordering
                                               // Center viewport on the moved item
                    self.center_viewport_on_selected_queue_item();
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
                    let _ = self.save_queue(); // Auto-save after reordering
                                               // Center viewport on the moved item
                    self.center_viewport_on_selected_queue_item();
                    self.status_message = format!(
                        "Moved '{}' down in queue",
                        self.art_queue[self.queue_selection_index].art.name
                    );
                }
            }
            KeyCode::Enter => {
                // Check if we have a selected item and it's failed - allow resuming it
                if !self.art_queue.is_empty() && self.queue_selection_index < self.art_queue.len() {
                    let is_failed = self.art_queue[self.queue_selection_index].status
                        == crate::app_state::QueueStatus::Failed;

                    if is_failed {
                        // Resume failed item by resetting to pending
                        let art_name = self.art_queue[self.queue_selection_index].art.name.clone();
                        self.art_queue[self.queue_selection_index].status =
                            crate::app_state::QueueStatus::Pending;
                        let _ = self.save_queue(); // Auto-save after status change

                        // Automatically start queue processing after resuming
                        self.input_mode = InputMode::None;
                        self.trigger_queue_processing();

                        self.status_message = format!(
                            "Resumed failed item '{}' and started queue processing.",
                            art_name
                        );
                        return Ok(());
                    }
                }

                // Start queue processing for all pending items
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
                let _ = self.save_queue(); // Auto-save after clearing
                self.status_message = "Queue cleared.".to_string();
            }
            KeyCode::Delete | KeyCode::Char('d') => {
                // Remove selected item from queue
                if !self.art_queue.is_empty() && self.queue_selection_index < self.art_queue.len() {
                    let removed_art = self.art_queue.remove(self.queue_selection_index);
                    let _ = self.save_queue(); // Auto-save after removal
                    self.status_message = format!("Removed '{}' from queue.", removed_art.art.name);
                    if self.queue_selection_index >= self.art_queue.len()
                        && !self.art_queue.is_empty()
                    {
                        self.queue_selection_index = self.art_queue.len() - 1;
                    }
                    // Center viewport on the newly selected item if queue is not empty
                    if !self.art_queue.is_empty() {
                        self.center_viewport_on_selected_queue_item();
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
                    let _ = self.save_queue(); // Auto-save after priority change
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
            KeyCode::Char('s') => {
                // Toggle pause/resume for selected queue item
                self.toggle_selected_queue_item_pause();
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_share_message_input(&mut self, key_code: KeyCode) {
        match key_code {
            KeyCode::Enter => {
                let share_message = if self.input_buffer.trim().is_empty() {
                    None
                } else {
                    Some(self.input_buffer.trim().to_string())
                };
                self.complete_art_sharing(share_message);
            }
            KeyCode::Esc => {
                self.input_mode = InputMode::None;
                self.current_share_art = None;
                self.current_share_coords = None;
                self.status_message = "Art sharing cancelled.".to_string();
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
            }
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
            }
            _ => {}
        }
    }

    fn handle_share_string_input(&mut self, key_code: KeyCode) {
        match key_code {
            KeyCode::Enter => {
                let share_string = self.input_buffer.trim().to_string();
                if !share_string.is_empty() {
                    self.apply_share_string(&share_string);
                } else {
                    self.status_message = "Empty share string.".to_string();
                    self.input_mode = InputMode::None;
                }
            }
            KeyCode::Esc => {
                self.input_mode = InputMode::None;
                self.status_message = "Share string input cancelled.".to_string();
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
            }
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
            }
            _ => {}
        }
    }

    fn handle_share_selection_input(&mut self, key_code: KeyCode) {
        match key_code {
            KeyCode::Up => {
                if self.share_selection_index > 0 {
                    self.share_selection_index -= 1;
                }
            }
            KeyCode::Down => {
                if self.share_selection_index < self.available_shares.len().saturating_sub(1) {
                    self.share_selection_index += 1;
                }
            }
            KeyCode::Enter => {
                self.load_shared_art(self.share_selection_index);
            }
            KeyCode::Esc => {
                self.input_mode = InputMode::None;
                self.status_message = "Share selection cancelled.".to_string();
            }
            _ => {}
        }
    }
}
