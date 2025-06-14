use reqwest::header::{CONTENT_TYPE, COOKIE};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::fs::File; // For file logging
use std::io::Write; // For file logging

// API Endpoint Base URL - can be configured later
const API_BASE_URL: &str = "https://ftplace.42lausanne.ch"; // TODO: Make this configurable

// Callback type for when tokens are refreshed
pub type TokenRefreshCallback = Box<dyn Fn(Option<String>, Option<String>) + Send + Sync>;

#[derive(Deserialize, Debug, Clone)]
pub struct ColorInfo {
    pub id: i32, // Assuming color ID is an integer
    pub name: String,
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PixelNetwork {
    pub c: i32, // color_id
    #[allow(dead_code)]
    pub u: String, // username - could be useful for future features showing who placed pixels
    #[allow(dead_code)]
    pub t: i64, // set_time (timestamp) - could be useful for pixel history/timeline features
}

#[derive(Deserialize, Debug)]
pub struct BoardGetResponse {
    pub colors: Vec<ColorInfo>,
    pub board: Vec<Vec<Option<PixelNetwork>>>,
    // Optional fields for admin view (min_time, max_time, type)
    #[allow(dead_code)]
    pub r#type: Option<String>, // "board" or "image" - for future admin features
    #[allow(dead_code)]
    pub min_time: Option<i64>, // for future admin/filtering features
    #[allow(dead_code)]
    pub max_time: Option<i64>, // for future admin/filtering features
}

#[derive(Deserialize, Debug)]
pub struct UserPixelTimer {
    // The backend returns an array of timestamps (milliseconds since epoch)
    // For simplicity, we might just store them as raw numbers or wrap them.
    // Let's assume it's Vec<i64> for now.
    // This might actually be part of UserInfos based on backend code.
}

#[derive(Deserialize, Debug)]
pub struct UserInfos {
    pub timers: Option<Vec<i64>>, // Changed to Option<Vec<i64>>
    pub pixel_buffer: i32,
    pub pixel_timer: i32,
    pub id: Option<i32>,
    pub username: Option<String>,
    pub soft_is_admin: Option<bool>,
    pub soft_is_banned: Option<bool>,

    // Fields observed from the actual /api/profile response
    pub num: Option<i32>,    // Assuming i32, make optional for safety
    pub min_px: Option<i32>, // Assuming i32, make optional for safety
    pub campus_name: Option<String>,
    pub iat: Option<i64>, // JWT issued-at timestamp
    pub exp: Option<i64>, // JWT expiration timestamp
}

#[derive(Deserialize, Debug)]
pub struct ProfileGetResponse {
    #[serde(rename = "userInfos")]
    pub user_infos: UserInfos,
}

#[derive(Deserialize, Debug)]
pub struct PixelUpdate {
    #[allow(dead_code)]
    pub c: i32, // could be useful for placement confirmation display
    #[allow(dead_code)]
    pub u: String, // could be useful for showing who placed the pixel
    #[allow(dead_code)]
    pub t: i64, // could be useful for placement timestamp display
    #[allow(dead_code)]
    pub x: i32, // could be useful for placement confirmation
    #[allow(dead_code)]
    pub y: i32, // could be useful for placement confirmation
    #[allow(dead_code)]
    pub p: Option<String>, // campus - could be useful for campus-based features
    #[allow(dead_code)]
    pub f: Option<String>, // country flag - could be useful for location-based features
}

#[derive(Deserialize, Debug)]
pub struct PixelSetResponse {
    #[allow(dead_code)]
    pub update: PixelUpdate,
    #[allow(dead_code)]
    pub timers: Vec<i64>,
    #[serde(rename = "userInfos")]
    pub user_infos: UserInfos,
}

// For error responses like 425 (Too Early) or 420 (Enhance Your Hype)
#[derive(Deserialize, Debug)]
pub struct ApiErrorResponse {
    pub message: String,
    pub timers: Option<Vec<i64>>,
    pub interval: Option<i64>,
}

#[derive(Debug)]
pub enum ApiError {
    #[allow(dead_code)]
    Network(reqwest::Error), // Used for Debug printing and error propagation
    ErrorResponse {
        status: reqwest::StatusCode,
        error_response: ApiErrorResponse,
    },
    #[allow(dead_code)]
    UnexpectedResponse(String), // Used for Debug printing with error details
    Unauthorized, // For 401/403 where we don't get an ApiErrorResponse
    #[allow(dead_code)]
    FileLogError(String), // Used for Debug printing and file operation errors
    TokenRefreshedPleaseRetry, // New variant for 426
}

impl From<reqwest::Error> for ApiError {
    fn from(err: reqwest::Error) -> Self {
        ApiError::Network(err)
    }
}

pub struct ApiClient {
    client: reqwest::Client,
    base_url: String,
    access_token: Option<String>,
    refresh_token: Option<String>,
    token_refresh_callback: Option<TokenRefreshCallback>,
}

impl std::fmt::Debug for ApiClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiClient")
            .field("base_url", &self.base_url)
            .field(
                "access_token",
                &self.access_token.as_ref().map(|_| "[REDACTED]"),
            )
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "[REDACTED]"),
            )
            .field(
                "token_refresh_callback",
                &self.token_refresh_callback.is_some(),
            )
            .finish()
    }
}

impl ApiClient {
    pub fn new(
        base_url: Option<String>,
        access_token: Option<String>,
        refresh_token: Option<String>,
    ) -> Self {
        ApiClient {
            client: reqwest::Client::builder()
                .cookie_store(true)
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            base_url: base_url.unwrap_or_else(|| API_BASE_URL.to_string()),
            access_token,
            refresh_token,
            token_refresh_callback: None,
        }
    }

    pub fn set_base_url(&mut self, base_url: String) {
        self.base_url = base_url;
    }

    pub fn set_tokens(&mut self, access: Option<String>, refresh: Option<String>) {
        self.access_token = access.clone();
        self.refresh_token = refresh.clone();

        // Call the callback if it's set
        if let Some(ref callback) = self.token_refresh_callback {
            callback(access, refresh);
        }
    }

    pub fn set_token_refresh_callback(&mut self, callback: TokenRefreshCallback) {
        self.token_refresh_callback = Some(callback);
    }

    // Getter for access token, primarily for use when setting the other token
    pub fn get_access_token_clone(&self) -> Option<String> {
        self.access_token.clone()
    }

    // Getter for refresh token, primarily for use when setting the other token
    pub fn get_refresh_token_clone(&self) -> Option<String> {
        self.refresh_token.clone()
    }

    /// Get both tokens for propagation (useful for updating main instance after background refresh)
    pub fn get_tokens(&self) -> (Option<String>, Option<String>) {
        (self.access_token.clone(), self.refresh_token.clone())
    }

    #[allow(dead_code)]
    pub fn get_base_url_preview(&self) -> String {
        // Return a portion of the base_url or the full thing if short
        let len = self.base_url.len();
        let preview_len = 20; // Show a bit more for URLs
        if len > preview_len {
            format!(
                "{}...",
                self.base_url.chars().take(preview_len).collect::<String>()
            )
        } else {
            self.base_url.clone()
        }
    }

    pub fn get_base_url_config_display(&self) -> String {
        // Better display for config that doesn't cut off ports poorly
        if self.base_url.len() <= 35 {
            // Show full URL if it's reasonably short
            self.base_url.clone()
        } else if self.base_url.starts_with("https://") {
            // For long HTTPS URLs, show protocol + domain + "..."
            let without_protocol = &self.base_url[8..]; // Remove "https://"
            if let Some(slash_pos) = without_protocol.find('/') {
                format!("https://{}...", &without_protocol[..slash_pos])
            } else if without_protocol.len() > 25 {
                format!("https://{}...", &without_protocol[..22])
            } else {
                self.base_url.clone()
            }
        } else if self.base_url.starts_with("http://") {
            // For long HTTP URLs, show protocol + domain + "..."
            let without_protocol = &self.base_url[7..]; // Remove "http://"
            if let Some(slash_pos) = without_protocol.find('/') {
                format!("http://{}...", &without_protocol[..slash_pos])
            } else if without_protocol.len() > 26 {
                format!("http://{}...", &without_protocol[..23])
            } else {
                self.base_url.clone()
            }
        } else {
            // For other protocols, just truncate normally
            if self.base_url.len() > 35 {
                format!("{}...", &self.base_url[..32])
            } else {
                self.base_url.clone()
            }
        }
    }

    pub fn get_base_url(&self) -> String {
        self.base_url.clone()
    }

    pub fn get_auth_cookie_preview(&self) -> Option<String> {
        self.access_token.as_ref().map(|s| {
            let len = s.len();
            let preview_len = 10;
            if len > preview_len {
                s.chars().take(preview_len).collect()
            } else {
                s.clone()
            }
        })
    }

    pub fn clear_tokens(&mut self) {
        self.access_token = None;
        self.refresh_token = None;
    }

    async fn handle_response<T: DeserializeOwned>(
        &mut self,
        response: reqwest::Response,
    ) -> Result<T, ApiError> {
        let status = response.status();
        let headers = response.headers().clone(); // Clone headers for later use if needed

        let response_text = match response.text().await {
            Ok(text) => text,
            Err(text_err) => {
                return Err(ApiError::UnexpectedResponse(format!(
                    "Failed to read response text (status {}): {}",
                    status, text_err
                )));
            }
        };

        if status.is_success() {
            // Try to parse the collected text as T (expected success response body)
            match serde_json::from_str::<T>(&response_text) {
                Ok(data) => Ok(data),
                Err(json_err) => {
                    // Log the problematic JSON to a file for inspection
                    let log_file_path = "./profile_response_error.json";
                    match File::create(log_file_path)
                        .and_then(|mut file| file.write_all(response_text.as_bytes()))
                    {
                        Ok(_) => { /* Successfully wrote to file */ }
                        Err(e) => {
                            return Err(ApiError::FileLogError(format!(
                                "Failed to write response to {}: {}",
                                log_file_path, e
                            )))
                        }
                    }

                    Err(ApiError::UnexpectedResponse(format!(
                        "Failed to parse successful response (status {}). Expected {}. Error: {}. Response body logged to '{}'",
                        status,
                        std::any::type_name::<T>(),
                        json_err,
                        log_file_path // Refer to the log file in the error message
                    )))
                }
            }
        } else if status.as_u16() == 426 {
            // Handle 426 for token refresh - improved cookie parsing
            let mut new_access_token: Option<String> = None;
            let mut new_refresh_token: Option<String> = None;

            for cookie_header in headers.get_all(reqwest::header::SET_COOKIE) {
                if let Ok(cookie_str) = cookie_header.to_str() {
                    // Parse each cookie properly, handling attributes
                    for cookie_part in cookie_str.split(',') {
                        let cookie_part = cookie_part.trim();

                        // Split on ';' to separate cookie value from attributes
                        if let Some(cookie_value) = cookie_part.split(';').next() {
                            let cookie_value = cookie_value.trim();

                            if cookie_value.starts_with("token=") {
                                let token_val =
                                    cookie_value.trim_start_matches("token=").trim_matches('"');
                                if !token_val.is_empty() && token_val != "deleted" {
                                    new_access_token = Some(token_val.to_string());
                                }
                            } else if cookie_value.starts_with("refresh=") {
                                let refresh_val = cookie_value
                                    .trim_start_matches("refresh=")
                                    .trim_matches('"');
                                if !refresh_val.is_empty() && refresh_val != "deleted" {
                                    new_refresh_token = Some(refresh_val.to_string());
                                }
                            }
                        }
                    }
                }
            }

            // Update tokens if we found new ones
            if new_access_token.is_some() || new_refresh_token.is_some() {
                // Update our tokens with the new values, keeping existing ones if not refreshed
                if let Some(new_token) = new_access_token {
                    self.access_token = Some(new_token);
                }
                if let Some(new_refresh) = new_refresh_token {
                    self.refresh_token = Some(new_refresh);
                }

                // Call the callback to persist the new tokens
                if let Some(ref callback) = self.token_refresh_callback {
                    callback(self.access_token.clone(), self.refresh_token.clone());
                }

                return Err(ApiError::TokenRefreshedPleaseRetry);
            } else {
                // If 426 but no new tokens found, treat as error
                return Err(ApiError::UnexpectedResponse(format!(
                    "Received 426 (Token Refresh) but no valid tokens found in Set-Cookie headers. Response: {}",
                    response_text
                )));
            }
        } else {
            // Try to parse the collected text as our specific ApiErrorResponse struct for known API errors
            match serde_json::from_str::<ApiErrorResponse>(&response_text) {
                Ok(error_body) => Err(ApiError::ErrorResponse {
                    status,
                    error_response: error_body,
                }),
                Err(_parse_err) => {
                    // If that fails, it's an unexpected error format or an auth error not matching ApiErrorResponse
                    if status == reqwest::StatusCode::UNAUTHORIZED
                        || status == reqwest::StatusCode::FORBIDDEN
                    {
                        Err(ApiError::Unauthorized)
                    } else {
                        Err(ApiError::UnexpectedResponse(format!(
                            "Request failed with status: {}. Response body: \"{}\". Could not parse as ApiErrorResponse.",
                            status, response_text
                        )))
                    }
                }
            }
        }
    }

    // Helper to build and send request, to be used by retry logic
    async fn send_request_with_retry<F, Fut, T>(
        &mut self,
        build_request_fn: F,
    ) -> Result<T, ApiError>
    where
        F: Fn(&mut Self) -> Fut,
        Fut: std::future::Future<Output = Result<reqwest::Response, reqwest::Error>>,
        T: DeserializeOwned,
    {
        let response = build_request_fn(self).await?;
        match self.handle_response(response).await {
            Ok(data) => Ok(data),
            Err(ApiError::TokenRefreshedPleaseRetry) => {
                // Token was refreshed, try the request again
                let response_retry = build_request_fn(self).await?;
                self.handle_response(response_retry).await
            }
            Err(e) => Err(e), // Other errors
        }
    }

    pub async fn get_board(&mut self) -> Result<BoardGetResponse, ApiError> {
        self.send_request_with_retry(|s| {
            let url = format!("{}/api/get", s.base_url);
            let mut request_builder = s.client.get(&url);
            let mut cookie_parts = Vec::new();
            if let Some(token) = &s.access_token {
                cookie_parts.push(format!("token={}", token));
            }
            if let Some(refresh) = &s.refresh_token {
                cookie_parts.push(format!("refresh={}", refresh));
            }
            if !cookie_parts.is_empty() {
                request_builder = request_builder.header(COOKIE, cookie_parts.join("; "));
            }
            async move { request_builder.send().await }
        })
        .await
    }

    pub async fn get_profile(&mut self) -> Result<ProfileGetResponse, ApiError> {
        self.send_request_with_retry(|s| {
            let url = format!("{}/api/profile", s.base_url);
            let mut request_builder = s.client.get(&url);
            let mut cookie_parts = Vec::new();
            if let Some(token) = &s.access_token {
                cookie_parts.push(format!("token={}", token));
            }
            if let Some(refresh) = &s.refresh_token {
                cookie_parts.push(format!("refresh={}", refresh));
            }
            if !cookie_parts.is_empty() {
                request_builder = request_builder.header(COOKIE, cookie_parts.join("; "));
            }
            async move { request_builder.send().await }
        })
        .await
    }

    pub async fn place_pixel(
        &mut self,
        x: i32,
        y: i32,
        color_id: i32,
    ) -> Result<PixelSetResponse, ApiError> {
        self.send_request_with_retry(|s| {
            let url = format!("{}/api/set", s.base_url);
            let mut request_builder = s.client.post(&url);
            let mut cookie_parts = Vec::new();
            if let Some(token) = &s.access_token {
                cookie_parts.push(format!("token={}", token));
            }
            if let Some(refresh) = &s.refresh_token {
                cookie_parts.push(format!("refresh={}", refresh));
            }
            if !cookie_parts.is_empty() {
                request_builder = request_builder.header(COOKIE, cookie_parts.join("; "));
            }
            let body = serde_json::json!({
                "x": x,
                "y": y,
                "color": color_id
            });
            request_builder = request_builder
                .header(CONTENT_TYPE, "application/json")
                .json(&body);
            async move { request_builder.send().await }
        })
        .await
    }
}

/// Utility function to create a token refresh callback that saves tokens to storage
pub fn create_token_refresh_callback(
    base_url: Option<String>,
) -> Result<TokenRefreshCallback, Box<dyn std::error::Error>> {
    let storage = crate::token_storage::TokenStorage::new()?;
    let storage = std::sync::Arc::new(std::sync::Mutex::new(storage));

    Ok(Box::new(
        move |access_token: Option<String>, refresh_token: Option<String>| {
            if let Ok(storage) = storage.lock() {
                let token_data = crate::token_storage::TokenData {
                    access_token,
                    refresh_token,
                    base_url: base_url.clone(),
                };
                let _ = storage.save(&token_data);
            }
        },
    ) as TokenRefreshCallback)
}

// Need to add this module to main.rs or lib.rs
