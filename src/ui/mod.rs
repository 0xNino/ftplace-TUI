pub mod art_editor;
pub mod art_management;
pub mod helpers;
pub mod popups;
pub mod render;

// Re-export the main render function
pub use render::render_ui;
