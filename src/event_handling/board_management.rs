use crate::api_client::ApiError;
use crate::app_state::{App, BoardFetchResult};
use std::time::Instant;
use tokio::sync::mpsc;

impl App {
    /// Trigger a non-blocking board fetch if one isn't already in progress
    pub fn trigger_board_fetch(&mut self) {
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
        let _colors = self.colors.clone();

        // Add API call log to status messages
        self.log_api_call("GET", "/api/get", None);

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
    pub fn handle_board_fetch_result(&mut self, result: BoardFetchResult) {
        let load_time = self
            .board_load_start
            .map(|start| start.elapsed().as_millis())
            .unwrap_or(0);

        match result {
            BoardFetchResult::Success(board_response) => {
                // Log successful API call
                self.log_api_call("GET", "/api/get", Some(200));

                self.board = board_response.board;
                self.colors = board_response.colors;

                // Update shared board state if it exists (for queue processing)
                if let Some(shared_board) = &self.shared_board_state {
                    if let Ok(mut board_lock) = shared_board.write() {
                        *board_lock = self.board.clone();
                    }
                }

                // Set status message directly without adding to history to avoid overriding other logs
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

                // Recalculate queue totals now that we have updated board data
                self.recalculate_queue_totals();

                // Check if queue should auto-resume (after app restart)
                self.check_auto_resume_queue();

                // Save tokens in case they were refreshed during the API call
                self.save_tokens();
            }
            BoardFetchResult::Error(error_msg) => {
                // Try to extract status code from error message for logging
                let status_code = if error_msg.contains("401") || error_msg.contains("Unauthorized")
                {
                    Some(401)
                } else if error_msg.contains("403") || error_msg.contains("Forbidden") {
                    Some(403)
                } else if error_msg.contains("404") {
                    Some(404)
                } else if error_msg.contains("500") {
                    Some(500)
                } else {
                    None
                };

                if let Some(code) = status_code {
                    self.log_api_call("GET", "/api/get", Some(code));
                }

                // Set status message directly without adding to history to avoid overriding other logs
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

    /// Legacy board fetch method for compatibility
    pub async fn fetch_board_data(&mut self) {
        // If not triggered by trigger_board_fetch, set up loading state
        if !self.board_loading {
            self.board_loading = true;
            self.board_load_start = Some(Instant::now());
            self.status_message = "Fetching board data...".to_string();
        }

        // Add API call log to status messages
        self.log_api_call("GET", "/api/get", None);

        match self.api_client.get_board().await {
            Ok(board_response) => {
                // Log successful API call
                self.log_api_call("GET", "/api/get", Some(200));

                self.board = board_response.board;
                self.colors = board_response.colors;

                // Update shared board state if it exists (for queue processing)
                if let Some(shared_board) = &self.shared_board_state {
                    if let Ok(mut board_lock) = shared_board.write() {
                        *board_lock = self.board.clone();
                    }
                }

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

                // Recalculate queue totals now that we have updated board data
                self.recalculate_queue_totals();

                // Save tokens in case they were refreshed during the API call
                self.save_tokens();
            }
            Err(e) => {
                let load_time = self
                    .board_load_start
                    .map(|start| start.elapsed().as_millis())
                    .unwrap_or(0);

                // Log API error with status code
                match &e {
                    ApiError::ErrorResponse { status, .. } => {
                        self.log_api_call("GET", "/api/get", Some(status.as_u16()));
                    }
                    ApiError::Unauthorized => {
                        self.log_api_call("GET", "/api/get", Some(401));
                    }
                    _ => {
                        // For network errors or other issues, log without status
                        self.log_api_call("GET", "/api/get", None);
                    }
                }

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
}
