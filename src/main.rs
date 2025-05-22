use std::io::{self, stdout};

use clap::Parser;
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

/// FtPlace TUI Client
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// API base URL (e.g., http://localhost:7979)
    #[arg(short, long, env = "FTPLACE_BASE_URL")]
    base_url: Option<String>,

    /// Session cookie value (token)
    #[arg(short, long, env = "FTPLACE_COOKIE")]
    cookie: Option<String>,
}

impl App {
    pub fn new(base_url: Option<String>, cookie: Option<String>) -> Self {
        let api_client = ApiClient::new(base_url, cookie.clone());
        let initial_status: String;

        if cookie.is_some() {
            initial_status = "Using provided cookie. Fetching board...".to_string();
        } else {
            initial_status =
                "No cookie provided. Limited functionality. Fetching board...".to_string();
        }

        Self {
            exit: false,
            api_client,
            input_mode: InputMode::None,
            cookie_input_buffer: String::new(),
            status_message: initial_status,
            board: Vec::new(),
            colors: Vec::new(),
            user_info: None,
            loaded_art: None,
            board_viewport_x: 0,
            board_viewport_y: 0,
            initial_board_fetched: false,
            current_editing_art: None,
            art_editor_cursor_x: 0,
            art_editor_cursor_y: 0,
            art_editor_selected_color_id: 1,
            art_editor_filename_buffer: String::new(),
            art_editor_canvas_width: 30,
            art_editor_canvas_height: 20,
            art_editor_viewport_x: 0,
            art_editor_viewport_y: 0,
            last_board_refresh: None,
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
    let cli = Cli::parse();

    enable_raw_mode()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    execute!(stdout(), EnterAlternateScreen)?;

    let mut app = App::new(cli.base_url, cli.cookie);
    let res = app.run(&mut terminal).await;

    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;

    res
}
