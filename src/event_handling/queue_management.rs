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
                    "🔄 Queue processing: Starting item {}/{} - '{}'",
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
                    "📋 Queue item {}: '{}' - placed {}/{} pixels at ({}, {})",
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
                                "{} | Long cooldown: {}m {}s",
                                base_msg, minutes, seconds
                            ));
                        } else {
                            self.add_status_message(format!(
                                "{} | Long cooldown: {}m",
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
                    "✅ Queue item {}: '{}' completed - {}/{} pixels placed",
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
                    "❌ Queue item {}: '{}' failed - {}",
                    item_index + 1,
                    art_name,
                    error_msg
                ));

                // Reset queue processing state when an item fails
                // This allows the queue to be restarted
                self.queue_processing = false;
                self.queue_processing_start = None;
                self.queue_receiver = None;
                self.queue_control_sender = None;
                self.queue_paused = false;
            }
            QueueUpdate::ItemSkipped {
                item_index,
                art_name,
                reason,
            } => {
                // Update the queue item status in our local queue
                if let Some(item) = self.art_queue.get_mut(item_index) {
                    item.status = QueueStatus::Skipped;

                    // If skipped because all pixels are already correct,
                    // set pixels_placed to pixels_total for proper display (e.g., 4/4)
                    if reason.contains("already correct") {
                        item.pixels_placed = item.pixels_total;
                    }
                }

                self.add_status_message(format!(
                    "⏭️ Queue item {}: '{}' skipped - {}",
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
					"🎉 Queue processing complete! {} items processed, {} pixels placed in {}s. Refreshing board...",
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
					"🛑 Queue processing cancelled: {} items processed, {} pixels placed. Press 'r' to refresh board.",
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
                    "⏸️ Queue paused at item {}: '{}' - {}/{} pixels placed. Press Esc to cancel.",
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
                    "▶️ Queue resumed at item {}: '{}'",
                    item_index + 1,
                    art_name
                ));
            }
            QueueUpdate::ApiCall { message } => {
                self.add_status_message(message);
            }
            QueueUpdate::EventTiming {
                waiting_for_event,
                event_starts_in_seconds,
                event_message,
            } => {
                // Update app event timing state
                self.waiting_for_event = waiting_for_event;
                self.last_event_check_time = Some(Instant::now());

                if waiting_for_event {
                    if let Some(seconds_until_start) = event_starts_in_seconds {
                        // Calculate the event start time based on current time + interval
                        self.event_start_time = Some(
                            std::time::SystemTime::now() + std::time::Duration::from_secs(seconds_until_start)
                        );
                    }
                } else {
                    // Event has ended or started, clear event timing
                    self.event_start_time = None;
                    self.event_end_time = None;
                }

                self.add_status_message(format!(
                    "🕒 Event Timing: {} - {}",
                    event_message,
                    if waiting_for_event {
                        if let Some(seconds) = event_starts_in_seconds {
                            format!("{} seconds until event", seconds)
                        } else {
                            "Event timing unknown".to_string()
                        }
                    } else {
                        "Event has ended".to_string()
                    }
                ));
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
            let mut control_rx = control_rx; // Make it mutable

            for (original_index, queue_item) in queue_items {
                // Check for cancel commands
                while let Ok(control_cmd) = control_rx.try_recv() {
                    match control_cmd {
                        crate::app_state::QueueControl::Cancel => {
                            let _ = tx.send(QueueUpdate::QueueCancelled {
                                items_processed: processed_count,
                                total_pixels_placed,
                            });
                            return;
                        }
                    }
                }

                // Check for cancel command during processing
                while let Ok(control_cmd) = control_rx.try_recv() {
                    match control_cmd {
                        crate::app_state::QueueControl::Cancel => {
                            let _ = tx.send(QueueUpdate::QueueCancelled {
                                items_processed: processed_count,
                                total_pixels_placed,
                            });
                            return;
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
                let mut pixels_placed_since_refresh = 0; // Track pixels placed since last board refresh
                let mut last_board_refresh = Instant::now(); // Track time since last board refresh
                const REFRESH_INTERVAL_PIXELS: usize = 10; // Refresh every 10 pixels
                const REFRESH_INTERVAL_SECONDS: u64 = 60; // Refresh every 1 minute

                // Process each pixel that needs to be placed
                for (_original_pixel_index, art_pixel) in pixels_to_place {
                    let abs_x = queue_item.art.board_x + art_pixel.x;
                    let abs_y = queue_item.art.board_y + art_pixel.y;

                    // Check if we need to refresh board data (every 20 pixels or 2 minutes)
                    let should_refresh = pixels_placed_since_refresh >= REFRESH_INTERVAL_PIXELS
                        || last_board_refresh.elapsed().as_secs() >= REFRESH_INTERVAL_SECONDS;

                    if should_refresh {
                        // Refresh board data to detect pixels overwritten by other users
                        match api_client.get_board().await {
                            Ok(board_response) => {
                                // Update shared board state
                                if let Ok(mut board_lock) = board_state.write() {
                                    *board_lock = board_response.board;
                                }

                                // Re-check if this pixel still needs to be placed
                                let board_lock = board_state.read().unwrap();
                                if Self::is_pixel_already_correct_static(
                                    &board_lock,
                                    abs_x,
                                    abs_y,
                                    art_pixel.color,
                                ) {
                                    // Pixel was corrected by someone else, skip it
                                    let _ = tx.send(QueueUpdate::ApiCall {
                                        message: format!(
                                            "📡 GET /api/get → ✅ 200 (board refresh)"
                                        ),
                                    });
                                    continue;
                                }
                                drop(board_lock);

                                pixels_placed_since_refresh = 0;
                                last_board_refresh = Instant::now();

                                let _ = tx.send(QueueUpdate::ApiCall {
                                    message: format!("📡 GET /api/get → ✅ 200 (board refresh)"),
                                });
                            }
                            Err(_) => {
                                // Board refresh failed, continue with current data
                                let _ = tx.send(QueueUpdate::ApiCall {
                                    message: format!("📡 GET /api/get → ❌ ERR (refresh failed)"),
                                });
                            }
                        }
                    }

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
                                    message: format!("🎨 POST /api/set → ✅ 200"),
                                });

                                pixels_placed_for_item += 1;
                                total_pixels_placed += 1;
                                pixels_placed_since_refresh += 1; // Track for board refresh timing
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
                                        format!("{} {}", status_emoji, status.as_u16())
                                    }
                                    crate::api_client::ApiError::Unauthorized => {
                                        "❌ 401".to_string()
                                    }
                                    crate::api_client::ApiError::TokenRefreshedPleaseRetry => {
                                        "🔄 426".to_string()
                                    }
                                    _ => "💥 ERR".to_string(),
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
                                        // Check if this is an event timing error (420)
                                        if status.as_u16() == 420 {
                                            // Handle "Enhance Your Calm - Out of event date"
                                            let wait_time = if let Some(interval) =
                                                error_response.interval
                                            {
                                                if interval > 0 {
                                                    // Event hasn't started yet - interval is seconds until start

                                                    // Send event timing update to main app
                                                    let _ = tx.send(QueueUpdate::EventTiming {
                                                        waiting_for_event: true,
                                                        event_starts_in_seconds: Some(
                                                            interval as u64,
                                                        ),
                                                        event_message: format!(
                                                            "Event starts in {}",
                                                            if interval > 3600 {
                                                                let hours = interval / 3600;
                                                                let minutes =
                                                                    (interval % 3600) / 60;
                                                                if minutes > 0 {
                                                                    format!(
                                                                        "{}h {}m",
                                                                        hours, minutes
                                                                    )
                                                                } else {
                                                                    format!("{}h", hours)
                                                                }
                                                            } else if interval > 60 {
                                                                let minutes = interval / 60;
                                                                let seconds = interval % 60;
                                                                if seconds > 0 {
                                                                    format!(
                                                                        "{}m {}s",
                                                                        minutes, seconds
                                                                    )
                                                                } else {
                                                                    format!("{}m", minutes)
                                                                }
                                                            } else {
                                                                format!("{}s", interval)
                                                            }
                                                        ),
                                                    });

                                                    interval as u64
                                                } else {
                                                    // Event has ended - interval is negative seconds since end
                                                    // Send event ended update
                                                    let _ = tx.send(QueueUpdate::EventTiming {
                                                        waiting_for_event: false,
                                                        event_starts_in_seconds: None,
                                                        event_message: format!(
                                                            "Event ended {} seconds ago",
                                                            interval.abs()
                                                        ),
                                                    });

                                                    // For ended events, we should probably stop processing
                                                    let _ = tx.send(QueueUpdate::ItemFailed {
                                                        item_index: original_index,
                                                        art_name: queue_item.art.name.clone(),
                                                        error_msg: format!(
                                                            "Event ended {} seconds ago. Event outside active window.",
                                                            interval.abs()
                                                        ),
                                                    });
                                                    return;
                                                }
                                            } else {
                                                // No interval provided - use default wait
                                                let _ = tx.send(QueueUpdate::EventTiming {
                                                    waiting_for_event: true,
                                                    event_starts_in_seconds: Some(300), // Default 5 minutes
                                                    event_message:
                                                        "Event timing unknown, waiting 5 minutes"
                                                            .to_string(),
                                                });
                                                300 // 5 minutes default
                                            };

                                            // Update display with event timing info
                                            let display_pixels_placed = pixels_placed_for_item + pixels_already_correct_at_start;
                                            let _ = tx.send(QueueUpdate::ItemProgress {
                                                item_index: original_index,
                                                art_name: queue_item.art.name.clone(),
                                                pixels_placed: display_pixels_placed,
                                                total_pixels: total_meaningful_pixels,
                                                position: (abs_x, abs_y),
                                                cooldown_remaining: Some(wait_time as u32),
                                            });

                                            // For very long waits (over 10 minutes), check periodically if event started
                                            if wait_time > 600 {
                                                let mut total_waited = 0u64;
                                                while total_waited < wait_time {
                                                    let wait_chunk = std::cmp::min(300, wait_time - total_waited); // Check every 5 minutes
                                                    tokio::time::sleep(Duration::from_secs(wait_chunk)).await;
                                                    total_waited += wait_chunk;

                                                    // Try a quick test placement to see if event started
                                                    let test_result = api_client.place_pixel(abs_x, abs_y, art_pixel.color).await;
                                                    match test_result {
                                                        Ok(_) => {
                                                            // Event started! Continue with normal placement
                                                            let _ = tx.send(QueueUpdate::EventTiming {
                                                                waiting_for_event: false,
                                                                event_starts_in_seconds: None,
                                                                event_message: "Event started! Resuming placement".to_string(),
                                                            });
                                                            let _ = tx.send(QueueUpdate::ApiCall {
                                                                message: "🎉 Event started! Resuming placement...".to_string(),
                                                            });
                                                            break;
                                                        }
                                                        Err(crate::api_client::ApiError::ErrorResponse { status, error_response }) 
                                                            if status.as_u16() == 420 => 
                                                        {
                                                            // Still waiting for event - update countdown
                                                            if let Some(new_interval) = error_response.interval {
                                                                if new_interval > 0 {
                                                                    let remaining_wait = new_interval as u64;
                                                                    
                                                                    // Update event timing
                                                                    let _ = tx.send(QueueUpdate::EventTiming {
                                                                        waiting_for_event: true,
                                                                        event_starts_in_seconds: Some(remaining_wait),
                                                                        event_message: format!("Event starts in {}", 
                                                                            if remaining_wait > 60 {
                                                                                let minutes = remaining_wait / 60;
                                                                                format!("{}m", minutes)
                                                                            } else {
                                                                                format!("{}s", remaining_wait)
                                                                            }
                                                                        ),
                                                                    });
                                                                    
                                                                    let _ = tx.send(QueueUpdate::ItemProgress {
                                                                        item_index: original_index,
                                                                        art_name: queue_item.art.name.clone(),
                                                                        pixels_placed: display_pixels_placed,
                                                                        total_pixels: total_meaningful_pixels,
                                                                        position: (abs_x, abs_y),
                                                                        cooldown_remaining: Some(remaining_wait as u32),
                                                                    });
                                                                    continue; // Continue waiting with updated time
                                                                } else {
                                                                    // Event ended while we were waiting
                                                                    let _ = tx.send(QueueUpdate::EventTiming {
                                                                        waiting_for_event: false,
                                                                        event_starts_in_seconds: None,
                                                                        event_message: format!("Event ended {} seconds ago", new_interval.abs()),
                                                                    });
                                                                    let _ = tx.send(QueueUpdate::ItemFailed {
                                                                        item_index: original_index,
                                                                        art_name: queue_item.art.name.clone(),
                                                                        error_msg: "Event ended while waiting. Event outside active window.".to_string(),
                                                                    });
                                                                    return;
                                                                }
                                                            }
                                                        }
                                                        Err(_) => {
                                                            // Some other error - might be auth, might be network
                                                            // Don't break the wait, just continue
                                                            continue;
                                                        }
                                                    }
                                                }
                                            } else {
                                                // Short wait - just wait the full duration
                                                tokio::time::sleep(Duration::from_secs(wait_time)).await;
                                            }

                                            // Continue to retry pixel placement after waiting
                                            continue;
                                        }
                                        // Check if this is a regular cooldown/rate limit error
                                        else if *status == reqwest::StatusCode::TOO_MANY_REQUESTS
                                            || status.as_u16() == 425
                                        // Too Early
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
                // Set pixels_placed to total for proper display (e.g., 4/4)
                item.pixels_placed = item.pixels_total;
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

        // Apply border-first ordering
        order_pixels_border_first(meaningful_pixels)
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

        // Apply border-first ordering
        order_pixels_border_first(meaningful_pixels)
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
        // Create queue directory if it doesn't exist
        std::fs::create_dir_all("queue")?;

        let queue_data = serde_json::to_string_pretty(&self.art_queue)?;
        std::fs::write("queue/queue.json", queue_data)?;
        Ok(())
    }

    /// Load queue from file
    pub fn load_queue(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if std::path::Path::new("queue/queue.json").exists() {
            let queue_data = std::fs::read_to_string("queue/queue.json")?;
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
                    "🔄 Auto-resuming queue processing: {} pending items found.",
                    pending_count
                ));
                self.trigger_queue_processing();
            }
        }
    }

    /// Start art sharing process
    pub fn start_art_sharing(&mut self, art: PixelArt, board_x: i32, board_y: i32) {
        self.current_share_art = Some(art.clone());
        self.current_share_coords = Some((board_x, board_y));
        self.input_mode = crate::app_state::InputMode::EnterShareMessage;
        self.input_buffer.clear();
        self.status_message = format!(
            "Sharing '{}' at ({}, {}). Enter share message (optional):",
            art.name, board_x, board_y
        );
    }

    /// Open share selection interface
    pub fn open_share_selection(&mut self) {
        // Load available shares
        self.available_shares = crate::art::get_available_shareable_arts();

        if self.available_shares.is_empty() {
            self.status_message =
                "No shared arts available. Shares are stored in the 'shares/' directory."
                    .to_string();
        } else {
            self.input_mode = crate::app_state::InputMode::ShareSelection;
            self.share_selection_index = 0;
            self.status_message = format!(
                "Found {} shared arts. Use arrows to navigate, Enter to load.",
                self.available_shares.len()
            );
        }
    }

    /// Complete art sharing by saving to shares directory
    pub fn complete_art_sharing(&mut self, share_message: Option<String>) {
        if let (Some(art), Some((board_x, board_y))) =
            (&self.current_share_art, self.current_share_coords)
        {
            let filename = format!(
                "{}_at_{}_{}.json",
                art.name.replace(' ', "_").to_lowercase(),
                board_x,
                board_y
            );
            let file_path = std::path::Path::new("shares").join(&filename);

            match crate::art::save_shareable_pixel_art(
                art,
                board_x,
                board_y,
                share_message,
                None, // Could be enhanced to include username
                &file_path,
            ) {
                Ok(()) => {
                    let share_string = crate::art::generate_share_string(art, board_x, board_y);
                    self.status_message = format!(
                        "Art shared! Saved to {}. Share string: {}",
                        filename, share_string
                    );
                }
                Err(e) => {
                    self.status_message = format!("Failed to save share: {}", e);
                }
            }
        } else {
            self.status_message = "No art to share.".to_string();
        }

        // Reset sharing state
        self.current_share_art = None;
        self.current_share_coords = None;
        self.input_mode = crate::app_state::InputMode::None;
    }

    /// Load shared art from selection
    pub fn load_shared_art(&mut self, index: usize) {
        if index < self.available_shares.len() {
            let shareable = &self.available_shares[index];
            let mut art = shareable.art.clone();
            art.board_x = shareable.board_x;
            art.board_y = shareable.board_y;

            // Move viewport to center on the art location
            let art_dimensions = crate::art::get_art_dimensions(&art);
            let art_center_x = art.board_x + art_dimensions.0 / 2;
            let art_center_y = art.board_y + art_dimensions.1 / 2;

            // Center the viewport on the art (with some padding)
            if let Some((_, _, board_width, board_height)) = self.board_area_bounds {
                // Calculate viewport position to center the art
                let viewport_x = (art_center_x - (board_width as i32 / 2)).max(0) as u16;
                let viewport_y = (art_center_y - (board_height as i32)).max(0) as u16; // *2 for half-blocks

                self.board_viewport_x = viewport_x;
                self.board_viewport_y = viewport_y;
            } else {
                // Fallback if board bounds not available
                self.board_viewport_x = (art.board_x - 25).max(0) as u16;
                self.board_viewport_y = (art.board_y - 15).max(0) as u16;
            }

            self.loaded_art = Some(art.clone());
            self.input_mode = crate::app_state::InputMode::None;

            let share_info = if let Some(msg) = &shareable.share_message {
                format!(" ({})", msg)
            } else {
                String::new()
            };

            self.status_message = format!(
                "Loaded shared art '{}' at ({}, {}){}. Viewport moved to art location. Use arrows to reposition or Enter to queue.",
                art.name, art.board_x, art.board_y, share_info
            );
        }
    }

    /// Parse and apply share string
    pub fn apply_share_string(&mut self, share_string: &str) {
        if let Some((art_name, x, y)) = crate::art::parse_share_string(share_string) {
            // Find matching art in available arts
            let available_arts = crate::art::get_available_pixel_arts();
            if let Some(mut art) = available_arts.into_iter().find(|a| a.name == art_name) {
                art.board_x = x;
                art.board_y = y;

                // Move viewport to center on the art location
                let art_dimensions = crate::art::get_art_dimensions(&art);
                let art_center_x = art.board_x + art_dimensions.0 / 2;
                let art_center_y = art.board_y + art_dimensions.1 / 2;

                // Center the viewport on the art (with some padding)
                if let Some((_, _, board_width, board_height)) = self.board_area_bounds {
                    // Calculate viewport position to center the art
                    let viewport_x = (art_center_x - (board_width as i32 / 2)).max(0) as u16;
                    let viewport_y = (art_center_y - (board_height as i32)).max(0) as u16; // *2 for half-blocks

                    self.board_viewport_x = viewport_x;
                    self.board_viewport_y = viewport_y;
                } else {
                    // Fallback if board bounds not available
                    self.board_viewport_x = (art.board_x - 25).max(0) as u16;
                    self.board_viewport_y = (art.board_y - 15).max(0) as u16;
                }

                self.loaded_art = Some(art.clone());
                self.input_mode = crate::app_state::InputMode::None;
                self.status_message = format!(
                    "Applied share coordinates: '{}' positioned at ({}, {}). Viewport moved to art location.",
                    art.name, x, y
                );
            } else {
                self.status_message = format!(
                    "Art '{}' not found in available arts. Coordinates: ({}, {})",
                    art_name, x, y
                );
            }
        } else {
            self.status_message =
                "Invalid share string format. Expected: ftplace-share: NAME at (X, Y)".to_string();
        }
    }

    /// Center viewport on the currently selected queue item
    pub fn center_viewport_on_selected_queue_item(&mut self) {
        if self.queue_selection_index >= self.art_queue.len() {
            return; // Invalid selection index
        }

        let selected_art = &self.art_queue[self.queue_selection_index].art;

        // Get art dimensions to center it properly
        let art_dimensions = crate::art::get_art_dimensions(selected_art);
        let art_center_x = selected_art.board_x + art_dimensions.0 / 2;
        let art_center_y = selected_art.board_y + art_dimensions.1 / 2;

        // Center the viewport on the art (with some padding)
        if let Some((_, _, board_width, board_height)) = self.board_area_bounds {
            // Calculate viewport position to center the art
            let viewport_x = (art_center_x - (board_width as i32 / 2)).max(0) as u16;
            let viewport_y = (art_center_y - (board_height as i32)).max(0) as u16; // *2 for half-blocks

            self.board_viewport_x = viewport_x;
            self.board_viewport_y = viewport_y;
        } else {
            // Fallback if board bounds not available
            self.board_viewport_x = (selected_art.board_x - 25).max(0) as u16;
            self.board_viewport_y = (selected_art.board_y - 15).max(0) as u16;
        }

        self.status_message = format!(
            "Centered viewport on '{}' at ({}, {})",
            selected_art.name, selected_art.board_x, selected_art.board_y
        );
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

/// Order pixels with border-first strategy: borders first, then top-to-bottom fill
/// This is a standalone function that can be used by both queue_management and art_placement
pub fn order_pixels_border_first(
    mut pixels: Vec<crate::art::ArtPixel>,
) -> Vec<crate::art::ArtPixel> {
    if pixels.is_empty() {
        return pixels;
    }

    // Find the bounding box of all pixels
    let min_x = pixels.iter().map(|p| p.x).min().unwrap();
    let max_x = pixels.iter().map(|p| p.x).max().unwrap();
    let min_y = pixels.iter().map(|p| p.y).min().unwrap();
    let max_y = pixels.iter().map(|p| p.y).max().unwrap();

    // Create a set of all pixel positions for fast lookup
    let pixel_positions: std::collections::HashSet<(i32, i32)> =
        pixels.iter().map(|p| (p.x, p.y)).collect();

    // Separate border pixels from interior pixels
    let mut border_pixels = Vec::new();
    let mut interior_pixels = Vec::new();

    for pixel in pixels.drain(..) {
        let x = pixel.x;
        let y = pixel.y;

        // A pixel is on the border if:
        // 1. It's on the edge of the bounding box, OR
        // 2. It has at least one adjacent position (4-directional) that doesn't contain a pixel
        let is_border = x == min_x
            || x == max_x
            || y == min_y
            || y == max_y
            || !pixel_positions.contains(&(x - 1, y))
            || !pixel_positions.contains(&(x + 1, y))
            || !pixel_positions.contains(&(x, y - 1))
            || !pixel_positions.contains(&(x, y + 1));

        if is_border {
            border_pixels.push(pixel);
        } else {
            interior_pixels.push(pixel);
        }
    }

    // Sort border pixels: top-to-bottom, left-to-right
    border_pixels.sort_by(|a, b| a.y.cmp(&b.y).then_with(|| a.x.cmp(&b.x)));

    // Sort interior pixels: top-to-bottom, left-to-right
    interior_pixels.sort_by(|a, b| a.y.cmp(&b.y).then_with(|| a.x.cmp(&b.x)));

    // Combine: borders first, then interior
    let mut result = border_pixels;
    result.extend(interior_pixels);
    result
}
