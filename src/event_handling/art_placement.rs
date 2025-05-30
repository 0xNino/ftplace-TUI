use crate::api_client::{ApiError, UserInfos};
use crate::app_state::{App, PlacementUpdate};
use crate::art::{ArtPixel, PixelArt};
use std::collections::HashSet;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

impl App {
    /// Handle placement updates from background art placement tasks
    pub fn handle_placement_update(&mut self, update: PlacementUpdate) {
        match update {
            PlacementUpdate::Progress {
                art_name,
                pixel_index,
                total_pixels,
                position,
                cooldown_remaining,
            } => {
                let base_msg = format!(
                    "Placing '{}': pixel {}/{} at ({}, {})",
                    art_name,
                    pixel_index + 1,
                    total_pixels,
                    position.0,
                    position.1
                );

                if let Some(cooldown) = cooldown_remaining {
                    self.add_status_message(format!("{} | Cooldown: {}s", base_msg, cooldown));
                } else {
                    self.add_status_message(base_msg);
                }
            }
            PlacementUpdate::Complete {
                art_name,
                pixels_placed,
                total_pixels,
            } => {
                let placement_time = self
                    .placement_start
                    .map(|start| start.elapsed().as_secs())
                    .unwrap_or(0);

                self.add_status_message(format!(
                    "Completed placing '{}': {}/{} pixels in {}s. Refreshing board...",
                    art_name, pixels_placed, total_pixels, placement_time
                ));

                // Reset placement state
                self.placement_in_progress = false;
                self.placement_start = None;
                self.placement_receiver = None;
                self.placement_cancel_requested = false;

                // Trigger board refresh to show results
                self.trigger_board_fetch();
            }
            PlacementUpdate::Error {
                art_name,
                error_msg,
                pixel_index,
                total_pixels,
            } => {
                self.add_status_message(format!(
                    "Error placing '{}' at pixel {}/{}: {}. Press 'r' to refresh board.",
                    art_name,
                    pixel_index + 1,
                    total_pixels,
                    error_msg
                ));

                // Reset placement state
                self.placement_in_progress = false;
                self.placement_start = None;
                self.placement_receiver = None;
                self.placement_cancel_requested = false;
            }
            PlacementUpdate::Cancelled {
                art_name,
                pixels_placed,
                total_pixels,
            } => {
                self.add_status_message(format!(
                    "Cancelled placing '{}': {}/{} pixels placed. Press 'r' to refresh board.",
                    art_name, pixels_placed, total_pixels
                ));

                // Reset placement state
                self.placement_in_progress = false;
                self.placement_start = None;
                self.placement_receiver = None;
                self.placement_cancel_requested = false;
            }
            PlacementUpdate::ApiCall { message } => {
                self.add_status_message(message);
            }
        }
    }

    /// Trigger non-blocking art placement if one isn't already in progress
    #[allow(dead_code)]
    pub fn trigger_art_placement(&mut self) {
        if self.placement_in_progress {
            self.status_message =
                "Art placement already in progress. Press Esc to cancel.".to_string();
            return;
        }

        if self.loaded_art.is_none() {
            self.status_message = "No art loaded to place.".to_string();
            return;
        }

        if self.api_client.get_auth_cookie_preview().is_none() {
            self.status_message =
                "Cannot place pixels: Access Token not set. Use 'c' to set token.".to_string();
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

        // Set up placement state
        self.placement_in_progress = true;
        self.placement_start = Some(Instant::now());
        self.placement_cancel_requested = false;

        // Create channel for placement updates
        let (tx, rx) = mpsc::unbounded_channel();
        self.placement_receiver = Some(rx);

        // Clone API client data and other needed data
        let base_url = self.api_client.get_base_url();
        let access_token = self.api_client.get_access_token_clone();
        let refresh_token = self.api_client.get_refresh_token_clone();
        let _colors = self.colors.clone();

        self.status_message = format!(
            "Starting to place art '{}' ({} meaningful pixels out of {} total)...",
            art_to_place.name,
            total_pixels,
            art_to_place.pattern.len()
        );

        // Spawn async task for art placement
        tokio::spawn(async move {
            let mut api_client =
                crate::api_client::ApiClient::new(Some(base_url), access_token, refresh_token);

            // Set up callback to save refreshed tokens to storage
            if let Ok(callback) = crate::api_client::create_token_refresh_callback(None) {
                api_client.set_token_refresh_callback(callback);
            }
            // Note: We don't fail the placement if callback setup fails

            let mut pixels_placed = 0;
            let mut user_info: Option<UserInfos> = None;

            for (index, art_pixel) in meaningful_pixels.iter().enumerate() {
                let abs_x = art_to_place.board_x + art_pixel.x;
                let abs_y = art_to_place.board_y + art_pixel.y;

                // Check for cooldown before placing pixel
                if let Some(ref info) = user_info {
                    if info.pixel_buffer <= 0 && info.pixel_timer > 0 {
                        let cooldown_duration = Duration::from_secs((info.pixel_timer * 60) as u64);
                        let start_time = Instant::now();

                        // Wait for cooldown with periodic updates
                        while start_time.elapsed() < cooldown_duration {
                            let remaining_secs =
                                (cooldown_duration - start_time.elapsed()).as_secs() as u32;

                            // Send cooldown progress update with actual remaining time
                            let _ = tx.send(PlacementUpdate::Progress {
                                art_name: art_to_place.name.clone(),
                                pixel_index: index,
                                total_pixels,
                                position: (abs_x, abs_y),
                                cooldown_remaining: Some(remaining_secs),
                            });

                            // Sleep for 1 second before next update
                            tokio::time::sleep(Duration::from_secs(1)).await;
                        }
                    }
                }

                // Send placement progress update
                let _ = tx.send(PlacementUpdate::Progress {
                    art_name: art_to_place.name.clone(),
                    pixel_index: index,
                    total_pixels,
                    position: (abs_x, abs_y),
                    cooldown_remaining: None,
                });

                // Send API call log to main thread
                let _ = tx.send(PlacementUpdate::ApiCall {
                    message: format!(
                        "🎨 POST /api/set (place pixel at {},{} color {})",
                        abs_x, abs_y, art_pixel.color
                    ),
                });

                match api_client.place_pixel(abs_x, abs_y, art_pixel.color).await {
                    Ok(response) => {
                        // Send success log
                        let _ = tx.send(PlacementUpdate::ApiCall {
                            message: format!("🎨 POST /api/set → ✅200"),
                        });
                        pixels_placed += 1;
                        user_info = Some(response.user_infos);
                    }
                    Err(e) => {
                        // Send error log with status
                        let status_text = match &e {
                            crate::api_client::ApiError::ErrorResponse { status, .. } => {
                                let status_emoji = match status.as_u16() {
                                    400..=499 => "❌",
                                    500..=599 => "💥",
                                    _ => "❓",
                                };
                                format!("{}{}", status_emoji, status.as_u16())
                            }
                            crate::api_client::ApiError::Unauthorized => "❌401".to_string(),
                            crate::api_client::ApiError::TokenRefreshedPleaseRetry => {
                                "🔄426".to_string()
                            }
                            _ => "💥ERR".to_string(),
                        };

                        let _ = tx.send(PlacementUpdate::ApiCall {
                            message: format!("🎨 POST /api/set → {}", status_text),
                        });

                        // Send error update
                        let error_msg = match e {
                            crate::api_client::ApiError::ErrorResponse {
                                status: _,
                                error_response,
                            } => error_response.message,
                            _ => format!("{:?}", e),
                        };
                        let _ = tx.send(PlacementUpdate::Error {
                            art_name: art_to_place.name.clone(),
                            error_msg,
                            pixel_index: index,
                            total_pixels,
                        });
                        return;
                    }
                }

                // Small delay between pixels
                tokio::time::sleep(Duration::from_millis(100)).await;
            }

            // Send completion update
            let _ = tx.send(PlacementUpdate::Complete {
                art_name: art_to_place.name.clone(),
                pixels_placed,
                total_pixels,
            });
        });
    }

    /// Legacy art placement method for synchronous placement
    #[allow(dead_code)]
    pub async fn place_loaded_art(&mut self) {
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
            art_to_place.pattern.len()
        );

        for (index, art_pixel) in meaningful_pixels.iter().enumerate() {
            let abs_x = art_to_place.board_x + art_pixel.x;
            let abs_y = art_to_place.board_y + art_pixel.y;

            // Check if pixel is already the correct color to avoid unnecessary placement
            if self.is_pixel_already_correct(abs_x, abs_y, art_pixel.color) {
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
                art_pixel.color
            );

            if let Some(u_info) = &self.user_info {
                if u_info.pixel_buffer <= 0 && u_info.pixel_timer > 0 {
                    self.status_message = format!(
                        "Cooldown active: waiting {}s before placing pixel {}/{}.",
                        u_info.pixel_timer * 60,
                        index + 1,
                        total_pixels
                    );
                    tokio::time::sleep(Duration::from_secs((u_info.pixel_timer * 60) as u64)).await;
                }
            }

            // Add API call log to status messages
            self.log_api_call("POST", "/api/set", None);

            match self
                .api_client
                .place_pixel(abs_x, abs_y, art_pixel.color)
                .await
            {
                Ok(response) => {
                    // Log successful API call
                    self.log_api_call("POST", "/api/set", Some(200));

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
                    // Log API error with status code
                    match &e {
                        ApiError::ErrorResponse { status, .. } => {
                            self.log_api_call("POST", "/api/set", Some(status.as_u16()));
                        }
                        ApiError::Unauthorized => {
                            self.log_api_call("POST", "/api/set", Some(401));
                        }
                        _ => {
                            // For network errors or other issues, log without status
                            self.log_api_call("POST", "/api/set", None);
                        }
                    }

                    // Use enhanced error display for API errors
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
                        | ApiError::ErrorResponse {
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

                    // For rate limiting errors, don't halt placement - let enhanced display handle it
                    if let ApiError::ErrorResponse { status, .. } = &e {
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

            // Small delay between pixels
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        self.status_message = format!(
            "Finished placing art '{}'. Refreshing board...",
            art_to_place.name
        );
        self.trigger_board_fetch();
    }

    /// Filter out background/transparent pixels and remove duplicates
    pub fn filter_meaningful_pixels(&self, art: &PixelArt) -> Vec<ArtPixel> {
        let mut meaningful_pixels = Vec::new();
        let mut seen_positions = HashSet::new();

        // Define background color IDs that should not be placed
        // Usually color_id 1 is white/background, but we can be smarter about this
        let background_color_ids = self.get_background_color_ids();

        for pixel in &art.pattern {
            // Skip if this position was already processed (remove duplicates)
            let position = (pixel.x, pixel.y);
            if seen_positions.contains(&position) {
                continue;
            }

            // Skip background/transparent colors
            if background_color_ids.contains(&pixel.color) {
                continue;
            }

            meaningful_pixels.push(pixel.clone());
            seen_positions.insert(position);
        }

        // Apply border-first ordering
        super::queue_management::order_pixels_border_first(meaningful_pixels)
    }

    /// Get color IDs that should be considered background/transparent
    pub fn get_background_color_ids(&self) -> HashSet<i32> {
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
    #[allow(dead_code)]
    pub fn is_pixel_already_correct(&self, x: i32, y: i32, expected_color_id: i32) -> bool {
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
}
