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
    Unauthorized,         // For 401/403 where we don't get an ApiErrorResponse
    FileLogError(String), // For errors during logging to file
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
    auth_cookie: Option<String>,
}

impl ApiClient {
    pub fn new(base_url: Option<String>, auth_cookie: Option<String>) -> Self {
        ApiClient {
            client: reqwest::Client::new(),
            base_url: base_url.unwrap_or_else(|| API_BASE_URL.to_string()),
            auth_cookie,
        }
    }

    pub fn set_cookie(&mut self, cookie: String) {
        self.auth_cookie = Some(cookie);
    }

    pub fn get_auth_cookie_preview(&self) -> Option<String> {
        self.auth_cookie.as_ref().map(|s| {
            let len = s.len();
            let preview_len = 10;
            if len > preview_len {
                s.chars().take(preview_len).collect()
            } else {
                s.clone()
            }
        })
    }

    pub fn clear_cookie(&mut self) {
        self.auth_cookie = None;
    }

    async fn handle_response<T: DeserializeOwned>(
        response: reqwest::Response,
    ) -> Result<T, ApiError> {
        let status = response.status();

        // Always try to get the text first, as .json() consumes the body and we might need the text for errors.
        let response_text = match response.text().await {
            Ok(text) => text,
            Err(text_err) => {
                // If we can't even get text, it's a fundamental issue with the response.
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

    pub async fn get_board(&self) -> Result<BoardGetResponse, ApiError> {
        let url = format!("{}/api/get", self.base_url);
        let response = self.client.get(&url).send().await?;
        Self::handle_response(response).await
    }

    pub async fn get_profile(&self) -> Result<ProfileGetResponse, ApiError> {
        let url = format!("{}/api/profile", self.base_url);
        let mut request_builder = self
            .client
            .get(&url)
            .header(reqwest::header::ACCEPT, "application/json");

        if let Some(token_value) = &self.auth_cookie {
            let cookie_header_value = format!("token={}", token_value);
            request_builder = request_builder.header(COOKIE, cookie_header_value);
        }

        let response = request_builder.send().await?;
        Self::handle_response(response).await
    }

    pub async fn place_pixel(
        &self,
        x: i32,
        y: i32,
        color_id: i32,
    ) -> Result<PixelSetResponse, ApiError> {
        let url = format!("{}/api/set", self.base_url);
        let mut request_builder = self.client.post(&url);

        if let Some(token_value) = &self.auth_cookie {
            let cookie_header_value = format!("token={}", token_value);
            request_builder = request_builder.header(COOKIE, cookie_header_value);
        }

        let body = serde_json::json!({
            "x": x,
            "y": y,
            "color": color_id // Backend expects "color" for color_id
        });

        request_builder = request_builder
            .header(CONTENT_TYPE, "application/json")
            .json(&body);
        let response = request_builder.send().await?;
        Self::handle_response(response).await
    }
}

// Need to add this module to main.rs or lib.rs
