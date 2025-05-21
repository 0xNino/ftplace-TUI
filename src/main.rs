use std::io::{self, stdout};

use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use keyring::Entry;
use ratatui::prelude::*;

mod api_client;
mod app_state;
mod art;
mod event_handler;
mod ui;
use api_client::ApiClient;
use app_state::{App, InputMode};

const KEYRING_SERVICE_NAME: &str = "ftplace_tui_service";
const KEYRING_USER_NAME: &str = "session_cookie";

impl App {
    pub fn new() -> Self {
        let mut api_client = ApiClient::new(None, None);
        let mut initial_status = "Loading TUI... Press 'q' to quit.".to_string(); // Initial message

        // Try to load cookie from keyring
        match Entry::new(KEYRING_SERVICE_NAME, KEYRING_USER_NAME) {
            Ok(entry) => match entry.get_password() {
                Ok(cookie) => {
                    let trimmed_cookie = cookie.trim();
                    if !trimmed_cookie.is_empty() {
                        api_client.set_cookie(trimmed_cookie.to_string());
                        initial_status = format!(
                            "Loaded cookie from keyring ({}... chars). Fetching board...",
                            trimmed_cookie.chars().take(10).collect::<String>().len()
                        );
                    } else {
                        // This case should ideally not happen if keyring stores empty string correctly,
                        // but good to handle.
                        initial_status =
                            "Found empty cookie in keyring. Fetching board... Press 'c' to set cookie."
                                .to_string();
                    }
                }
                Err(keyring::Error::NoEntry) => {
                    initial_status =
                        "No cookie found in keyring. Fetching board... Press 'c' to set cookie."
                            .to_string();
                }
                Err(e) => {
                    initial_status = format!(
                        "Error loading cookie from keyring: {}. Fetching board... Press 'c' to set.",
                        e
                    );
                }
            },
            Err(e) => {
                initial_status = format!(
                    "Failed to access keyring service '{}': {}. Fetching board... Press 'c' to set cookie.",
                    KEYRING_SERVICE_NAME, e
                );
            }
        }

        Self {
            exit: false,
            api_client,
            input_mode: InputMode::None,
            cookie_input_buffer: String::new(),
            status_message: initial_status,
            board: Vec::new(), // Board is empty initially
            colors: Vec::new(),
            user_info: None,
            loaded_art: None,
            board_viewport_x: 0,
            board_viewport_y: 0,
            initial_board_fetched: false, // Initialize new flag
            // Initialize Pixel Art Editor State
            current_editing_art: None,
            art_editor_cursor_x: 0,
            art_editor_cursor_y: 0,
            art_editor_selected_color_id: 1, // Default to color_id 1 (often a common color)
            art_editor_filename_buffer: String::new(),
            art_editor_canvas_width: 30, // Default canvas size, can be configurable later
            art_editor_canvas_height: 20, // Default canvas size
            art_editor_viewport_x: 0,
            art_editor_viewport_y: 0,
        }
    }

    pub async fn run(&mut self, terminal: &mut Terminal<impl Backend>) -> io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| ui::render_ui(self, frame))?;
            self.handle_events().await?;
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    execute!(stdout(), EnterAlternateScreen)?;

    let mut app = App::new();
    let res = app.run(&mut terminal).await;

    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;

    res
}
