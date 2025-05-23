use crate::api_client::{ApiClient, BoardGetResponse, ColorInfo, PixelNetwork, UserInfos};
use crate::art::PixelArt;
use crate::token_storage::TokenStorage;
use std::collections::VecDeque;
use std::time::Instant;
use tokio::sync::mpsc;

#[derive(Debug, PartialEq, Eq, Default)]
pub enum InputMode {
    #[default]
    None,
    EnterBaseUrl,           // Will now involve selection or custom input
    EnterCustomBaseUrlText, // New sub-mode for when "Custom" URL is chosen
    EnterAccessToken,       // Renamed from Cookie
    EnterRefreshToken,      // New
    ArtEditor,              // New mode for creating/editing pixel art
    ArtEditorNewArtName,    // New mode for entering name when creating new art
    ArtSelection,           // New mode for selecting pixel art to load/place
    ArtQueue,               // New mode for managing art placement queue
    ShowHelp,               // New mode for displaying available commands
    ShowProfile,            // New mode for displaying user profile
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueueStatus {
    Pending,
    InProgress,
    Complete,
    Skipped, // If no meaningful pixels to place
    Failed,  // If placement failed
}

#[derive(Debug, Clone)]
pub struct ArtQueueItem {
    pub art: PixelArt,
    pub priority: u8, // 1=high, 5=low
    pub status: QueueStatus,
    pub pixels_placed: usize, // Track progress
    pub pixels_total: usize,  // Total meaningful pixels
    pub added_time: Instant,  // When added to queue
}

#[derive(Debug)]
pub struct App {
    pub exit: bool,
    pub api_client: ApiClient,
    pub token_storage: TokenStorage,
    pub input_mode: InputMode,
    pub input_buffer: String, // Generic input buffer (renamed from cookie_input_buffer for clarity)
    pub status_message: String, // To display messages to the user
    pub status_messages: VecDeque<(String, Instant)>, // History of recent status messages
    pub cooldown_status: String, // Persistent cooldown/timer info
    pub board: Vec<Vec<Option<PixelNetwork>>>,
    pub colors: Vec<ColorInfo>,
    pub user_info: Option<UserInfos>,
    pub loaded_art: Option<PixelArt>,
    pub board_viewport_x: u16,       // X offset of the viewport in pixels
    pub board_viewport_y: u16,       // Y offset of the viewport in pixel rows (top row of the pair)
    pub initial_board_fetched: bool, // New flag
    pub last_board_refresh: Option<Instant>, // For auto-refresh
    pub should_fetch_board_on_start: bool, // Flag to trigger board fetch when tokens are restored
    pub board_loading: bool,         // Flag to indicate board is being fetched in background
    pub board_load_start: Option<Instant>, // When background load started
    pub board_fetch_receiver: Option<mpsc::UnboundedReceiver<BoardFetchResult>>, // Channel for receiving board fetch results
    pub placement_receiver: Option<mpsc::UnboundedReceiver<PlacementUpdate>>, // Channel for receiving placement updates
    pub placement_in_progress: bool, // Flag to indicate art placement is in progress
    pub placement_start: Option<Instant>, // When placement started
    pub placement_cancel_requested: bool, // Flag to request cancellation
    pub queue_receiver: Option<mpsc::UnboundedReceiver<QueueUpdate>>, // Channel for receiving queue processing updates
    pub queue_processing_start: Option<Instant>, // When queue processing started
    pub profile_receiver: Option<mpsc::UnboundedReceiver<ProfileFetchResult>>, // Channel for receiving profile fetch results

    // State for Base URL selection
    pub base_url_options: Vec<String>,
    pub base_url_selection_index: usize,

    // Pixel Art Editor State
    pub current_editing_art: Option<PixelArt>, // Holds the art being created/edited
    pub art_editor_cursor_x: i32,              // Cursor X position on the art canvas
    pub art_editor_cursor_y: i32,              // Cursor Y position on the art canvas
    pub art_editor_selected_color_id: i32,     // Currently selected color_id for drawing
    pub art_editor_color_palette_index: usize, // Index in the colors array for palette navigation
    pub art_editor_canvas_width: u16,          // Width of the art editor canvas
    pub art_editor_canvas_height: u16,         // Height of the art editor canvas
    #[allow(dead_code)]
    pub art_editor_viewport_x: i32, // X offset of the art editor viewport - for future scrolling
    #[allow(dead_code)]
    pub art_editor_viewport_y: i32, // Y offset of the art editor viewport - for future scrolling

    // Pixel Art Selection State
    pub available_pixel_arts: Vec<PixelArt>, // List of available pixel arts (saved + default)
    pub art_selection_index: usize,          // Current selection in art list

    // Art Queue System
    pub art_queue: Vec<ArtQueueItem>, // Queue of arts to be placed
    pub queue_selection_index: usize, // Current selection in queue list
    pub queue_processing: bool,       // Whether queue is currently being processed
    pub queue_blink_state: bool,      // For blinking preview effect
    pub last_blink_time: Option<Instant>, // Last time blink state changed
}

#[derive(Debug)]
pub enum BoardFetchResult {
    Success(BoardGetResponse),
    Error(String),
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // Some variants are for future features
pub enum PlacementUpdate {
    Progress {
        art_name: String,
        pixel_index: usize,
        total_pixels: usize,
        position: (i32, i32),
        cooldown_remaining: Option<u32>,
    },
    Complete {
        art_name: String,
        pixels_placed: usize,
        total_pixels: usize,
    },
    Error {
        art_name: String,
        error_msg: String,
        pixel_index: usize,
        total_pixels: usize,
    },
    Cancelled {
        art_name: String,
        pixels_placed: usize,
        total_pixels: usize,
    },
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // Some variants are for future features
pub enum QueueUpdate {
    ItemStarted {
        item_index: usize,
        art_name: String,
        total_items: usize,
    },
    ItemProgress {
        item_index: usize,
        art_name: String,
        pixels_placed: usize,
        total_pixels: usize,
        position: (i32, i32),
        cooldown_remaining: Option<u32>,
    },
    ItemCompleted {
        item_index: usize,
        art_name: String,
        pixels_placed: usize,
        total_pixels: usize,
    },
    ItemFailed {
        item_index: usize,
        art_name: String,
        error_msg: String,
    },
    ItemSkipped {
        item_index: usize,
        art_name: String,
        reason: String,
    },
    QueueCompleted {
        total_items_processed: usize,
        total_pixels_placed: usize,
        duration_secs: u64,
    },
    QueueCancelled {
        items_processed: usize,
        total_pixels_placed: usize,
    },
}

#[derive(Debug)]
pub enum ProfileFetchResult {
    Success(crate::api_client::UserInfos),
    Error(String),
}
