use std::collections::VecDeque;
use std::io::{self, stdout};

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;

mod api_client;
mod app_state;
mod art;
mod event_handling;
mod token_storage;
mod ui;
use api_client::ApiClient;
use app_state::{App, InputMode};
use token_storage::TokenStorage;

impl App {
    pub fn new() -> Self {
        // Initialize token storage
        let token_storage = match TokenStorage::new() {
            Ok(storage) => storage,
            Err(e) => {
                eprintln!("Warning: Could not initialize token storage: {}", e);
                // Create a temporary storage that will work but not persist
                TokenStorage::new().unwrap_or_else(|_| panic!("Failed to create token storage"))
            }
        };

        // Load saved tokens
        let saved_tokens = token_storage.load();

        // Initialize API client with saved tokens and base URL
        let api_client = ApiClient::new(
            saved_tokens.base_url.clone(),
            saved_tokens.access_token.clone(),
            saved_tokens.refresh_token.clone(),
        );

        let base_url_options = vec![
            "https://ftplace.42lausanne.ch".to_string(),
            "http://localhost:7979".to_string(),
            "Custom".to_string(),
        ];

        // Determine initial input mode based on saved data
        let (initial_mode, initial_message, should_fetch_on_start) =
            if saved_tokens.base_url.is_some()
                && (saved_tokens.access_token.is_some() || saved_tokens.refresh_token.is_some())
            {
                // Have saved config, go directly to help/main view and fetch board
                (
                    InputMode::ShowHelp,
                    format!(
                        "Restored session: {}. Press any key to continue or 'c' to reconfigure.",
                        saved_tokens.base_url.as_deref().unwrap_or("Unknown URL")
                    ),
                    true, // Trigger board fetch
                )
            } else {
                // No saved config, start with URL selection
                (
                    InputMode::EnterBaseUrl,
                    "Select API Base URL or choose Custom:".to_string(),
                    false, // Don't fetch board yet
                )
            };

        let mut app = Self {
            exit: false,
            api_client,
            token_storage,
            input_mode: initial_mode,
            input_buffer: String::new(),
            status_message: initial_message.clone(),
            status_messages: VecDeque::new(),
            cooldown_status: String::new(),
            board: Vec::new(),
            colors: Vec::new(),
            user_info: None,
            loaded_art: None,
            board_viewport_x: 0,
            board_viewport_y: 0,
            initial_board_fetched: false,
            last_board_refresh: None,
            should_fetch_board_on_start: should_fetch_on_start,
            board_loading: false,
            board_load_start: None,
            board_fetch_receiver: None,
            placement_receiver: None,
            placement_in_progress: false,
            placement_start: None,
            placement_cancel_requested: false,
            queue_receiver: None,
            queue_control_sender: None,
            queue_processing_start: None,
            profile_receiver: None,
            base_url_options,
            base_url_selection_index: 0,
            current_editing_art: None,
            art_editor_cursor_x: 0,
            art_editor_cursor_y: 0,
            art_editor_selected_color_id: 1,
            art_editor_color_palette_index: 0,
            art_editor_canvas_width: 30,
            art_editor_canvas_height: 20,
            art_editor_viewport_x: 0,
            art_editor_viewport_y: 0,
            available_pixel_arts: Vec::new(),
            art_selection_index: 0,
            art_preview_art: None,
            art_queue: Vec::new(),
            queue_selection_index: 0,
            queue_processing: false,
            queue_paused: false,
            queue_blink_state: false,
            last_blink_time: None,
            shared_board_state: None,
            board_area_bounds: None,
            available_shares: Vec::new(),
            share_selection_index: 0,
            current_share_art: None,
            current_share_coords: None,
        };

        // Load saved queue
        let _ = app.load_queue();

        // Load saved status messages
        let _ = app.load_status_messages();

        // Add initial status message if we have saved config
        if should_fetch_on_start {
            app.add_status_message(initial_message);
        }

        app
    }

    pub async fn run(&mut self, terminal: &mut Terminal<impl Backend>) -> io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| ui::render_ui(self, frame))?;
            self.handle_events().await?;
        }

        // Save status messages before exiting
        let _ = self.save_status_messages();

        Ok(())
    }
}

#[tokio::main]
async fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    execute!(stdout(), EnterAlternateScreen, EnableMouseCapture)?;

    let mut app = App::new();
    let res = app.run(&mut terminal).await;

    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen, DisableMouseCapture)?;

    res
}
