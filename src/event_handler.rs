use crate::api_client::ApiError;
use crate::app_state::{App, InputMode};
use crate::art::{load_default_pixel_art, ArtPixel, PixelArt};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use keyring::Entry;
use std::fs::File;
use std::io;
use std::io::Write;
use std::path::Path;
use std::time::Duration;

const KEYRING_SERVICE_NAME: &str = "ftplace_tui_service";
const KEYRING_USER_NAME: &str = "session_cookie";

impl App {
    pub async fn handle_events(&mut self) -> io::Result<()> {
        if !self.initial_board_fetched {
            self.initial_board_fetched = true;
            self.status_message = "Fetching initial board state...".to_string();
            self.fetch_board_data().await;
            return Ok(());
        }

        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key_event) => {
                    if key_event.kind == KeyEventKind::Press {
                        match self.input_mode {
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
                                            self.input_mode = InputMode::Cookie;
                                            self.status_message =
                                                "Editing cookie. Press Enter to save, Esc to cancel.".to_string();
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
                                        _ => {}
                                    }
                                }
                            }
                            InputMode::Cookie => match key_event.code {
                                KeyCode::Enter => {
                                    let cookie_to_set = self.cookie_input_buffer.trim().to_string();
                                    if !cookie_to_set.is_empty() {
                                        self.api_client.set_cookie(cookie_to_set.clone());
                                        match Entry::new(KEYRING_SERVICE_NAME, KEYRING_USER_NAME) {
                                            Ok(entry) => match entry.set_password(&cookie_to_set) {
                                                Ok(_) => {
                                                    self.status_message = format!(
                                                        "Cookie set and saved to keyring. Length: {}. Press 'p' to get profile.",
                                                        cookie_to_set.len()
                                                    );
                                                }
                                                Err(e) => {
                                                    self.status_message = format!(
                                                        "Cookie set, but FAILED to save to keyring: {}.",
                                                        e
                                                    );
                                                }
                                            },
                                            Err(e) => {
                                                self.status_message = format!(
                                                    "Cookie set, but FAILED to access keyring service '{}' for saving: {}.",
                                                    KEYRING_SERVICE_NAME, e
                                                );
                                            }
                                        }
                                    } else {
                                        self.api_client.clear_cookie();
                                        match Entry::new(KEYRING_SERVICE_NAME, KEYRING_USER_NAME) {
                                            Ok(entry) => match entry.delete_credential() {
                                                Ok(_) => {
                                                    self.status_message =
                                                        "Cookie input empty. Cleared from keyring."
                                                            .to_string();
                                                }
                                                Err(keyring::Error::NoEntry) => {
                                                    self.status_message =
                                                        "Cookie input empty. No cookie was found in keyring to delete.".to_string();
                                                }
                                                Err(e) => {
                                                    self.status_message = format!(
                                                        "Cookie input empty. FAILED to delete from keyring: {}.",
                                                        e
                                                    );
                                                }
                                            },
                                            Err(e) => {
                                                self.status_message = format!(
                                                    "Cookie input empty. FAILED to access keyring service '{}' for deletion: {}.",
                                                    KEYRING_SERVICE_NAME, e
                                                );
                                            }
                                        }
                                    }
                                    self.input_mode = InputMode::None;
                                    self.cookie_input_buffer.clear();
                                }
                                KeyCode::Esc => {
                                    self.input_mode = InputMode::None;
                                    self.status_message = "Cookie input cancelled.".to_string();
                                    self.cookie_input_buffer.clear();
                                }
                                KeyCode::Char(to_insert) => {
                                    self.cookie_input_buffer.push(to_insert);
                                }
                                KeyCode::Backspace => {
                                    self.cookie_input_buffer.pop();
                                }
                                _ => {}
                            },
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
            }
            Err(e) => {
                self.status_message = format!(
                    "Error fetching board: {:?}. Try 'r' to refresh or 'c' for cookie.",
                    e
                );
            }
        }
    }

    async fn fetch_profile_data(&mut self) {
        if self.api_client.get_auth_cookie_preview().is_none() {
            self.status_message =
                "Cannot fetch profile: Cookie not set. Press 'c' to set cookie.".to_string();
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
            }
            Err(e) => {
                self.user_info = None;
                match e {
                    ApiError::Unauthorized => {
                        self.status_message = "Error fetching profile: Unauthorized. Cookie might be invalid or expired. Try 'c' to update.".to_string();
                    }
                    _ => {
                        self.status_message = format!("Error fetching profile: {:?}", e);
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
                "Cannot place pixels: Cookie not set. Press 'c' to set cookie.".to_string();
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
                    let mut error_message = format!(
                        "Error placing pixel {}/{} at ({},{}): {:?}.",
                        index + 1,
                        total_pixels,
                        abs_x,
                        abs_y,
                        e
                    );
                    match e {
                        ApiError::Unauthorized
                        | ApiError::ApiErrorResponse {
                            status: reqwest::StatusCode::UNAUTHORIZED,
                            ..
                        } => {
                            self.api_client.clear_cookie();
                            let mut cleared_message =
                                "Authentication failed (token expired or invalid). Cookie cleared from app.".to_string();
                            match Entry::new(KEYRING_SERVICE_NAME, KEYRING_USER_NAME) {
                                Ok(entry) => match entry.delete_credential() {
                                    Ok(_) => {
                                        cleared_message.push_str(" Also deleted from keyring.");
                                    }
                                    Err(keyring::Error::NoEntry) => {
                                        cleared_message.push_str(" No corresponding entry in keyring or already deleted.");
                                    }
                                    Err(e) => {
                                        cleared_message.push_str(&format!(
                                            " Failed to delete from keyring: {}.",
                                            e
                                        ));
                                    }
                                },
                                Err(e) => {
                                    cleared_message.push_str(&format!(
                                        " Failed to access keyring service '{}' for deletion: {}.",
                                        KEYRING_SERVICE_NAME, e
                                    ));
                                }
                            }
                            error_message = format!("{} Please press 'c' to enter a new cookie from the website. Halting placement.", cleared_message);
                        }
                        _ => {
                            error_message.push_str(" Halting placement.");
                        }
                    }
                    self.status_message = error_message;
                    return;
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        self.status_message = format!(
            "Finished placing art '{}'. Refresh board with 'r'.",
            art_to_place.name
        );
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
}
