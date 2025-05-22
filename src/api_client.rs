use reqwest::header::{CONTENT_TYPE, COOKIE};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::fs::File; // For file logging
use std::io::Write; // For file logging

// API Endpoint Base URL - can be configured later
const API_BASE_URL: &str = "https://ftplace.42lausanne.ch"; // TODO: Make this configurable

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
    pub c: i32,    // color_id
    pub u: String, // username
    pub t: i64,    // set_time (timestamp)
}

#[derive(Deserialize, Debug)]
pub struct BoardGetResponse {
    pub colors: Vec<ColorInfo>,
    pub board: Vec<Vec<Option<PixelNetwork>>>,
    // Optional fields for admin view (min_time, max_time, type)
    pub r#type: Option<String>, // "board" or "image"
    pub min_time: Option<i64>,
    pub max_time: Option<i64>,
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
    pub c: i32,
    pub u: String,
    pub t: i64,
    pub x: i32,
    pub y: i32,
    pub p: Option<String>, // campus, present in the error log
    pub f: Option<String>, // country flag, present in the error log
}

#[derive(Deserialize, Debug)]
pub struct PixelSetResponse {
    pub update: PixelUpdate,
    pub timers: Vec<i64>, // This top-level one is fine and present
    #[serde(rename = "userInfos")]
    pub user_infos: UserInfos, // This UserInfos is now more flexible
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
    Network(reqwest::Error),
    ApiErrorResponse {
        status: reqwest::StatusCode,
        error_response: ApiErrorResponse,
    },
    UnexpectedResponse(String),
    Unauthorized,              // For 401/403 where we don't get an ApiErrorResponse
    FileLogError(String),      // For errors during logging to file
    TokenRefreshedPleaseRetry, // New variant for 426
}

impl From<reqwest::Error> for ApiError {
    fn from(err: reqwest::Error) -> Self {
        ApiError::Network(err)
    }
}

#[derive(Debug)]
pub struct ApiClient {
    client: reqwest::Client,
    base_url: String,
    access_token: Option<String>,
    refresh_token: Option<String>,
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
        }
    }

    pub fn set_base_url(&mut self, base_url: String) {
        self.base_url = base_url;
    }

    pub fn set_tokens(&mut self, access: Option<String>, refresh: Option<String>) {
        self.access_token = access;
        self.refresh_token = refresh;
    }

    // Getter for access token, primarily for use when setting the other token
    pub fn get_access_token_clone(&self) -> Option<String> {
        self.access_token.clone()
    }

    // Getter for refresh token, primarily for use when setting the other token
    pub fn get_refresh_token_clone(&self) -> Option<String> {
        self.refresh_token.clone()
    }

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
            // Handle 426 for token refresh
            let mut new_access_token_found = false;
            let mut new_refresh_token_found = false;
            for cookie_str_val in headers.get_all(reqwest::header::SET_COOKIE) {
                if let Ok(cookie_str) = cookie_str_val.to_str() {
                    if let Some(token_part) = cookie_str.split(';').next() {
                        if token_part.trim().starts_with("token=") {
                            let new_token_val =
                                token_part.trim().trim_start_matches("token=").to_string();
                            if !new_token_val.is_empty() {
                                self.access_token = Some(new_token_val);
                                new_access_token_found = true;
                            }
                        } else if token_part.trim().starts_with("refresh=") {
                            let new_refresh_val =
                                token_part.trim().trim_start_matches("refresh=").to_string();
                            if !new_refresh_val.is_empty() {
                                self.refresh_token = Some(new_refresh_val);
                                new_refresh_token_found = true;
                            }
                        }
                    }
                    if new_access_token_found && new_refresh_token_found {
                        break;
                    }
                }
            }
            if new_access_token_found {
                return Err(ApiError::TokenRefreshedPleaseRetry);
            } else {
                // If 426 but no new token found in Set-Cookie, treat as unexpected or error
                return Err(ApiError::UnexpectedResponse(format!(
                    "Received 426 but no new token found in Set-Cookie. Response body: \"{}\"",
                    response_text
                )));
            }
        } else {
            // Try to parse the collected text as our specific ApiErrorResponse struct for known API errors
            match serde_json::from_str::<ApiErrorResponse>(&response_text) {
                Ok(error_body) => Err(ApiError::ApiErrorResponse {
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
            let mut request_builder = s
                .client
                .get(&url)
                .header(reqwest::header::ACCEPT, "application/json");
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

// Need to add this module to main.rs or lib.rs
