use crate::api_client::UserInfos;
use crate::app_state::App;
use std::time::{Duration, Instant};

impl App {
    /// Enhanced error message formatting that utilizes timers and interval from ErrorResponse
    pub fn format_enhanced_error_message(
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

    /// Enhanced error handling for API operations that uses the ErrorResponse fields
    pub async fn handle_api_error_with_enhanced_display(
        &mut self,
        base_message: &str,
        error: &crate::api_client::ApiError,
    ) {
        match error {
            crate::api_client::ApiError::ErrorResponse {
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
                        if let Some(ref mut info) = self.user_info {
                            info.timers = Some(timers.clone());
                        } else {
                            // Create minimal user info if we don't have it
                            self.user_info = Some(UserInfos {
                                timers: Some(timers.clone()),
                                pixel_buffer: 0,
                                pixel_timer: error_response.interval.unwrap_or(5000) as i32,
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
    pub fn update_blink_state(&mut self) {
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

    /// Add a new status message to the history and update the main status
    pub fn add_status_message(&mut self, message: String) {
        // Generate UTC+2 timestamp
        let now = chrono::Utc::now() + chrono::Duration::hours(2);
        let timestamp_utc2 = now.format("%Y-%m-%d %H:%M:%S").to_string();

        // Add to history with timestamp
        self.status_messages
            .push_back((message.clone(), Instant::now(), timestamp_utc2));

        // Keep only last 100 messages (increased from 5)
        while self.status_messages.len() > 100 {
            self.status_messages.pop_front();
        }

        // Note: We no longer overwrite status_message here to preserve contextual guidance
    }

    /// Update the persistent cooldown status
    pub fn update_cooldown_status(&mut self) {
        if let Some(user_info) = &self.user_info {
            let available_pixels = if let Some(timers) = &user_info.timers {
                user_info.pixel_buffer - timers.len() as i32
            } else {
                user_info.pixel_buffer
            };

            if available_pixels > 0 {
                self.cooldown_status = "Ready to place pixels".to_string();
            } else if let Some(timers) = &user_info.timers {
                if !timers.is_empty() {
                    let current_time_ms = chrono::Utc::now().timestamp_millis();
                    let mut active_timers = Vec::new();
                    let mut next_available_ms = i64::MAX;

                    for (i, &timer_ms) in timers.iter().enumerate() {
                        let remaining_ms = timer_ms - current_time_ms;
                        if remaining_ms > 0 {
                            let remaining_secs = (remaining_ms as f64 / 1000.0).ceil() as u64;

                            // Format timer without progress bar - cleaner display with fixed width
                            if remaining_secs > 60 {
                                let minutes = remaining_secs / 60;
                                let seconds = remaining_secs % 60;
                                if seconds > 0 {
                                    active_timers.push(format!(
                                        "T{}:{:2}m{:02}s",
                                        i + 1,
                                        minutes,
                                        seconds
                                    ));
                                } else {
                                    active_timers.push(format!("T{}:{:2}m   ", i + 1, minutes));
                                }
                            } else {
                                active_timers.push(format!("T{}:{:2}s   ", i + 1, remaining_secs));
                            }

                            // Track the earliest timer for next pixel available
                            if timer_ms < next_available_ms {
                                next_available_ms = timer_ms;
                            }
                        }
                    }

                    if active_timers.is_empty() {
                        self.cooldown_status = "Ready to place pixels".to_string();
                    } else {
                        // Calculate next pixel available time
                        let next_remaining_ms = next_available_ms - current_time_ms;
                        let next_remaining_secs = (next_remaining_ms as f64 / 1000.0).ceil() as u64;

                        let next_pixel_str = if next_remaining_secs > 60 {
                            let minutes = next_remaining_secs / 60;
                            let seconds = next_remaining_secs % 60;
                            if seconds > 0 {
                                format!("{:2}m{:02}s", minutes, seconds)
                            } else {
                                format!("{:2}m    ", minutes)
                            }
                        } else {
                            format!("{:2}s    ", next_remaining_secs)
                        };

                        self.cooldown_status = format!(
                            "Next pixel: {} | Timers: {}",
                            next_pixel_str,
                            active_timers.join(", ")
                        );
                    }
                } else {
                    self.cooldown_status = format!(
                        "No active timers - Cooldown: {}s",
                        user_info.pixel_timer * 60 // Convert minutes to seconds
                    );
                }
            } else {
                self.cooldown_status = format!(
                    "No timers data - Cooldown: {}s",
                    user_info.pixel_timer * 60 // Convert minutes to seconds
                );
            }
        } else {
            self.cooldown_status = "No user info available - use 'p' to fetch profile".to_string();
        }
    }

    /// Get formatted timer status for display in headers
    pub fn get_formatted_timer_status(&self) -> String {
        if let Some(user_info) = &self.user_info {
            let available_pixels = if let Some(timers) = &user_info.timers {
                user_info.pixel_buffer - timers.len() as i32
            } else {
                user_info.pixel_buffer
            };

            if available_pixels >= user_info.pixel_buffer {
                return "🟢 All pixels available".to_string();
            }

            if let Some(timers) = &user_info.timers {
                if !timers.is_empty() {
                    return self.create_timer_timeline(user_info, timers, available_pixels);
                }
            }

            format!(
                "🟡 No active timers - Cooldown: {}min",
                user_info.pixel_timer
            )
        } else {
            "⚪ No user info - use 'p' to fetch profile".to_string()
        }
    }

    /// Create a timeline progress bar showing all timers
    fn create_timer_timeline(
        &self,
        user_info: &UserInfos,
        timers: &[i64],
        available_pixels: i32,
    ) -> String {
        let current_time_ms = chrono::Utc::now().timestamp_millis();
        let total_cooldown_secs = (user_info.pixel_timer * 60) as f64; // Convert minutes to seconds

        // Timeline parameters
        const TIMELINE_WIDTH: usize = 50;
        const AVAILABLE_CHAR: char = '█';
        const COOLDOWN_CHAR: char = '░';
        const TIMER_CHAR: char = '●';
        const SOON_CHAR: char = '○'; // For timers finishing soon (< 2 min)

        // Calculate available zone width
        let available_zone_width = if available_pixels > 0 {
            ((available_pixels as f64 / user_info.pixel_buffer as f64) * TIMELINE_WIDTH as f64)
                as usize
        } else {
            0
        };

        // Create base timeline
        let mut timeline: Vec<char> = Vec::with_capacity(TIMELINE_WIDTH);

        // Fill available zone
        for _ in 0..available_zone_width.min(TIMELINE_WIDTH) {
            timeline.push(AVAILABLE_CHAR);
        }

        // Fill cooldown zone
        for _ in available_zone_width..TIMELINE_WIDTH {
            timeline.push(COOLDOWN_CHAR);
        }

        // Place timer markers
        let mut next_available_secs = f64::MAX;
        let mut active_timer_count = 0;

        for &timer_ms in timers {
            let remaining_ms = timer_ms - current_time_ms;
            if remaining_ms > 0 {
                let remaining_secs = remaining_ms as f64 / 1000.0;
                next_available_secs = next_available_secs.min(remaining_secs);
                active_timer_count += 1;

                // Calculate position on timeline (0 = available, TIMELINE_WIDTH = full cooldown)
                let position_ratio = remaining_secs / total_cooldown_secs;
                let timeline_position = (position_ratio * TIMELINE_WIDTH as f64) as usize;

                if timeline_position < TIMELINE_WIDTH {
                    // Use different char for timers finishing soon
                    let timer_symbol = if remaining_secs < 120.0 {
                        // Less than 2 minutes
                        SOON_CHAR
                    } else {
                        TIMER_CHAR
                    };
                    timeline[timeline_position] = timer_symbol;
                }
            }
        }

        // Create timeline string
        let timeline_str: String = timeline.into_iter().collect();

        // Format next available time
        let next_time_str = if next_available_secs != f64::MAX {
            let next_secs = next_available_secs as u64;
            if next_secs > 60 {
                let minutes = next_secs / 60;
                let seconds = next_secs % 60;
                if seconds > 0 {
                    format!("{}m{:02}s", minutes, seconds)
                } else {
                    format!("{}m", minutes)
                }
            } else {
                format!("{}s", next_secs)
            }
        } else {
            "ready".to_string()
        };

        // Create labels
        let buffer_status = format!("Buffer: {}/{}", available_pixels, user_info.pixel_buffer);

        // Create properly aligned timeline labels
        let total_minutes = total_cooldown_secs as u64 / 60;
        let left_label = "0min";
        let right_label = format!("{}min", total_minutes);

        // Calculate spacing to align right label with end of timeline
        let spacing_needed = TIMELINE_WIDTH.saturating_sub(left_label.len() + right_label.len());
        let timeline_labels = format!(
            "{}{}{}",
            left_label,
            " ".repeat(spacing_needed),
            right_label
        );

        format!(
            "Pixel Cooldown Timeline\n{}\n{}\nNext: {} | {} | {} timers ● active ○ soon",
            timeline_labels, timeline_str, next_time_str, buffer_status, active_timer_count
        )
    }

    /// Clean up old status messages (older than 10 minutes)
    pub fn cleanup_old_status_messages(&mut self) {
        let cutoff = Instant::now() - Duration::from_secs(600); // 10 minutes instead of 30 seconds
        while let Some((_, timestamp, _)) = self.status_messages.front() {
            if *timestamp < cutoff {
                self.status_messages.pop_front();
            } else {
                break;
            }
        }
    }

    /// Update event timer status to refresh countdown display
    pub fn update_event_timer_status(&mut self) {
        if self.waiting_for_event {
            if let Some(event_start_time) = self.event_start_time {
                // Check if event has started
                if let Ok(_elapsed_since_start) =
                    std::time::SystemTime::now().duration_since(event_start_time)
                {
                    // Event has started, clear waiting state
                    self.waiting_for_event = false;
                    self.event_start_time = None;
                    self.event_end_time = None;
                    self.last_event_check_time = None;
                }
            }
        }
    }

    /// Check if tokens were refreshed and save them if needed
    #[allow(dead_code)]
    pub async fn check_and_save_refreshed_tokens(&mut self) {
        // This will be called after API operations that might refresh tokens
        self.save_tokens();
    }

    /// Log API call with status code
    pub fn log_api_call(&mut self, method: &str, endpoint: &str, status_code: Option<u16>) {
        let emoji = match method {
            "GET" => "📡",
            "POST" => "🎨",
            _ => "🔗",
        };

        let status_text = match status_code {
            Some(code) => {
                let status_emoji = match code {
                    200..=299 => "✅",
                    400..=499 => "❌",
                    500..=599 => "💥",
                    _ => "❓",
                };
                format!(" → {} {:3}", status_emoji, code)
            }
            None => " → ⏳    ".to_string(), // Request initiated - same width as status codes
        };

        self.add_status_message(format!("{} {} {}{}", emoji, method, endpoint, status_text));
    }

    /// Save status messages to file for persistence between runs
    pub fn save_status_messages(&self) -> Result<(), Box<dyn std::error::Error>> {
        use serde::{Deserialize, Serialize};

        #[derive(Serialize, Deserialize)]
        struct PersistentStatusMessage {
            message: String,
            timestamp_utc: String, // Store as UTC+2 formatted string
        }

        let persistent_messages: Vec<PersistentStatusMessage> = self
            .status_messages
            .iter()
            .map(
                |(message, _instant, utc2_timestamp)| PersistentStatusMessage {
                    message: message.clone(),
                    timestamp_utc: utc2_timestamp.clone(),
                },
            )
            .collect();

        // Create logs directory if it doesn't exist
        std::fs::create_dir_all("logs")?;

        let json_data = serde_json::to_string_pretty(&persistent_messages)?;
        std::fs::write("logs/status_messages.json", json_data)?;
        Ok(())
    }

    /// Load status messages from file for persistence between runs
    pub fn load_status_messages(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        use serde::{Deserialize, Serialize};

        #[derive(Serialize, Deserialize)]
        struct PersistentStatusMessage {
            message: String,
            timestamp_utc: String,
        }

        if !std::path::Path::new("logs/status_messages.json").exists() {
            return Ok(());
        }

        let json_data = std::fs::read_to_string("logs/status_messages.json")?;
        let persistent_messages: Vec<PersistentStatusMessage> = serde_json::from_str(&json_data)?;

        // Convert back to runtime format with current Instant (for cleanup purposes)
        // We'll use the stored UTC+2 timestamp for display
        let now = Instant::now();
        for persistent_msg in persistent_messages {
            self.status_messages.push_back((
                persistent_msg.message,
                now,
                persistent_msg.timestamp_utc,
            ));
        }

        Ok(())
    }
}
