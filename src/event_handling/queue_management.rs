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
                self.status_message = format!(
                    "Queue processing: Starting item {}/{} - '{}'",
                    item_index + 1,
                    total_items,
                    art_name
                );
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
                }

                self.status_message = format!(
                    "Queue item {}: '{}' completed - {}/{} pixels placed",
                    item_index + 1,
                    art_name,
                    pixels_placed,
                    total_pixels
                );
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

                self.status_message = format!(
                    "Queue item {}: '{}' failed - {}",
                    item_index + 1,
                    art_name,
                    error_msg
                );
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

                self.status_message = format!(
                    "Queue item {}: '{}' skipped - {}",
                    item_index + 1,
                    art_name,
                    reason
                );
            }
            QueueUpdate::QueueCompleted {
                total_items_processed,
                total_pixels_placed,
                duration_secs,
            } => {
                self.status_message = format!(
					"Queue processing complete! {} items processed, {} pixels placed in {}s. Refreshing board...",
					total_items_processed,
					total_pixels_placed,
					duration_secs
				);

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
                self.status_message = format!(
					"Queue processing cancelled: {} items processed, {} pixels placed. Press 'r' to refresh board.",
					items_processed,
					total_pixels_placed
				);

                // Reset queue processing state
                self.queue_processing = false;
                self.queue_processing_start = None;
                self.queue_receiver = None;
            }
        }
    }

    /// Add an art to the placement queue
    pub async fn add_art_to_queue(&mut self, art: PixelArt) {
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

        // Clone API client data and queue data needed for processing
        let base_url = self.api_client.get_base_url();
        let access_token = self.api_client.get_access_token_clone();
        let refresh_token = self.api_client.get_refresh_token_clone();
        let board_state = self.board.clone(); // Clone board state for pixel checking
        let queue_items: Vec<_> = self
            .art_queue
            .iter()
            .enumerate()
            .filter(|(_, item)| item.status == QueueStatus::Pending)
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

            for (original_index, queue_item) in queue_items {
                // Send item started update
                let _ = tx.send(QueueUpdate::ItemStarted {
                    item_index: original_index,
                    art_name: queue_item.art.name.clone(),
                    total_items: processed_count + 1, // Will be corrected as we process
                });

                // Filter meaningful pixels for this art and exclude already-correct pixels
                let meaningful_pixels = Self::filter_meaningful_pixels_static(&queue_item.art);
                let total_meaningful_pixels = meaningful_pixels.len(); // Calculate length before move
                let pixels_to_place: Vec<_> = meaningful_pixels
                    .into_iter()
                    .enumerate()
                    .filter(|(_, art_pixel)| {
                        let abs_x = queue_item.art.board_x + art_pixel.x;
                        let abs_y = queue_item.art.board_y + art_pixel.y;
                        // Only include pixels that need to be changed
                        !Self::is_pixel_already_correct_static(
                            &board_state,
                            abs_x,
                            abs_y,
                            art_pixel.color,
                        )
                    })
                    .collect();

                if pixels_to_place.is_empty() {
                    // Send skip update - all pixels already correct
                    let _ = tx.send(QueueUpdate::ItemSkipped {
                        item_index: original_index,
                        art_name: queue_item.art.name.clone(),
                        reason: "All pixels already correct".to_string(),
                    });
                    continue;
                }

                let mut pixels_placed_for_item = 0;
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

                            let _ = tx.send(QueueUpdate::ItemProgress {
                                item_index: original_index,
                                art_name: queue_item.art.name.clone(),
                                pixels_placed: pixels_placed_for_item,
                                total_pixels: total_meaningful_pixels,
                                position: (abs_x, abs_y),
                                cooldown_remaining: Some(wait_time as u32),
                            });

                            // For very long waits, check every minute if cooldown changed
                            let mut remaining_wait = wait_time;
                            while remaining_wait > 60 {
                                tokio::time::sleep(Duration::from_secs(60)).await;
                                remaining_wait = remaining_wait.saturating_sub(60);

                                let _ = tx.send(QueueUpdate::ItemProgress {
                                    item_index: original_index,
                                    art_name: queue_item.art.name.clone(),
                                    pixels_placed: pixels_placed_for_item,
                                    total_pixels: total_meaningful_pixels,
                                    position: (abs_x, abs_y),
                                    cooldown_remaining: Some(remaining_wait as u32),
                                });
                            }

                            // Wait the remaining time (less than 1 minute)
                            if remaining_wait > 0 {
                                tokio::time::sleep(Duration::from_secs(remaining_wait)).await;
                            }
                        } else if wait_time > 0 {
                            // Short cooldown - wait normally
                            let _ = tx.send(QueueUpdate::ItemProgress {
                                item_index: original_index,
                                art_name: queue_item.art.name.clone(),
                                pixels_placed: pixels_placed_for_item,
                                total_pixels: total_meaningful_pixels,
                                position: (abs_x, abs_y),
                                cooldown_remaining: Some(wait_time as u32),
                            });

                            tokio::time::sleep(Duration::from_secs(wait_time)).await;
                        }
                    }

                    // Send placement progress update
                    let _ = tx.send(QueueUpdate::ItemProgress {
                        item_index: original_index,
                        art_name: queue_item.art.name.clone(),
                        pixels_placed: pixels_placed_for_item,
                        total_pixels: total_meaningful_pixels,
                        position: (abs_x, abs_y),
                        cooldown_remaining: None,
                    });

                    // Attempt to place the pixel (with minimal retries for cooldown errors)
                    let mut pixel_placement_success = false;
                    const MAX_RETRIES: u32 = 1; // Reduced retries since we wait properly now

                    for retry_attempt in 0..=MAX_RETRIES {
                        match api_client.place_pixel(abs_x, abs_y, art_pixel.color).await {
                            Ok(response) => {
                                pixels_placed_for_item += 1;
                                total_pixels_placed += 1;
                                user_info = Some(response.user_infos);
                                pixel_placement_success = true;
                                break; // Successfully placed, move to next pixel
                            }
                            Err(e) => {
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

                                            // For cooldown errors, wait immediately and only retry once
                                            if retry_attempt < MAX_RETRIES {
                                                if let Some(ref info) = user_info {
                                                    let (_, wait_time) =
                                                        should_pause_queue_processing(info);

                                                    let _ = tx.send(QueueUpdate::ItemProgress {
                                                        item_index: original_index,
                                                        art_name: queue_item.art.name.clone(),
                                                        pixels_placed: pixels_placed_for_item,
                                                        total_pixels: total_meaningful_pixels,
                                                        position: (abs_x, abs_y),
                                                        cooldown_remaining: Some(wait_time as u32),
                                                    });

                                                    // Wait for the cooldown period
                                                    tokio::time::sleep(Duration::from_secs(
                                                        wait_time,
                                                    ))
                                                    .await;
                                                }
                                                // Continue to retry after waiting
                                                continue;
                                            } else {
                                                // Max retries reached for this pixel - skip it and continue with next
                                                // The cooldown will be respected when we start the next pixel
                                                break;
                                            }
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

                    // Small delay between pixels (only when successful)
                    if pixel_placement_success {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }

                // Send item completion update
                let _ = tx.send(QueueUpdate::ItemCompleted {
                    item_index: original_index,
                    art_name: queue_item.art.name.clone(),
                    pixels_placed: pixels_placed_for_item,
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

            // Place the pixel
            match self
                .api_client
                .place_pixel(abs_x, abs_y, art_pixel.color)
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

/// Calculate how long to wait before we can place a pixel based on user timers and buffer
pub fn calculate_cooldown_wait_time(user_info: &UserInfos) -> u64 {
    // If we have pixel buffer available, we can place immediately
    if user_info.pixel_buffer > 0 {
        return 0;
    }

    // No buffer available, check timers to see when we can place next
    if let Some(timers) = &user_info.timers {
        if timers.is_empty() {
            // No active timers, use pixel_timer as fallback but be conservative
            let fallback_time = (user_info.pixel_timer as f64 / 1000.0) as u64;
            return fallback_time.max(5); // Minimum 5 seconds
        }

        // Find the earliest timer that will expire (most important change)
        let current_time_ms = chrono::Utc::now().timestamp_millis();
        let mut earliest_expiry = i64::MAX;

        for &timer_ms in timers {
            if timer_ms > current_time_ms && timer_ms < earliest_expiry {
                earliest_expiry = timer_ms;
            }
        }

        if earliest_expiry == i64::MAX {
            // All timers have expired - we should be able to place now
            return 0;
        }

        // Calculate exact wait time in seconds
        let wait_time_ms = earliest_expiry - current_time_ms;
        let wait_time_secs = (wait_time_ms as f64 / 1000.0).ceil() as u64;

        // Return the exact time (no artificial minimums for accurate timing)
        wait_time_secs + 1 // Just 1 second buffer for timing precision
    } else {
        // No timer data, fall back to pixel_timer but be conservative
        let fallback_time = (user_info.pixel_timer as f64 / 1000.0) as u64;
        fallback_time.max(5) // Minimum 5 seconds
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
