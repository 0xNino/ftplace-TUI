use crate::api_client::ApiError;
use crate::app_state::{App, InputMode, ProfileFetchResult};
use tokio::sync::mpsc;

impl App {
    /// Handle profile fetch results from background profile fetch tasks
    pub fn handle_profile_fetch_result(&mut self, result: ProfileFetchResult) {
        match result {
            ProfileFetchResult::Success(user_infos) => {
                self.add_status_message(format!(
                    "Profile: {}, Pixels: {}, Cooldown: {}s, User Timers: {}",
                    user_infos.username.as_deref().unwrap_or("N/A"),
                    user_infos.pixel_buffer,
                    user_infos.pixel_timer,
                    user_infos.timers.as_ref().map_or(0, |v| v.len())
                ));
                self.user_info = Some(user_infos);
                // Save tokens in case they were refreshed during the API call
                self.save_tokens();
            }
            ProfileFetchResult::Error(error_msg) => {
                self.user_info = None;
                self.add_status_message(format!(
                    "Error fetching profile: {}. Try 'p' to retry.",
                    error_msg
                ));
            }
        }

        // Reset profile fetch state
        self.profile_receiver = None;
    }

    /// Trigger non-blocking profile fetch
    pub fn trigger_profile_fetch(&mut self) {
        if self.api_client.get_auth_cookie_preview().is_none() {
            self.status_message =
                "Cannot fetch profile: Access Token not set. Please enter it.".to_string();
            self.input_mode = InputMode::EnterAccessToken;
            self.input_buffer.clear();
            return;
        }

        // Create channel for profile fetch
        let (tx, rx) = mpsc::unbounded_channel();
        self.profile_receiver = Some(rx);

        // Clone API client data needed for the fetch
        let base_url = self.api_client.get_base_url();
        let access_token = self.api_client.get_access_token_clone();
        let refresh_token = self.api_client.get_refresh_token_clone();
        let _colors = self.colors.clone();

        self.status_message = "Fetching profile data...".to_string();

        // Spawn async task for profile fetching
        tokio::spawn(async move {
            let mut api_client =
                crate::api_client::ApiClient::new(Some(base_url), access_token, refresh_token);

            let result = match api_client.get_profile().await {
                Ok(profile_response) => ProfileFetchResult::Success(profile_response.user_infos),
                Err(e) => {
                    let error_msg = match e {
                        crate::api_client::ApiError::Unauthorized => {
                            "Unauthorized. Access Token might be invalid or expired".to_string()
                        }
                        _ => format!("{:?}", e),
                    };
                    ProfileFetchResult::Error(error_msg)
                }
            };

            // Send result back - if this fails, the main app has been dropped
            let _ = tx.send(result);
        });
    }

    /// Legacy profile fetch method for compatibility
    #[allow(dead_code)]
    pub async fn fetch_profile_data(&mut self) {
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
}
