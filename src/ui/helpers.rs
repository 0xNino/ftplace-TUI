use crate::app_state::App;
use ratatui::prelude::*;

/// helper function to create a centered rect using up certain percentage of the available rect `r`
pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

pub fn get_ratatui_color(app: &App, color_id: i32, default_fallback_color: Color) -> Color {
    app.colors
        .iter()
        .find(|c| c.id == color_id)
        .map_or(default_fallback_color, |color_info| {
            Color::Rgb(color_info.red, color_info.green, color_info.blue)
        })
}

pub fn get_color_name(app: &App, color_id: i32) -> String {
    app.colors
        .iter()
        .find(|c| c.id == color_id)
        .map(|color_info| {
            if color_info.name.trim().is_empty() {
                format!("Color {}", color_id)
            } else {
                color_info.name.clone()
            }
        })
        .unwrap_or_else(|| format!("Unknown Color {}", color_id))
}

/// Check if a pixel at the given position already has the correct color (UI helper)
pub fn is_pixel_already_correct_ui(
    board: &[Vec<Option<crate::api_client::PixelNetwork>>],
    x: i32,
    y: i32,
    expected_color_id: i32,
) -> bool {
    // Convert to usize for array indexing
    let x_idx = x as usize;
    let y_idx = y as usize;

    // Check bounds
    if x_idx >= board.len() || y_idx >= board.get(x_idx).map_or(0, |col| col.len()) {
        return false;
    }

    // Check if the pixel exists and has the correct color - collapsed if-let pattern
    if let Some(Some(pixel)) = board.get(x_idx).and_then(|row| row.get(y_idx)) {
        pixel.c == expected_color_id
    } else {
        // No pixel exists, so it's not the correct color
        false
    }
}

/// Get the current color of a pixel on the board (UI helper)
pub fn get_current_board_color_ui(
    board: &[Vec<Option<crate::api_client::PixelNetwork>>],
    colors: &[crate::api_client::ColorInfo],
    x: i32,
    y: i32,
) -> Color {
    // Convert to usize for array indexing
    let x_idx = x as usize;
    let y_idx = y as usize;

    // Check bounds
    if x_idx >= board.len() || y_idx >= board.get(x_idx).map_or(0, |col| col.len()) {
        return Color::DarkGray; // Out of bounds
    }

    // Get the current pixel color - collapsed if-let pattern
    if let Some(Some(pixel)) = board.get(x_idx).and_then(|row| row.get(y_idx)) {
        // Find the color info for this pixel's color_id
        if let Some(color_info) = colors.iter().find(|c| c.id == pixel.c) {
            Color::Rgb(color_info.red, color_info.green, color_info.blue)
        } else {
            Color::Gray // Color ID not found in palette
        }
    } else {
        // No pixel exists - empty/default
        Color::Black
    }
}
