use crate::api_client::{ApiClient, ColorInfo, PixelNetwork, UserInfos};
use crate::art::PixelArt;
use crate::token_storage::TokenStorage;
use std::time::Instant;

#[derive(Debug, PartialEq, Eq, Default)]
pub enum InputMode {
    #[default]
    None,
    EnterBaseUrl,           // Will now involve selection or custom input
    EnterCustomBaseUrlText, // New sub-mode for when "Custom" URL is chosen
    EnterAccessToken,       // Renamed from Cookie
    EnterRefreshToken,      // New
    ArtEditor,              // New mode for creating/editing pixel art
    ArtEditorFileName,      // New mode for entering filename for saving art
    ShowHelp,               // New mode for displaying available commands
    ShowProfile,            // New mode for displaying user profile
}

#[derive(Debug)]
pub struct App {
    pub exit: bool,
    pub api_client: ApiClient,
    pub token_storage: TokenStorage,
    pub input_mode: InputMode,
    pub input_buffer: String, // Generic input buffer (renamed from cookie_input_buffer for clarity)
    pub status_message: String, // To display messages to the user
    pub board: Vec<Vec<Option<PixelNetwork>>>,
    pub colors: Vec<ColorInfo>,
    pub user_info: Option<UserInfos>,
    pub loaded_art: Option<PixelArt>,
    pub board_viewport_x: u16,       // X offset of the viewport in pixels
    pub board_viewport_y: u16,       // Y offset of the viewport in pixel rows (top row of the pair)
    pub initial_board_fetched: bool, // New flag
    pub last_board_refresh: Option<Instant>, // For auto-refresh
    pub should_fetch_board_on_start: bool, // Flag to trigger board fetch when tokens are restored

    // State for Base URL selection
    pub base_url_options: Vec<String>,
    pub base_url_selection_index: usize,

    // Pixel Art Editor State
    pub current_editing_art: Option<PixelArt>, // Holds the art being created/edited
    pub art_editor_cursor_x: i32,              // Cursor X position on the art canvas
    pub art_editor_cursor_y: i32,              // Cursor Y position on the art canvas
    pub art_editor_selected_color_id: i32,     // Currently selected color_id for drawing
    pub art_editor_color_palette_index: usize, // Index in the colors array for palette navigation
    pub art_editor_filename_buffer: String,    // Buffer for filename input
    pub art_editor_canvas_width: u16,          // Width of the art editor canvas
    pub art_editor_canvas_height: u16,         // Height of the art editor canvas
    pub art_editor_viewport_x: i32,            // X offset of the art editor viewport
    pub art_editor_viewport_y: i32,            // Y offset of the art editor viewport
}
