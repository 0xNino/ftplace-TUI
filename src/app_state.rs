use crate::api_client::{ApiClient, ColorInfo, PixelNetwork, UserInfos};
use crate::art::PixelArt;
use std::time::Instant;

#[derive(Debug, PartialEq, Eq, Default)]
pub enum InputMode {
    #[default]
    None,
    Cookie,
    ArtEditor,         // New mode for creating/editing pixel art
    ArtEditorFileName, // New mode for entering filename for saving art
}

#[derive(Debug)]
pub struct App {
    pub exit: bool,
    pub api_client: ApiClient,
    pub input_mode: InputMode,
    pub cookie_input_buffer: String,
    pub status_message: String, // To display messages to the user
    pub board: Vec<Vec<Option<PixelNetwork>>>,
    pub colors: Vec<ColorInfo>,
    pub user_info: Option<UserInfos>,
    pub loaded_art: Option<PixelArt>,
    pub board_viewport_x: u16,       // X offset of the viewport in pixels
    pub board_viewport_y: u16,       // Y offset of the viewport in pixel rows (top row of the pair)
    pub initial_board_fetched: bool, // New flag
    pub last_board_refresh: Option<Instant>, // For auto-refresh

    // Pixel Art Editor State
    pub current_editing_art: Option<PixelArt>, // Holds the art being created/edited
    pub art_editor_cursor_x: i32,              // Cursor X position on the art canvas
    pub art_editor_cursor_y: i32,              // Cursor Y position on the art canvas
    pub art_editor_selected_color_id: i32,     // Currently selected color_id for drawing
    pub art_editor_filename_buffer: String,    // Buffer for filename input
    pub art_editor_canvas_width: u16,          // Width of the art editor canvas
    pub art_editor_canvas_height: u16,         // Height of the art editor canvas
    pub art_editor_viewport_x: i32,            // X offset of the art editor viewport
    pub art_editor_viewport_y: i32,            // Y offset of the art editor viewport
}
