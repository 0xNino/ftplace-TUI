use crate::app_state::{App, ArtQueueItem, QueueStatus, ValidationControl, ValidationUpdate};
use crate::art::PixelArt;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

impl App {
    /// Handle validation updates from background validation task
    pub fn handle_validation_update(&mut self, update: ValidationUpdate) {
        match update {
            ValidationUpdate::ItemValidated {
                item_index,
                art_name,
                pixels_correct,
                pixels_total,
                needs_requeue,
            } => {
                if needs_requeue {
                    // Mark item back to pending for re-processing
                    if let Some(item) = self.art_queue.get_mut(item_index) {
                        item.status = QueueStatus::Pending;
                        item.pixels_placed = 0; // Reset progress
                    }

                    self.add_status_message(format!(
                        "🔍 Validation: '{}' griefed ({}/{} pixels correct) - re-queued for correction",
                        art_name, pixels_correct, pixels_total
                    ));
                } else {
                    self.add_status_message(format!(
                        "✅ Validation: '{}' still intact ({}/{} pixels correct)",
                        art_name, pixels_correct, pixels_total
                    ));
                }
            }
            ValidationUpdate::ValidationCycle {
                completed_items_checked,
                items_requeued,
                next_check_in_seconds,
            } => {
                if items_requeued > 0 {
                    self.add_status_message(format!(
                        "🔍 Validation cycle: {} items checked, {} re-queued. Next check in {}min",
                        completed_items_checked,
                        items_requeued,
                        next_check_in_seconds / 60
                    ));
                } else {
                    self.add_status_message(format!(
                        "🔍 Validation cycle: {} items checked, all intact. Next check in {}min",
                        completed_items_checked,
                        next_check_in_seconds / 60
                    ));
                }

                self.last_validation_time = Some(Instant::now());
            }
            ValidationUpdate::ValidationError { error_msg } => {
                self.add_status_message(format!("❌ Validation error: {}", error_msg));
            }
        }
    }

    /// Start periodic validation of completed queue items
    pub fn start_validation(&mut self) {
        if self.validation_enabled {
            self.status_message = "Validation is already running.".to_string();
            return;
        }

        if self.art_queue.is_empty() {
            self.status_message = "No queue items to validate.".to_string();
            return;
        }

        let completed_count = self
            .art_queue
            .iter()
            .filter(|item| item.status == QueueStatus::Complete)
            .count();

        if completed_count == 0 {
            self.status_message = "No completed items to validate.".to_string();
            return;
        }

        // Set up validation task
        self.validation_enabled = true;
        self.last_validation_time = Some(Instant::now());

        // Create channels
        let (tx, rx) = mpsc::unbounded_channel();
        self.validation_receiver = Some(rx);

        let (control_tx, control_rx) = mpsc::unbounded_channel();
        self.validation_control_sender = Some(control_tx);

        // Clone data needed for validation
        let base_url = self.api_client.get_base_url();
        let access_token = self.api_client.get_access_token_clone();
        let refresh_token = self.api_client.get_refresh_token_clone();
        let colors = self.colors.clone();

        // Get completed queue items to validate
        let completed_items: Vec<(usize, ArtQueueItem)> = self
            .art_queue
            .iter()
            .enumerate()
            .filter(|(_, item)| item.status == QueueStatus::Complete)
            .map(|(index, item)| (index, item.clone()))
            .collect();

        self.status_message = format!(
            "🔍 Starting validation: {} completed items will be checked every 5 minutes",
            completed_count
        );

        // Spawn validation task
        tokio::spawn(async move {
            let mut api_client =
                crate::api_client::ApiClient::new(Some(base_url), access_token, refresh_token);
            let mut control_rx = control_rx;

            const VALIDATION_INTERVAL_SECONDS: u64 = 300; // 5 minutes

            loop {
                // Check for stop commands
                if let Ok(control_cmd) = control_rx.try_recv() {
                    match control_cmd {
                        ValidationControl::Stop => {
                            let _ = tx.send(ValidationUpdate::ValidationCycle {
                                completed_items_checked: 0,
                                items_requeued: 0,
                                next_check_in_seconds: 0, // Indicates stopped
                            });
                            return;
                        }
                    }
                }

                // Wait for validation interval
                tokio::time::sleep(Duration::from_secs(VALIDATION_INTERVAL_SECONDS)).await;

                // Check for stop commands again after sleep
                if let Ok(control_cmd) = control_rx.try_recv() {
                    match control_cmd {
                        ValidationControl::Stop => {
                            return;
                        }
                    }
                }

                // Fetch current board state
                let board = match api_client.get_board().await {
                    Ok(board_response) => board_response.board,
                    Err(e) => {
                        let _ = tx.send(ValidationUpdate::ValidationError {
                            error_msg: format!("Failed to fetch board for validation: {:?}", e),
                        });
                        continue;
                    }
                };

                let mut items_requeued = 0;

                // Validate each completed item
                for (original_index, queue_item) in &completed_items {
                    // Filter meaningful pixels for this art
                    let meaningful_pixels =
                        filter_meaningful_pixels_for_validation(&queue_item.art, &colors);
                    let total_meaningful_pixels = meaningful_pixels.len();

                    // Count how many pixels are still correct
                    let mut pixels_correct = 0;
                    let mut needs_requeue = false;

                    for art_pixel in &meaningful_pixels {
                        let abs_x = queue_item.art.board_x + art_pixel.x;
                        let abs_y = queue_item.art.board_y + art_pixel.y;

                        if is_pixel_correct_on_board(&board, abs_x, abs_y, art_pixel.color) {
                            pixels_correct += 1;
                        }
                    }

                    // If less than 90% of pixels are correct, mark for re-queue
                    let correctness_threshold = (total_meaningful_pixels as f32 * 0.9) as usize;
                    if pixels_correct < correctness_threshold {
                        needs_requeue = true;
                        items_requeued += 1;
                    }

                    // Send validation result
                    let _ = tx.send(ValidationUpdate::ItemValidated {
                        item_index: *original_index,
                        art_name: queue_item.art.name.clone(),
                        pixels_correct,
                        pixels_total: total_meaningful_pixels,
                        needs_requeue,
                    });
                }

                // Send cycle completion update
                let _ = tx.send(ValidationUpdate::ValidationCycle {
                    completed_items_checked: completed_items.len(),
                    items_requeued,
                    next_check_in_seconds: VALIDATION_INTERVAL_SECONDS,
                });
            }
        });
    }

    /// Stop periodic validation
    pub fn stop_validation(&mut self) {
        if !self.validation_enabled {
            self.status_message = "Validation is not running.".to_string();
            return;
        }

        // Send stop command to background task
        if let Some(sender) = &self.validation_control_sender {
            let _ = sender.send(ValidationControl::Stop);
        }

        // Reset validation state
        self.validation_enabled = false;
        self.validation_receiver = None;
        self.validation_control_sender = None;
        self.status_message = "🔍 Validation stopped.".to_string();
    }

    /// Toggle validation on/off
    pub fn toggle_validation(&mut self) {
        if self.validation_enabled {
            self.stop_validation();
        } else {
            self.start_validation();
        }
    }
}

/// Filter meaningful pixels for validation (same logic as queue processing)
fn filter_meaningful_pixels_for_validation(
    art: &PixelArt,
    colors: &[crate::api_client::ColorInfo],
) -> Vec<crate::art::ArtPixel> {
    let mut meaningful_pixels = Vec::new();
    let mut seen_positions = std::collections::HashSet::new();

    // Define background color IDs that should not be placed
    let mut background_color_ids = std::collections::HashSet::new();
    for color in colors {
        let name_lower = color.name.to_lowercase();
        if name_lower.contains("transparent")
            || name_lower.contains("background")
            || name_lower.contains("empty")
            || name_lower == "none"
            || name_lower.contains("alpha")
        {
            background_color_ids.insert(color.id);
        }
    }

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

    meaningful_pixels
}

/// Check if a pixel at the given position has the expected color
fn is_pixel_correct_on_board(
    board: &Vec<Vec<Option<crate::api_client::PixelNetwork>>>,
    x: i32,
    y: i32,
    expected_color_id: i32,
) -> bool {
    // Convert to usize for array indexing
    let x_idx = x as usize;
    let y_idx = y as usize;

    // Check bounds
    if x_idx >= board.len() || y_idx >= board.get(x_idx).map_or(0, |col| col.len()) {
        return false;
    }

    // Check if the pixel exists and has the correct color
    if let Some(current_pixel) = board.get(x_idx).and_then(|row| row.get(y_idx)) {
        if let Some(pixel) = current_pixel {
            pixel.c == expected_color_id
        } else {
            // No pixel exists, so it's not the correct color
            false
        }
    } else {
        // No pixel exists, so it's not the correct color
        false
    }
}
