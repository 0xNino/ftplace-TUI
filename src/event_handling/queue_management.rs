use crate::api_client::UserInfos;
use crate::app_state::{App, ArtQueueItem, QueueStatus, QueueUpdate};
use crate::art::{ArtPixel, PixelArt};
use std::collections::HashSet;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

impl App {
    /// Handle queue processing updates from background queue processing tasks
    pub fn handle_queue_update(&mut self, update: QueueUpdate) {
        match update {
            QueueUpdate::ItemStarted {
                item_index,
                art_name,
                total_items,
            } => {
                self.add_status_message(format!(
                    "Queue processing: Starting item {}/{} - '{}'",
                    item_index + 1,
                    total_items,
                    art_name
                ));
            }
            QueueUpdate::ItemProgress {
                item_index,
                art_name,
                pixels_placed,
                total_pixels,
                position,
                cooldown_remaining,
            } => {
                // Update the queue item progress in our local queue
                if let Some(item) = self.art_queue.get_mut(item_index) {
                    item.pixels_placed = pixels_placed; // Now correctly using actual successful placements
                    item.pixels_total = total_pixels; // Update total to reflect actual pixels that need placing
                }

                let base_msg = format!(
                    "Queue item {}: '{}' - placed {}/{} pixels at ({}, {})",
                    item_index + 1,
                    art_name,
                    pixels_placed, // Show successful placements count
                    total_pixels,
                    position.0,
                    position.1
                );

                if let Some(cooldown) = cooldown_remaining {
                    if cooldown > 120 {
                        // Long cooldown - show in minutes
                        let minutes = cooldown / 60;
                        let seconds = cooldown % 60;
                        if seconds > 0 {
                            self.add_status_message(format!(
                                "{} | Long cooldown: {}m {}s (checking every minute)",
                                base_msg, minutes, seconds
                            ));
                        } else {
                            self.add_status_message(format!(
                                "{} | Long cooldown: {}m (checking every minute)",
                                base_msg, minutes
                            ));
                        }
                    } else {
                        // Normal cooldown
                        self.add_status_message(format!("{} | Cooldown: {}s", base_msg, cooldown));
                    }
                } else {
                    self.add_status_message(base_msg);
                }
            }
            QueueUpdate::ItemCompleted {
                item_index,
                art_name,
                pixels_placed,
                total_pixels,
            } => {
                // Update the queue item status in our local queue
                if let Some(item) = self.art_queue.get_mut(item_index) {
                    item.status = QueueStatus::Complete;
                    item.pixels_placed = pixels_placed;
                    item.pixels_total = total_pixels; // Update total to reflect actual pixels that needed placing
                }

                self.add_status_message(format!(
                    "Queue item {}: '{}' completed - {}/{} pixels placed",
                    item_index + 1,
                    art_name,
                    pixels_placed,
                    total_pixels
                ));
            }
            QueueUpdate::ItemFailed {
                item_index,
                art_name,
                error_msg,
            } => {
                // Update the queue item status in our local queue
                if let Some(item) = self.art_queue.get_mut(item_index) {
                    item.status = QueueStatus::Failed;
                }

                self.add_status_message(format!(
                    "Queue item {}: '{}' failed - {}",
                    item_index + 1,
                    art_name,
                    error_msg
                ));
            }
            QueueUpdate::ItemSkipped {
                item_index,
                art_name,
                reason,
            } => {
                // Update the queue item status in our local queue
                if let Some(item) = self.art_queue.get_mut(item_index) {
                    item.status = QueueStatus::Skipped;
                }

                self.add_status_message(format!(
                    "Queue item {}: '{}' skipped - {}",
                    item_index + 1,
                    art_name,
                    reason
                ));
            }
            QueueUpdate::QueueCompleted {
                total_items_processed,
                total_pixels_placed,
                duration_secs,
            } => {
                self.add_status_message(format!(
					"Queue processing complete! {} items processed, {} pixels placed in {}s. Refreshing board...",
					total_items_processed,
					total_pixels_placed,
					duration_secs
				));

                // Reset queue processing state
                self.queue_processing = false;
                self.queue_processing_start = None;
                self.queue_receiver = None;

                // Trigger board refresh to show results
                self.trigger_board_fetch();
            }
            QueueUpdate::QueueCancelled {
                items_processed,
                total_pixels_placed,
            } => {
                self.add_status_message(format!(
					"Queue processing cancelled: {} items processed, {} pixels placed. Press 'r' to refresh board.",
					items_processed,
					total_pixels_placed
				));

                // Reset queue processing state
                self.queue_processing = false;
                self.queue_processing_start = None;
                self.queue_receiver = None;
            }
            QueueUpdate::QueuePaused {
                item_index,
                art_name,
                pixels_placed,
                total_pixels,
            } => {
                self.queue_paused = true;
                self.add_status_message(format!(
                    "Queue paused at item {}: '{}' - {}/{} pixels placed. Press 'space' to resume.",
                    item_index + 1,
                    art_name,
                    pixels_placed,
                    total_pixels
                ));
            }
            QueueUpdate::QueueResumed {
                item_index,
                art_name,
            } => {
                self.queue_paused = false;
                self.add_status_message(format!(
                    "Queue resumed at item {}: '{}'",
                    item_index + 1,
                    art_name
                ));
            }
            QueueUpdate::ApiCall { message } => {
                self.add_status_message(message);
            }
        }
    }

    /// Add an art to the placement queue
    pub async fn add_art_to_queue(&mut self, art: PixelArt) {
        let meaningful_pixels = self.filter_meaningful_pixels(&art);

        // Calculate pixels that are already correct
        let pixels_already_correct = meaningful_pixels
            .iter()
            .filter(|art_pixel| {
                let abs_x = art.board_x + art_pixel.x;
                let abs_y = art.board_y + art_pixel.y;
                self.is_pixel_already_correct(abs_x, abs_y, art_pixel.color)
            })
            .count();

        let queue_item = ArtQueueItem {
            art: art.clone(),
            priority: 3, // Default priority
            status: QueueStatus::Pending,
            pixels_placed: 0, // Start with 0, only count actually placed pixels
            pixels_total: meaningful_pixels.len(), // Total meaningful pixels
            added_time: Instant::now(),
            paused: false, // Default to not paused
        };

        self.art_queue.push(queue_item);
        self.sort_queue_by_priority();

        // Auto-save queue
        let _ = self.save_queue();

        let pixels_needing_placement = meaningful_pixels.len() - pixels_already_correct;
        self.status_message = format!(
            "Added '{}' to queue at position ({}, {}) - {}/{} pixels correct, {} need placement.",
            art.name,
            art.board_x,
            art.board_y,
            pixels_already_correct,
            meaningful_pixels.len(),
            pixels_needing_placement
        );
    }

    /// Sort queue by priority (1=highest, 5=lowest)
    pub fn sort_queue_by_priority(&mut self) {
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

    /// Trigger non-blocking queue processing if not already in progress
    pub fn trigger_queue_processing(&mut self) {
        if self.queue_processing {
            self.status_message =
                "Queue processing already in progress. Press Esc to cancel.".to_string();
            return;
        }

        if self.art_queue.is_empty() {
            self.status_message = "Queue is empty.".to_string();
            return;
        }

        let pending_count = self
            .art_queue
            .iter()
            .filter(|item| item.status == QueueStatus::Pending)
            .count();

        if pending_count == 0 {
            self.status_message = "No pending items in queue.".to_string();
            return;
        }

        // Set up queue processing state
        self.queue_processing = true;
        self.queue_processing_start = Some(Instant::now());

        // Create channel for queue updates
        let (tx, rx) = mpsc::unbounded_channel();
        self.queue_receiver = Some(rx);

        // Create channel for queue control (pause/resume)
        let (control_tx, control_rx) = mpsc::unbounded_channel();
        self.queue_control_sender = Some(control_tx);

        // Clone API client data and queue data needed for processing
        let base_url = self.api_client.get_base_url();
        let access_token = self.api_client.get_access_token_clone();
        let refresh_token = self.api_client.get_refresh_token_clone();

        // Create or get shared reference to board state that can be updated
        let board_state = if let Some(existing_shared_board) = &self.shared_board_state {
            existing_shared_board.clone()
        } else {
            let new_shared_board = std::sync::Arc::new(std::sync::RwLock::new(self.board.clone()));
            self.shared_board_state = Some(new_shared_board.clone());
            new_shared_board
        };
        let queue_items: Vec<_> = self
            .art_queue
            .iter()
            .enumerate()
            .filter(|(_, item)| item.status == QueueStatus::Pending && !item.paused)
            .map(|(index, item)| (index, item.clone()))
            .collect();

        self.status_message = format!(
			"Starting queue processing: {} pending items (intelligent timer-based cooldown management)...",
			pending_count
		);

        // Spawn async task for queue processing
        tokio::spawn(async move {
            let mut api_client =
                crate::api_client::ApiClient::new(Some(base_url), access_token, refresh_token);

            let mut processed_count = 0;
            let mut total_pixels_placed = 0;
            let start_time = Instant::now();
            let mut is_paused = false;
            let mut control_rx = control_rx; // Make it mutable

            for (original_index, queue_item) in queue_items {
                // Check for pause/resume commands
                while let Ok(control_cmd) = control_rx.try_recv() {
                    match control_cmd {
                        crate::app_state::QueueControl::Pause => {
                            is_paused = true;
                            let _ = tx.send(QueueUpdate::QueuePaused {
                                item_index: original_index,
                                art_name: queue_item.art.name.clone(),
                                pixels_placed: 0,
                                total_pixels: 0,
                            });
                        }
                        crate::app_state::QueueControl::Resume => {
                            is_paused = false;
                            let _ = tx.send(QueueUpdate::QueueResumed {
                                item_index: original_index,
                                art_name: queue_item.art.name.clone(),
                            });
                        }
                        crate::app_state::QueueControl::Cancel => {
                            let _ = tx.send(QueueUpdate::QueueCancelled {
                                items_processed: processed_count,
                                total_pixels_placed,
                            });
                            return;
                        }
                    }
                }

                // Wait while paused
                while is_paused {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    // Check for resume command
                    while let Ok(control_cmd) = control_rx.try_recv() {
                        match control_cmd {
                            crate::app_state::QueueControl::Resume => {
                                is_paused = false;
                                let _ = tx.send(QueueUpdate::QueueResumed {
                                    item_index: original_index,
                                    art_name: queue_item.art.name.clone(),
                                });
                            }
                            crate::app_state::QueueControl::Cancel => {
                                let _ = tx.send(QueueUpdate::QueueCancelled {
                                    items_processed: processed_count,
                                    total_pixels_placed,
                                });
                                return;
                            }
                            _ => {}
                        }
                    }
                }

                // Send item started update
                let _ = tx.send(QueueUpdate::ItemStarted {
                    item_index: original_index,
                    art_name: queue_item.art.name.clone(),
                    total_items: processed_count + 1, // Will be corrected as we process
                });

                // Filter meaningful pixels for this art
                let meaningful_pixels = Self::filter_meaningful_pixels_static(&queue_item.art);
                let total_meaningful_pixels = meaningful_pixels.len();

                // Count pixels already correct at start
                let pixels_already_correct_at_start = {
                    let board_lock = board_state.read().unwrap();
                    meaningful_pixels
                        .iter()
                        .filter(|art_pixel| {
                            let abs_x = queue_item.art.board_x + art_pixel.x;
                            let abs_y = queue_item.art.board_y + art_pixel.y;
                            Self::is_pixel_already_correct_static(
                                &board_lock,
                                abs_x,
                                abs_y,
                                art_pixel.color,
                            )
                        })
                        .count()
                };

                // Filter pixels that need to be placed (check against current board state)
                let pixels_to_place: Vec<_> = {
                    let board_lock = board_state.read().unwrap();
                    meaningful_pixels
                        .into_iter()
                        .enumerate()
                        .filter(|(_, art_pixel)| {
                            let abs_x = queue_item.art.board_x + art_pixel.x;
                            let abs_y = queue_item.art.board_y + art_pixel.y;
                            // Only include pixels that need to be changed
                            !Self::is_pixel_already_correct_static(
                                &board_lock,
                                abs_x,
                                abs_y,
                                art_pixel.color,
                            )
                        })
                        .collect()
                };

                if pixels_to_place.is_empty() {
                    // Send skip update - all pixels already correct
                    let _ = tx.send(QueueUpdate::ItemSkipped {
                        item_index: original_index,
                        art_name: queue_item.art.name.clone(),
                        reason: "All pixels already correct".to_string(),
                    });
                    continue;
                }

                let mut pixels_placed_for_item = 0; // Only count actually placed pixels
                let mut user_info: Option<UserInfos> = None;

                // Process each pixel that needs to be placed
                for (_original_pixel_index, art_pixel) in pixels_to_place {
                    let abs_x = queue_item.art.board_x + art_pixel.x;
                    let abs_y = queue_item.art.board_y + art_pixel.y;

                    // ALWAYS check cooldown before attempting each pixel (critical fix!)
                    // This ensures we respect cooldowns from previous 425 error responses
                    if let Some(ref info) = user_info {
                        let (should_pause, wait_time) = should_pause_queue_processing(info);

                        if should_pause {
                            // Long cooldown detected - send pause update and wait
                            let _minutes = wait_time / 60;
                            let _seconds = wait_time % 60;

                            let display_pixels_placed =
                                pixels_placed_for_item + pixels_already_correct_at_start;
                            let _ = tx.send(QueueUpdate::ItemProgress {
                                item_index: original_index,
                                art_name: queue_item.art.name.clone(),
                                pixels_placed: display_pixels_placed,
                                total_pixels: total_meaningful_pixels,
                                position: (abs_x, abs_y),
                                cooldown_remaining: Some(wait_time as u32),
                            });

                            // For very long waits, check every minute if we can place earlier
                            let mut total_waited = 0u64;
                            while total_waited < wait_time {
                                let wait_chunk = std::cmp::min(60, wait_time - total_waited);
                                tokio::time::sleep(Duration::from_secs(wait_chunk)).await;
                                total_waited += wait_chunk;

                                // Try to get fresh user info to see if cooldown changed
                                match api_client.get_profile().await {
                                    Ok(profile_response) => {
                                        user_info = Some(profile_response.user_infos);

                                        // Check if we can place now (buffer available or timers expired)
                                        if let Some(ref fresh_info) = user_info {
                                            let fresh_wait =
                                                calculate_cooldown_wait_time(fresh_info);
                                            if fresh_wait == 0 {
                                                // We can place now! Break out of waiting loop
                                                let display_pixels_placed = pixels_placed_for_item
                                                    + pixels_already_correct_at_start;
                                                let _ = tx.send(QueueUpdate::ItemProgress {
                                                    item_index: original_index,
                                                    art_name: queue_item.art.name.clone(),
                                                    pixels_placed: display_pixels_placed,
                                                    total_pixels: total_meaningful_pixels,
                                                    position: (abs_x, abs_y),
                                                    cooldown_remaining: None,
                                                });
                                                break;
                                            }
                                        }
                                    }
                                    Err(_) => {
                                        // Profile fetch failed, continue waiting
                                    }
                                }

                                // Send progress update with remaining time
                                let remaining_wait = wait_time - total_waited;
                                let display_pixels_placed =
                                    pixels_placed_for_item + pixels_already_correct_at_start;
                                let _ = tx.send(QueueUpdate::ItemProgress {
                                    item_index: original_index,
                                    art_name: queue_item.art.name.clone(),
                                    pixels_placed: display_pixels_placed,
                                    total_pixels: total_meaningful_pixels,
                                    position: (abs_x, abs_y),
                                    cooldown_remaining: Some(remaining_wait as u32),
                                });
                            }
                        } else if wait_time > 0 {
                            // Short cooldown - wait normally
                            let display_pixels_placed =
                                pixels_placed_for_item + pixels_already_correct_at_start;
                            let _ = tx.send(QueueUpdate::ItemProgress {
                                item_index: original_index,
                                art_name: queue_item.art.name.clone(),
                                pixels_placed: display_pixels_placed,
                                total_pixels: total_meaningful_pixels,
                                position: (abs_x, abs_y),
                                cooldown_remaining: Some(wait_time as u32),
                            });

                            tokio::time::sleep(Duration::from_secs(wait_time)).await;
                        }
                    }

                    // Send placement progress update
                    // For display purposes, include already correct pixels in the count
                    let display_pixels_placed =
                        pixels_placed_for_item + pixels_already_correct_at_start;
                    let _ = tx.send(QueueUpdate::ItemProgress {
                        item_index: original_index,
                        art_name: queue_item.art.name.clone(),
                        pixels_placed: display_pixels_placed,
                        total_pixels: total_meaningful_pixels,
                        position: (abs_x, abs_y),
                        cooldown_remaining: None,
                    });

                    // Attempt to place the pixel (no retries for cooldown errors)
                    loop {
                        // Send API call log to main thread
                        let _ = tx.send(QueueUpdate::ApiCall {
                            message: format!(
                                "🎨 POST /api/set (place pixel at {},{} color {})",
                                abs_x, abs_y, art_pixel.color
                            ),
                        });

                        match api_client.place_pixel(abs_x, abs_y, art_pixel.color).await {
                            Ok(response) => {
                                // Send success log
                                let _ = tx.send(QueueUpdate::ApiCall {
                                    message: format!("🎨 POST /api/set → ✅200"),
                                });

                                pixels_placed_for_item += 1;
                                total_pixels_placed += 1;
                                user_info = Some(response.user_infos);
                                break; // Successfully placed, move to next pixel
                            }
                            Err(e) => {
                                // Send error log with status
                                let status_text = match &e {
                                    crate::api_client::ApiError::ErrorResponse {
                                        status, ..
                                    } => {
                                        let status_emoji = match status.as_u16() {
                                            400..=499 => "❌",
                                            500..=599 => "💥",
                                            _ => "❓",
                                        };
                                        format!("{}{}", status_emoji, status.as_u16())
                                    }
                                    crate::api_client::ApiError::Unauthorized => {
                                        "❌401".to_string()
                                    }
                                    crate::api_client::ApiError::TokenRefreshedPleaseRetry => {
                                        "🔄426".to_string()
                                    }
                                    _ => "💥ERR".to_string(),
                                };

                                let _ = tx.send(QueueUpdate::ApiCall {
                                    message: format!("🎨 POST /api/set → {}", status_text),
                                });

                                // Handle different types of errors
                                match &e {
                                    crate::api_client::ApiError::ErrorResponse {
                                        status,
                                        error_response,
                                    } => {
                                        // Check if this is a cooldown/rate limit error
                                        if *status == reqwest::StatusCode::TOO_MANY_REQUESTS
											|| status.as_u16() == 425 // Too Early
											|| status.as_u16() == 420
                                        // Enhance Your Hype
                                        {
                                            // Update user info with new timers from error response
                                            if let Some(timers) = &error_response.timers {
                                                if let Some(ref mut info) = user_info {
                                                    info.timers = Some(timers.clone());
                                                    // Also update pixel_timer if available
                                                    if let Some(interval) = error_response.interval
                                                    {
                                                        info.pixel_timer = interval as i32;
                                                    }
                                                } else {
                                                    // Create minimal user info if we don't have it
                                                    user_info = Some(UserInfos {
                                                        timers: Some(timers.clone()),
                                                        pixel_buffer: 0,
                                                        pixel_timer: error_response
                                                            .interval
                                                            .unwrap_or(5000)
                                                            as i32,
                                                        id: None,
                                                        username: None,
                                                        soft_is_admin: None,
                                                        soft_is_banned: None,
                                                        num: None,
                                                        min_px: None,
                                                        campus_name: None,
                                                        iat: None,
                                                        exp: None,
                                                    });
                                                }
                                            }

                                            // For cooldown errors, wait for cooldown and retry
                                            let wait_time = if let Some(ref info) = user_info {
                                                let calculated_wait =
                                                    calculate_cooldown_wait_time(info);
                                                // For 425 errors, if calculated time is very small, it means
                                                // the timer calculation failed - use a longer fallback
                                                if calculated_wait < 5 {
                                                    30 // 30 seconds when calculation seems wrong
                                                } else {
                                                    calculated_wait
                                                }
                                            } else {
                                                30 // Default 30 seconds if no user info
                                            };

                                            let display_pixels_placed = pixels_placed_for_item
                                                + pixels_already_correct_at_start;
                                            let _ = tx.send(QueueUpdate::ItemProgress {
                                                item_index: original_index,
                                                art_name: queue_item.art.name.clone(),
                                                pixels_placed: display_pixels_placed,
                                                total_pixels: total_meaningful_pixels,
                                                position: (abs_x, abs_y),
                                                cooldown_remaining: Some(wait_time as u32),
                                            });

                                            // Wait for the full cooldown period
                                            tokio::time::sleep(Duration::from_secs(wait_time))
                                                .await;
                                            // Continue to retry after waiting
                                            continue;
                                        } else {
                                            // Other API errors (auth, server error, etc.) - stop processing
                                            let _ = tx.send(QueueUpdate::ItemFailed {
                                                item_index: original_index,
                                                art_name: queue_item.art.name.clone(),
                                                error_msg: error_response.message.clone(),
                                            });
                                            return;
                                        }
                                    }
                                    crate::api_client::ApiError::Unauthorized => {
                                        // Auth error - stop processing
                                        let _ = tx.send(QueueUpdate::ItemFailed {
                                            item_index: original_index,
                                            art_name: queue_item.art.name.clone(),
                                            error_msg: "Unauthorized - check tokens".to_string(),
                                        });
                                        return;
                                    }
                                    _ => {
                                        // Other errors (network, etc.) - stop processing
                                        let _ = tx.send(QueueUpdate::ItemFailed {
                                            item_index: original_index,
                                            art_name: queue_item.art.name.clone(),
                                            error_msg: format!("{:?}", e),
                                        });
                                        return;
                                    }
                                }
                            }
                        }
                    }

                    // Small delay between pixels
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }

                // Send item completion update
                let display_pixels_placed =
                    pixels_placed_for_item + pixels_already_correct_at_start;
                let _ = tx.send(QueueUpdate::ItemCompleted {
                    item_index: original_index,
                    art_name: queue_item.art.name.clone(),
                    pixels_placed: display_pixels_placed,
                    total_pixels: total_meaningful_pixels,
                });

                processed_count += 1;
            }

            // Send queue completion update
            let duration_secs = start_time.elapsed().as_secs();
            let _ = tx.send(QueueUpdate::QueueCompleted {
                total_items_processed: processed_count,
                total_pixels_placed,
                duration_secs,
            });
        });
    }

    /// Pause queue processing
    pub fn pause_queue(&mut self) {
        if !self.queue_processing {
            self.status_message = "No queue processing to pause.".to_string();
            return;
        }

        if self.queue_paused {
            self.status_message = "Queue is already paused.".to_string();
            return;
        }

        // Send pause command to background task
        if let Some(sender) = &self.queue_control_sender {
            if sender.send(crate::app_state::QueueControl::Pause).is_ok() {
                self.queue_paused = true;
                self.status_message =
                    "Queue processing paused. Press 'space' to resume.".to_string();
            } else {
                self.status_message = "Failed to pause queue processing.".to_string();
            }
        } else {
            self.status_message = "No queue control channel available.".to_string();
        }
    }

    /// Resume queue processing
    pub fn resume_queue(&mut self) {
        if !self.queue_processing {
            self.status_message = "No queue processing to resume.".to_string();
            return;
        }

        if !self.queue_paused {
            self.status_message = "Queue is not paused.".to_string();
            return;
        }

        // Send resume command to background task
        if let Some(sender) = &self.queue_control_sender {
            if sender.send(crate::app_state::QueueControl::Resume).is_ok() {
                self.queue_paused = false;
                self.status_message = "Queue processing resumed.".to_string();
            } else {
                self.status_message = "Failed to resume queue processing.".to_string();
            }
        } else {
            self.status_message = "No queue control channel available.".to_string();
        }
    }

    /// Toggle queue pause/resume
    pub fn toggle_queue_pause(&mut self) {
        if !self.queue_processing {
            self.status_message = "No queue processing active.".to_string();
            return;
        }

        if self.queue_paused {
            self.resume_queue();
        } else {
            self.pause_queue();
        }
    }

    /// Pause individual queue item
    pub fn pause_queue_item(&mut self, index: usize) {
        if index >= self.art_queue.len() {
            self.status_message = "Invalid queue item index.".to_string();
            return;
        }

        if self.art_queue[index].paused {
            self.status_message = format!(
                "Queue item '{}' is already paused.",
                self.art_queue[index].art.name
            );
            return;
        }

        self.art_queue[index].paused = true;
        let _ = self.save_queue(); // Auto-save after pausing item
        self.status_message = format!(
            "Paused queue item '{}'. It will be skipped during processing.",
            self.art_queue[index].art.name
        );
    }

    /// Resume individual queue item
    pub fn resume_queue_item(&mut self, index: usize) {
        if index >= self.art_queue.len() {
            self.status_message = "Invalid queue item index.".to_string();
            return;
        }

        if !self.art_queue[index].paused {
            self.status_message = format!(
                "Queue item '{}' is not paused.",
                self.art_queue[index].art.name
            );
            return;
        }

        self.art_queue[index].paused = false;
        let _ = self.save_queue(); // Auto-save after resuming item
        self.status_message = format!(
            "Resumed queue item '{}'. It will be processed normally.",
            self.art_queue[index].art.name
        );
    }

    /// Toggle pause/resume for individual queue item
    pub fn toggle_queue_item_pause(&mut self, index: usize) {
        if index >= self.art_queue.len() {
            self.status_message = "Invalid queue item index.".to_string();
            return;
        }

        if self.art_queue[index].paused {
            self.resume_queue_item(index);
        } else {
            self.pause_queue_item(index);
        }
    }

    /// Toggle pause/resume for currently selected queue item
    pub fn toggle_selected_queue_item_pause(&mut self) {
        self.toggle_queue_item_pause(self.queue_selection_index);
    }

    /// Recalculate queue totals based on current board state
    /// Call this after board refreshes to update pixel counts
    pub fn recalculate_queue_totals(&mut self) {
        // Clone the board and colors to avoid borrowing issues
        let board = self.board.clone();
        let colors = self.colors.clone();

        for item in &mut self.art_queue {
            // Only recalculate for pending items
            if item.status != QueueStatus::Pending {
                continue;
            }

            // Filter meaningful pixels using static method to avoid borrowing self
            let meaningful_pixels = Self::filter_meaningful_pixels_for_art(&item.art, &colors);
            let pixels_already_correct = meaningful_pixels
                .iter()
                .filter(|art_pixel| {
                    let abs_x = item.art.board_x + art_pixel.x;
                    let abs_y = item.art.board_y + art_pixel.y;
                    Self::is_pixel_already_correct_static(&board, abs_x, abs_y, art_pixel.color)
                })
                .count();

            // Update totals - total is all meaningful pixels, placed should remain 0 for pending items
            item.pixels_total = meaningful_pixels.len();
            // Don't update pixels_placed for pending items - it should only track actually placed pixels

            // If all pixels are now correct, mark as complete
            if pixels_already_correct == meaningful_pixels.len() {
                item.status = QueueStatus::Complete;
            }
        }
    }

    /// Static helper for filtering meaningful pixels (used in spawned tasks)
    fn filter_meaningful_pixels_static(art: &PixelArt) -> Vec<ArtPixel> {
        let mut meaningful_pixels = Vec::new();
        let mut seen_positions = HashSet::new();

        // For now, include all pixels since we can't access colors from static context
        // This could be improved by passing color filtering rules to the spawned task
        for pixel in &art.pattern {
            let position = (pixel.x, pixel.y);
            if seen_positions.contains(&position) {
                continue;
            }
            meaningful_pixels.push(pixel.clone());
            seen_positions.insert(position);
        }

        meaningful_pixels
    }

    /// Static helper for filtering meaningful pixels with color filtering
    fn filter_meaningful_pixels_for_art(
        art: &PixelArt,
        colors: &[crate::api_client::ColorInfo],
    ) -> Vec<ArtPixel> {
        let mut meaningful_pixels = Vec::new();
        let mut seen_positions = HashSet::new();

        // Define background color IDs that should not be placed
        let mut background_color_ids = HashSet::new();
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

    /// Static helper for checking if a pixel is already correct
    fn is_pixel_already_correct_static(
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

    /// Legacy queue processing method for compatibility
    #[allow(dead_code)]
    pub async fn start_queue_processing(&mut self) {
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
    #[allow(dead_code)]
    pub async fn place_art_from_queue(
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
            if self.is_pixel_already_correct(abs_x, abs_y, art_pixel.color) {
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

            // Add API call log to status messages
            self.log_api_call("POST", "/api/set", None);

            // Place the pixel
            match self
                .api_client
                .place_pixel(abs_x, abs_y, art_pixel.color)
                .await
            {
                Ok(response) => {
                    // Log successful API call
                    self.log_api_call("POST", "/api/set", Some(200));

                    pixels_placed += 1;
                    self.user_info = Some(response.user_infos);
                }
                Err(e) => {
                    // Log API error with status code
                    match &e {
                        crate::api_client::ApiError::ErrorResponse { status, .. } => {
                            self.log_api_call("POST", "/api/set", Some(status.as_u16()));
                        }
                        crate::api_client::ApiError::Unauthorized => {
                            self.log_api_call("POST", "/api/set", Some(401));
                        }
                        _ => {
                            // For network errors or other issues, log without status
                            self.log_api_call("POST", "/api/set", None);
                        }
                    }

                    return Err(format!("API error: {:?}", e));
                }
            }

            // Small delay between pixels
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        Ok(pixels_placed)
    }

    /// Cancel queue processing
    pub fn cancel_queue_processing(&mut self) {
        if !self.queue_processing {
            self.status_message = "No queue processing to cancel.".to_string();
            return;
        }

        // Send cancel command to background task
        if let Some(sender) = &self.queue_control_sender {
            let _ = sender.send(crate::app_state::QueueControl::Cancel);
        }

        // Reset queue processing state
        self.queue_processing = false;
        self.queue_paused = false;
        self.queue_receiver = None;
        self.queue_control_sender = None;
        self.status_message = "Queue processing cancelled.".to_string();
    }

    /// Save queue to file
    pub fn save_queue(&self) -> Result<(), Box<dyn std::error::Error>> {
        let queue_data = serde_json::to_string_pretty(&self.art_queue)?;
        std::fs::write("queue.json", queue_data)?;
        Ok(())
    }

    /// Load queue from file
    pub fn load_queue(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if std::path::Path::new("queue.json").exists() {
            let queue_data = std::fs::read_to_string("queue.json")?;
            self.art_queue = serde_json::from_str(&queue_data)?;

            let pending_count = self
                .art_queue
                .iter()
                .filter(|item| item.status == QueueStatus::Pending && !item.paused)
                .count();

            if pending_count > 0 {
                self.status_message = format!(
                    "Loaded {} items from saved queue ({} pending). Queue will auto-resume after board loads.",
                    self.art_queue.len(),
                    pending_count
                );
            } else {
                self.status_message =
                    format!("Loaded {} items from saved queue.", self.art_queue.len());
            }
        }
        Ok(())
    }

    /// Check if queue should auto-resume and start it if conditions are met
    pub fn check_auto_resume_queue(&mut self) {
        // Only auto-resume if:
        // 1. Queue is not already processing
        // 2. We have pending items
        // 3. We have valid tokens
        // 4. Board has been fetched (so we have colors for filtering)
        if !self.queue_processing
            && !self.art_queue.is_empty()
            && self.api_client.get_auth_cookie_preview().is_some()
            && self.initial_board_fetched
            && !self.colors.is_empty()
        {
            let pending_count = self
                .art_queue
                .iter()
                .filter(|item| item.status == QueueStatus::Pending && !item.paused)
                .count();

            if pending_count > 0 {
                self.add_status_message(format!(
                    "Auto-resuming queue processing: {} pending items found.",
                    pending_count
                ));
                self.trigger_queue_processing();
            }
        }
    }
}

/// Calculate how long to wait before we can place a pixel based on user timers and buffer
pub fn calculate_cooldown_wait_time(user_info: &UserInfos) -> u64 {
    // If we have pixel buffer available, we can place immediately
    if user_info.pixel_buffer > 0 {
        return 0;
    }

    // No buffer available, check timers to see when we can place next
    if let Some(timers) = &user_info.timers {
        if timers.is_empty() {
            // No active timers - this usually means user has no pixels available for a long time
            // Use pixel_timer as base but be more conservative
            let fallback_time = (user_info.pixel_timer as f64 / 1000.0) as u64;
            return fallback_time.max(60); // Minimum 1 minute when no timers
        }

        // Find the earliest timer that will expire
        let current_time_ms = chrono::Utc::now().timestamp_millis();
        let mut earliest_expiry = i64::MAX;

        for &timer_ms in timers {
            if timer_ms > current_time_ms && timer_ms < earliest_expiry {
                earliest_expiry = timer_ms;
            }
        }

        if earliest_expiry == i64::MAX {
            // All timers have expired but we still got 425 error
            // This suggests the user has no pixels available for a longer period
            let fallback_time = (user_info.pixel_timer as f64 / 1000.0) as u64;
            return fallback_time.max(60); // Minimum 1 minute
        }

        // Calculate exact wait time in seconds
        let wait_time_ms = earliest_expiry - current_time_ms;
        let wait_time_secs = (wait_time_ms as f64 / 1000.0).ceil() as u64;

        // Return the calculated time with small buffer
        wait_time_secs.max(1) + 2 // Minimum 1 second + 2 second buffer
    } else {
        // No timer data at all - very conservative fallback
        let fallback_time = (user_info.pixel_timer as f64 / 1000.0) as u64;
        fallback_time.max(120) // Minimum 2 minutes when no timer data
    }
}

/// Check if we should pause queue processing due to long cooldowns
pub fn should_pause_queue_processing(user_info: &UserInfos) -> (bool, u64) {
    let wait_time = calculate_cooldown_wait_time(user_info);

    // Pause if we need to wait more than 2 minutes
    if wait_time > 120 {
        (true, wait_time)
    } else {
        (false, wait_time)
    }
}
