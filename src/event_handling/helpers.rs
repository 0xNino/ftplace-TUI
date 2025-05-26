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
        // Add to history with timestamp
        self.status_messages
            .push_back((message.clone(), Instant::now()));

        // Keep only last 100 messages (increased from 5)
        while self.status_messages.len() > 100 {
            self.status_messages.pop_front();
        }

        // Update main status message
        self.status_message = message;
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

                            // Create a loading bar for each timer
                            let total_cooldown_secs = user_info.pixel_timer as u64 / 1000;
                            let progress = if total_cooldown_secs > 0 {
                                ((total_cooldown_secs - remaining_secs) as f64
                                    / total_cooldown_secs as f64)
                                    .min(1.0)
                                    .max(0.0)
                            } else {
                                0.0
                            };

                            let bar_width = 8;
                            let filled_width = (progress * bar_width as f64) as usize;
                            let empty_width = bar_width - filled_width;

                            let bar =
                                format!("{}{}", "█".repeat(filled_width), "░".repeat(empty_width));

                            if remaining_secs > 60 {
                                let minutes = remaining_secs / 60;
                                let seconds = remaining_secs % 60;
                                active_timers.push(format!(
                                    "T{}[{}]{}m{}s",
                                    i + 1,
                                    bar,
                                    minutes,
                                    seconds
                                ));
                            } else {
                                active_timers.push(format!(
                                    "T{}[{}]{}s",
                                    i + 1,
                                    bar,
                                    remaining_secs
                                ));
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
                            format!("{}m{}s", minutes, seconds)
                        } else {
                            format!("{}s", next_remaining_secs)
                        };

                        self.cooldown_status = format!(
                            "Next pixel: {} | All: {}",
                            next_pixel_str,
                            active_timers.join(", ")
                        );
                    }
                } else {
                    self.cooldown_status = format!(
                        "No active timers - Cooldown: {}s",
                        user_info.pixel_timer / 1000
                    );
                }
            } else {
                self.cooldown_status = format!(
                    "No timers data - Cooldown: {}s",
                    user_info.pixel_timer / 1000
                );
            }
        } else {
            self.cooldown_status = "No user info available - use 'p' to fetch profile".to_string();
        }
    }

    /// Clean up old status messages (older than 10 minutes)
    pub fn cleanup_old_status_messages(&mut self) {
        let cutoff = Instant::now() - Duration::from_secs(600); // 10 minutes instead of 30 seconds
        while let Some((_, timestamp)) = self.status_messages.front() {
            if *timestamp < cutoff {
                self.status_messages.pop_front();
            } else {
                break;
            }
        }
    }

    /// Check if tokens were refreshed and save them if needed
    #[allow(dead_code)]
    pub async fn check_and_save_refreshed_tokens(&mut self) {
        // This will be called after API operations that might refresh tokens
        self.save_tokens();
    }
}
