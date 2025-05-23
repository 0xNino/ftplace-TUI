use crate::api_client::ApiError;
use crate::app_state::{App, ArtQueueItem, BoardFetchResult, InputMode, QueueStatus};
use crate::art::{get_available_pixel_arts, load_default_pixel_art, ArtPixel, PixelArt};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use std::collections::HashSet;
use std::fs::File;
use std::io;
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

impl App {
    pub async fn handle_events(&mut self) -> io::Result<()> {
        // Check if we need to fetch board on startup (when tokens were restored)
        if self.should_fetch_board_on_start {
            self.should_fetch_board_on_start = false; // Clear the flag
            self.trigger_board_fetch();
        }

        // Update blink state for queue previews
        self.update_blink_state();

        // Check for completed board fetches
        if let Some(receiver) = &mut self.board_fetch_receiver {
            if let Ok(result) = receiver.try_recv() {
                self.handle_board_fetch_result(result);
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
            self.status_message = "Auto-refreshing board...".to_string();
            self.trigger_board_fetch();
        }

        // Check for user input first - only process board loading if no input is pending
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
                                        KeyCode::Esc => {
                                            self.loaded_art = None;
                                            self.status_message =
                                                "Loaded art cancelled. Board scroll re-enabled."
                                                    .to_string();
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
                                        KeyCode::Char('r') => self.trigger_board_fetch(),
                                        KeyCode::Char('p') => self.fetch_profile_data().await,
                                        KeyCode::Char('l') => {
                                            // Load art selection menu
                                            self.available_pixel_arts = get_available_pixel_arts();
                                            if !self.available_pixel_arts.is_empty() {
                                                self.input_mode = InputMode::ArtSelection;
                                                self.art_selection_index = 0;
                                                self.status_message = format!(
                                                    "Select pixel art to load ({} available). Use arrows and Enter.",
                                                    self.available_pixel_arts.len()
                                                );
                                            } else {
                                                self.status_message = "No pixel arts available. Create some first with 'e'.".to_string();
                                            }
                                        }
                                        KeyCode::Char('e') => {
                                            // Start by asking for art name
                                            self.input_mode = InputMode::ArtEditorNewArtName;
                                            self.input_buffer.clear();
                                            self.status_message =
                                                "Enter name for new pixel art:".to_string();
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
                                        KeyCode::Char('w') => {
                                            // Open work queue management
                                            self.input_mode = InputMode::ArtQueue;
                                            self.status_message = "Work Queue Management. Use arrows to navigate, + to add loaded art, Q to start processing.".to_string();
                                        }
                                        KeyCode::Char('+') => {
                                            // Add currently loaded art to queue
                                            if let Some(art) = &self.loaded_art {
                                                self.add_art_to_queue(art.clone()).await;
                                            } else {
                                                self.status_message =
                                                    "No art loaded. Use 'l' to load art first."
                                                        .to_string();
                                            }
                                        }
                                        KeyCode::Char('Q') => {
                                            // Start queue processing
                                            if !self.art_queue.is_empty() {
                                                self.queue_processing = true;
                                                self.input_mode = InputMode::None;
                                                self.start_queue_processing().await;
                                            } else {
                                                self.status_message =
                                                    "Queue is empty. Add some arts first."
                                                        .to_string();
                                            }
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
                                            (self.art_editor_color_palette_index + 1)
                                                % self.colors.len();

                                        // Update selected color to match palette index
                                        if let Some(color) =
                                            self.colors.get(self.art_editor_color_palette_index)
                                        {
                                            self.art_editor_selected_color_id = color.id;
                                            let color_name = if color.name.trim().is_empty() {
                                                format!("Color {}", color.id)
                                            } else {
                                                color.name.clone()
                                            };
                                            self.status_message =
                                                format!("Selected color: {}", color_name);
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
                                        if let Some(color) =
                                            self.colors.get(self.art_editor_color_palette_index)
                                        {
                                            self.art_editor_selected_color_id = color.id;
                                            let color_name = if color.name.trim().is_empty() {
                                                format!("Color {}", color.id)
                                            } else {
                                                color.name.clone()
                                            };
                                            self.status_message =
                                                format!("Selected color: {}", color_name);
                                        }
                                    }
                                }
                                KeyCode::Backspace => {
                                    // No action needed for backspace in art editor
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
                            InputMode::ArtEditorNewArtName => match key_event.code {
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
                                        self.status_message =
                                            "Name cannot be empty. Please enter a name."
                                                .to_string();
                                    }
                                }
                                KeyCode::Esc => {
                                    self.input_mode = InputMode::None;
                                    self.status_message =
                                        "New art name input cancelled.".to_string();
                                }
                                KeyCode::Char(to_insert) => self.input_buffer.push(to_insert),
                                KeyCode::Backspace => {
                                    self.input_buffer.pop();
                                }
                                _ => {}
                            },
                            InputMode::ArtSelection => match key_event.code {
                                KeyCode::Up => {
                                    if self.art_selection_index > 0 {
                                        self.art_selection_index -= 1;
                                    }
                                }
                                KeyCode::Down => {
                                    if self.art_selection_index
                                        < self.available_pixel_arts.len() - 1
                                    {
                                        self.art_selection_index += 1;
                                    }
                                }
                                KeyCode::Enter => {
                                    if let Some(selected_art) =
                                        self.available_pixel_arts.get(self.art_selection_index)
                                    {
                                        self.loaded_art = Some(selected_art.clone());
                                        self.input_mode = InputMode::None;
                                        self.status_message = format!(
                                            "Loaded art: '{}'. Use arrow keys to move. Board scroll disabled.",
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
                            },
                            InputMode::ArtQueue => match key_event.code {
                                KeyCode::Up => {
                                    if self.queue_selection_index > 0 {
                                        self.queue_selection_index -= 1;
                                    }
                                }
                                KeyCode::Down => {
                                    if self.queue_selection_index
                                        < self.art_queue.len().saturating_sub(1)
                                    {
                                        self.queue_selection_index += 1;
                                    }
                                }
                                KeyCode::Char('+') => {
                                    // Add currently loaded art to queue
                                    if let Some(art) = &self.loaded_art {
                                        self.add_art_to_queue(art.clone()).await;
                                    } else {
                                        self.status_message =
                                            "No art loaded. Use 'l' to load art first.".to_string();
                                    }
                                }
                                KeyCode::Char('Q') => {
                                    // Start queue processing
                                    if !self.art_queue.is_empty() {
                                        self.queue_processing = true;
                                        self.input_mode = InputMode::None;
                                        self.start_queue_processing().await;
                                    } else {
                                        self.status_message =
                                            "Queue is empty. Add some arts first.".to_string();
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
                                    if !self.art_queue.is_empty()
                                        && self.queue_selection_index < self.art_queue.len()
                                    {
                                        let removed_art =
                                            self.art_queue.remove(self.queue_selection_index);
                                        self.status_message = format!(
                                            "Removed '{}' from queue.",
                                            removed_art.art.name
                                        );
                                        if self.queue_selection_index >= self.art_queue.len()
                                            && !self.art_queue.is_empty()
                                        {
                                            self.queue_selection_index = self.art_queue.len() - 1;
                                        }
                                    }
                                }
                                KeyCode::Char('1'..='5') => {
                                    // Set priority for selected item
                                    if !self.art_queue.is_empty()
                                        && self.queue_selection_index < self.art_queue.len()
                                    {
                                        let priority = match key_event.code {
                                            KeyCode::Char('1') => 1,
                                            KeyCode::Char('2') => 2,
                                            KeyCode::Char('3') => 3,
                                            KeyCode::Char('4') => 4,
                                            KeyCode::Char('5') => 5,
                                            _ => 3, // Default priority
                                        };
                                        self.art_queue[self.queue_selection_index].priority =
                                            priority;
                                        self.sort_queue_by_priority();
                                        self.status_message = format!(
                                            "Set priority {} for '{}'",
                                            priority,
                                            self.art_queue[self.queue_selection_index].art.name
                                        );
                                    }
                                }
                                KeyCode::Esc => {
                                    self.input_mode = InputMode::None;
                                    self.status_message = "Queue management closed.".to_string();
                                }
                                _ => {}
                            },
                        }
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

    /// Trigger a non-blocking board fetch if one isn't already in progress
    fn trigger_board_fetch(&mut self) {
        if self.board_loading {
            // Already loading, don't start another
            return;
        }

        self.board_loading = true;
        self.board_load_start = Some(Instant::now());

        if self.board.is_empty() {
            self.status_message = "Loading board data...".to_string();
        } else {
            self.status_message = "Refreshing board data...".to_string();
        }

        // Create channel for this fetch
        let (tx, rx) = mpsc::unbounded_channel();
        self.board_fetch_receiver = Some(rx);

        // Clone API client data needed for the fetch
        let base_url = self.api_client.get_base_url();
        let access_token = self.api_client.get_access_token_clone();
        let refresh_token = self.api_client.get_refresh_token_clone();

        // Spawn async task for board fetching
        tokio::spawn(async move {
            let mut api_client =
                crate::api_client::ApiClient::new(Some(base_url), access_token, refresh_token);

            let result = match api_client.get_board().await {
                Ok(board_response) => BoardFetchResult::Success(board_response),
                Err(e) => BoardFetchResult::Error(format!("{:?}", e)),
            };

            // Send result back - if this fails, the main app has been dropped
            let _ = tx.send(result);
        });
    }

    /// Handle completed board fetch results from background tasks
    fn handle_board_fetch_result(&mut self, result: BoardFetchResult) {
        let load_time = self
            .board_load_start
            .map(|start| start.elapsed().as_millis())
            .unwrap_or(0);

        match result {
            BoardFetchResult::Success(board_response) => {
                self.board = board_response.board;
                self.colors = board_response.colors;

                self.status_message = format!(
                    "Board data loaded in {}ms. {} colors. Board size: {}x{}. Arrows to scroll.",
                    load_time,
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
            BoardFetchResult::Error(error_msg) => {
                self.status_message = format!(
                    "Error fetching board after {}ms: {}. Try 'r' to refresh.",
                    load_time, error_msg
                );
                self.last_board_refresh = Some(Instant::now());
            }
        }

        // Reset loading state
        self.board_loading = false;
        self.board_load_start = None;
        self.board_fetch_receiver = None;
    }

    async fn fetch_board_data(&mut self) {
        // If not triggered by trigger_board_fetch, set up loading state
        if !self.board_loading {
            self.board_loading = true;
            self.board_load_start = Some(Instant::now());
            self.status_message = "Fetching board data...".to_string();
        }

        match self.api_client.get_board().await {
            Ok(board_response) => {
                self.board = board_response.board;
                self.colors = board_response.colors;

                let load_time = self
                    .board_load_start
                    .map(|start| start.elapsed().as_millis())
                    .unwrap_or(0);

                self.status_message = format!(
                    "Board data loaded in {}ms. {} colors. Board size: {}x{}. Arrows to scroll.",
                    load_time,
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
                let load_time = self
                    .board_load_start
                    .map(|start| start.elapsed().as_millis())
                    .unwrap_or(0);

                match e {
                    ApiError::Unauthorized => {
                        self.status_message = format!(
                            "Session expired after {}ms. Auto-refresh paused. Enter new tokens or restart.", 
                            load_time
                        );
                        self.api_client.clear_tokens();
                        // Clear saved tokens when session expires
                        self.clear_saved_tokens();
                    }
                    _ => {
                        // Use enhanced error display for API errors
                        self.handle_api_error_with_enhanced_display("Error fetching board", &e)
                            .await;
                        self.status_message
                            .push_str(&format!(" ({}ms) Try 'r' to refresh.", load_time));
                    }
                }
                self.last_board_refresh = Some(Instant::now());
            }
        }

        // Reset loading state
        self.board_loading = false;
        self.board_load_start = None;
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

        // Filter out background/transparent pixels and duplicates
        let meaningful_pixels = self.filter_meaningful_pixels(&art_to_place);
        let total_pixels = meaningful_pixels.len();

        if total_pixels == 0 {
            self.status_message = format!(
                "Art '{}' has no meaningful pixels to place (all background/transparent).",
                art_to_place.name
            );
            return;
        }

        self.status_message = format!(
            "Starting to place art '{}' ({} meaningful pixels out of {} total)...",
            art_to_place.name,
            total_pixels,
            art_to_place.pixels.len()
        );

        for (index, art_pixel) in meaningful_pixels.iter().enumerate() {
            let abs_x = art_to_place.board_x + art_pixel.x;
            let abs_y = art_to_place.board_y + art_pixel.y;

            // Check if pixel is already the correct color to avoid unnecessary placement
            if self.is_pixel_already_correct(abs_x, abs_y, art_pixel.color_id) {
                self.status_message = format!(
                    "Pixel {}/{} at ({},{}) already correct color, skipping...",
                    index + 1,
                    total_pixels,
                    abs_x,
                    abs_y
                );
                continue;
            }

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
                            self.trigger_board_fetch();
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
        self.trigger_board_fetch();
    }

    /// Filter out background/transparent pixels and remove duplicates
    fn filter_meaningful_pixels(&self, art: &PixelArt) -> Vec<ArtPixel> {
        let mut meaningful_pixels = Vec::new();
        let mut seen_positions = HashSet::new();

        // Define background color IDs that should not be placed
        // Usually color_id 1 is white/background, but we can be smarter about this
        let background_color_ids = self.get_background_color_ids();

        for pixel in &art.pixels {
            // Skip if this position was already processed (remove duplicates)
            let position = (pixel.x, pixel.y);
            if seen_positions.contains(&position) {
                continue;
            }

            // Skip background/transparent colors
            if background_color_ids.contains(&pixel.color_id) {
                continue;
            }

            meaningful_pixels.push(pixel.clone());
            seen_positions.insert(position);
        }

        meaningful_pixels
    }

    /// Get color IDs that should be considered background/transparent
    fn get_background_color_ids(&self) -> HashSet<i32> {
        let mut background_ids = HashSet::new();

        // Only filter colors explicitly marked as transparent/background
        for color in &self.colors {
            let name_lower = color.name.to_lowercase();
            if name_lower.contains("transparent") 
                || name_lower.contains("background")
                || name_lower.contains("empty")
                || name_lower == "none"
                // Only filter if explicitly alpha/transparent in name
                || name_lower.contains("alpha")
            {
                background_ids.insert(color.id);
            }
        }

        // Don't filter any colors by default - let users place any color they want
        // Including white (color_id 1) which is a valid placeable color

        background_ids
    }

    /// Check if a pixel at the given position already has the correct color
    fn is_pixel_already_correct(&self, x: i32, y: i32, expected_color_id: i32) -> bool {
        // Convert to usize for array indexing
        let x_idx = x as usize;
        let y_idx = y as usize;

        // Check bounds
        if x_idx >= self.board.len() || y_idx >= self.board.get(x_idx).map_or(0, |col| col.len()) {
            return false;
        }

        // Check if the pixel exists and has the correct color
        if let Some(current_pixel) = &self.board[x_idx][y_idx] {
            current_pixel.c == expected_color_id
        } else {
            // No pixel exists, so it's not the correct color
            false
        }
    }

    async fn save_current_art_to_file(&mut self, filename: String) {
        if let Some(art) = &self.current_editing_art {
            // Use the art's existing name, and preserve any positioning
            let art_with_name = PixelArt {
                name: art.name.clone(),
                pixels: art.pixels.clone(),
                board_x: art.board_x, // Preserve position for queue automation
                board_y: art.board_y,
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
                                self.status_message = format!(
                                    "Art '{}' saved to {}",
                                    art_with_name.name,
                                    file_path.display()
                                );
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

    /// Update blink state for queue preview effects
    fn update_blink_state(&mut self) {
        let now = Instant::now();
        if let Some(last_blink) = self.last_blink_time {
            if now.duration_since(last_blink) >= Duration::from_millis(500) {
                self.queue_blink_state = !self.queue_blink_state;
                self.last_blink_time = Some(now);
            }
        } else {
            self.last_blink_time = Some(now);
        }
    }

    /// Add an art to the placement queue
    async fn add_art_to_queue(&mut self, art: PixelArt) {
        let meaningful_pixels = self.filter_meaningful_pixels(&art);

        let queue_item = ArtQueueItem {
            art: art.clone(),
            priority: 3, // Default priority
            status: QueueStatus::Pending,
            pixels_placed: 0,
            pixels_total: meaningful_pixels.len(),
            added_time: Instant::now(),
        };

        self.art_queue.push(queue_item);
        self.sort_queue_by_priority();

        self.status_message = format!(
            "Added '{}' to queue at position ({}, {}) with {} meaningful pixels.",
            art.name,
            art.board_x,
            art.board_y,
            meaningful_pixels.len()
        );
    }

    /// Sort queue by priority (1=highest, 5=lowest)
    fn sort_queue_by_priority(&mut self) {
        self.art_queue.sort_by(|a, b| {
            // Primary: priority (lower number = higher priority)
            match a.priority.cmp(&b.priority) {
                std::cmp::Ordering::Equal => {
                    // Secondary: status (pending first, then others)
                    match (&a.status, &b.status) {
                        (QueueStatus::Pending, QueueStatus::Pending) => {
                            // Tertiary: added time (earlier first)
                            a.added_time.cmp(&b.added_time)
                        }
                        (QueueStatus::Pending, _) => std::cmp::Ordering::Less,
                        (_, QueueStatus::Pending) => std::cmp::Ordering::Greater,
                        _ => a.added_time.cmp(&b.added_time),
                    }
                }
                other => other,
            }
        });
    }

    /// Start processing the art queue
    async fn start_queue_processing(&mut self) {
        if self.art_queue.is_empty() {
            self.status_message = "Queue is empty.".to_string();
            self.queue_processing = false;
            return;
        }

        let pending_count = self
            .art_queue
            .iter()
            .filter(|item| item.status == QueueStatus::Pending)
            .count();
        self.status_message = format!(
            "Starting queue processing: {} pending items...",
            pending_count
        );

        // Process only pending items
        let mut processed_count = 0;
        let mut total_pixels_placed = 0;

        for i in 0..self.art_queue.len() {
            if self.art_queue[i].status != QueueStatus::Pending {
                continue;
            }

            // Update status to in progress
            self.art_queue[i].status = QueueStatus::InProgress;
            let art = self.art_queue[i].art.clone();

            self.status_message = format!(
                "Processing queue item {}/{}: '{}' at ({}, {})",
                processed_count + 1,
                pending_count,
                art.name,
                art.board_x,
                art.board_y
            );

            // Place the art at its stored position
            match self.place_art_from_queue(&art, i).await {
                Ok(pixels_placed) => {
                    self.art_queue[i].status = QueueStatus::Complete;
                    self.art_queue[i].pixels_placed = pixels_placed;
                    total_pixels_placed += pixels_placed;
                    processed_count += 1;
                }
                Err(error_msg) => {
                    self.art_queue[i].status = QueueStatus::Failed;
                    self.status_message = format!("Failed to place '{}': {}", art.name, error_msg);
                    break; // Stop processing on error
                }
            }
        }

        self.queue_processing = false;
        self.status_message = format!(
            "Queue processing complete: {} arts placed, {} total pixels placed.",
            processed_count, total_pixels_placed
        );

        // Refresh board to show results
        self.trigger_board_fetch();
    }

    /// Place art from queue at its stored position
    async fn place_art_from_queue(
        &mut self,
        art: &PixelArt,
        queue_index: usize,
    ) -> Result<usize, String> {
        let meaningful_pixels = self.filter_meaningful_pixels(art);

        if meaningful_pixels.is_empty() {
            self.art_queue[queue_index].status = QueueStatus::Skipped;
            return Ok(0);
        }

        let mut pixels_placed = 0;

        for (pixel_index, art_pixel) in meaningful_pixels.iter().enumerate() {
            let abs_x = art.board_x + art_pixel.x;
            let abs_y = art.board_y + art_pixel.y;

            // Check if pixel is already correct
            if self.is_pixel_already_correct(abs_x, abs_y, art_pixel.color_id) {
                continue;
            }

            // Update progress
            self.art_queue[queue_index].pixels_placed = pixel_index;
            self.status_message = format!(
                "Placing '{}': pixel {}/{} at ({}, {})",
                art.name,
                pixel_index + 1,
                meaningful_pixels.len(),
                abs_x,
                abs_y
            );

            // Wait for cooldown if needed
            if let Some(u_info) = &self.user_info {
                if u_info.pixel_buffer <= 0 && u_info.pixel_timer > 0 {
                    tokio::time::sleep(Duration::from_secs(u_info.pixel_timer as u64)).await;
                }
            }

            // Place the pixel
            match self
                .api_client
                .place_pixel(abs_x, abs_y, art_pixel.color_id)
                .await
            {
                Ok(response) => {
                    pixels_placed += 1;
                    self.user_info = Some(response.user_infos);
                }
                Err(e) => {
                    return Err(format!("API error: {:?}", e));
                }
            }

            // Small delay between pixels
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        Ok(pixels_placed)
    }
}
