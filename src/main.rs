use std::io::{self, stdout};

use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;

mod api_client;
mod app_state;
mod art;
mod event_handler;
mod ui;
use api_client::ApiClient;
use app_state::{App, InputMode};

impl App {
    pub fn new() -> Self {
        let api_client = ApiClient::new(None, None, None);

        let base_url_options = vec![
            "https://ftplace.42lausanne.ch".to_string(),
            "http://localhost:7979".to_string(),
            "Custom".to_string(),
        ];

        Self {
            exit: false,
            api_client,
            input_mode: InputMode::EnterBaseUrl,
            input_buffer: String::new(),
            status_message: "Select API Base URL or choose Custom:".to_string(),
            board: Vec::new(),
            colors: Vec::new(),
            user_info: None,
            loaded_art: None,
            board_viewport_x: 0,
            board_viewport_y: 0,
            initial_board_fetched: false,
            last_board_refresh: None,
            base_url_options,
            base_url_selection_index: 0,
            current_editing_art: None,
            art_editor_cursor_x: 0,
            art_editor_cursor_y: 0,
            art_editor_selected_color_id: 1,
            art_editor_filename_buffer: String::new(),
            art_editor_canvas_width: 30,
            art_editor_canvas_height: 20,
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
