use crate::app_state::App;
use crate::ui::helpers::{get_color_name, get_ratatui_color};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

pub fn render_art_editor_ui(app: &mut App, frame: &mut Frame, area: Rect) {
    let editor_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(0),     // Art Canvas
            Constraint::Length(20), // Color Palette (fixed width for now)
        ])
        .split(area);

    let canvas_area = editor_layout[0];
    let palette_area = editor_layout[1];

    let selected_color_name = get_color_name(app, app.art_editor_selected_color_id);
    let editor_block = Block::default()
		.borders(Borders::ALL)
		.title(format!(
			"Pixel Art Editor (Canvas: {}x{}, Cursor: {},{}, Color: {}) - Arrows, Space, Tab, s:Save, Esc:Exit",
			app.art_editor_canvas_width,
			app.art_editor_canvas_height,
			app.art_editor_cursor_x,
			app.art_editor_cursor_y,
			selected_color_name
		));
    frame.render_widget(editor_block.clone(), canvas_area); // Clone for the title, draw border over full area

    let inner_canvas_area = canvas_area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });

    // Render the art canvas
    // y_cell iterates over the rows of terminal cells in the canvas_area
    for y_cell in 0..inner_canvas_area.height {
        // x_cell iterates over the columns of terminal cells in the canvas_area
        for x_cell in 0..inner_canvas_area.width {
            // These are the art pixel coordinates corresponding to the current cell
            let art_px_x = x_cell as i32;
            let art_px_y_top = (y_cell * 2) as i32;
            let art_px_y_bottom = (y_cell * 2 + 1) as i32;

            let mut top_pixel_color = Color::DarkGray; // Default for top half of the cell
            let mut bottom_pixel_color = Color::DarkGray; // Default for bottom half of the cell

            if let Some(art) = &app.current_editing_art {
                // Check if the current cell's corresponding art pixels are within the defined art dimensions
                if art_px_x < app.art_editor_canvas_width as i32 {
                    // Top pixel of the cell
                    if art_px_y_top < app.art_editor_canvas_height as i32 {
                        if let Some(pixel) = art
                            .pixels
                            .iter()
                            .find(|p| p.x == art_px_x && p.y == art_px_y_top)
                        {
                            top_pixel_color = get_ratatui_color(app, pixel.color_id, Color::White);
                        } else {
                            // No art pixel here, could draw grid dot if desired
                            // top_pixel_color remains DarkGray (or some grid color)
                        }
                    } else {
                        // This part of cell (top art pixel) is outside art's defined height.
                        // top_pixel_color remains DarkGray.
                    }

                    // Bottom pixel of the cell
                    if art_px_y_bottom < app.art_editor_canvas_height as i32 {
                        if let Some(pixel) = art
                            .pixels
                            .iter()
                            .find(|p| p.x == art_px_x && p.y == art_px_y_bottom)
                        {
                            bottom_pixel_color =
                                get_ratatui_color(app, pixel.color_id, Color::White);
                        } else {
                            // No art pixel here
                            // bottom_pixel_color remains DarkGray
                        }
                    } else {
                        // This part of cell (bottom art pixel) is outside art's defined height.
                        // bottom_pixel_color remains DarkGray.
                    }
                } else {
                    // This cell (art_px_x) is outside art's defined width.
                    // Both top_pixel_color and bottom_pixel_color remain DarkGray.
                }
            }

            let cell_char = '▀';
            let mut cell_style = Style::default().fg(top_pixel_color).bg(bottom_pixel_color);

            // Highlight cursor position
            // app.art_editor_cursor_x and app.art_editor_cursor_y are PIXEL coordinates of the cursor.
            // Check if the cursor is on one of the art pixels this cell represents.
            if art_px_x == app.art_editor_cursor_x
                && (art_px_y_top == app.art_editor_cursor_y
                    || art_px_y_bottom == app.art_editor_cursor_y)
            {
                // Ensure the cursor is actually within the drawable area of the art piece
                if app.art_editor_cursor_x < app.art_editor_canvas_width as i32
                    && app.art_editor_cursor_y < app.art_editor_canvas_height as i32
                {
                    // Get the selected color for cursor preview
                    let cursor_color =
                        get_ratatui_color(app, app.art_editor_selected_color_id, Color::Yellow);

                    if art_px_y_top == app.art_editor_cursor_y {
                        // Cursor is on the top art pixel of this cell
                        // Show the selected color that would be placed
                        cell_style = Style::default().fg(cursor_color).bg(bottom_pixel_color);
                    } else if art_px_y_bottom == app.art_editor_cursor_y {
                        // Cursor is on the bottom art pixel of this cell
                        // Show the selected color that would be placed
                        cell_style = Style::default().fg(top_pixel_color).bg(cursor_color);
                    }
                }
            }

            frame
                .buffer_mut()
                .get_mut(inner_canvas_area.x + x_cell, inner_canvas_area.y + y_cell)
                .set_char(cell_char)
                .set_style(cell_style);
        }
    }

    // Interactive Color Palette with Names
    render_color_palette(app, frame, palette_area);
}

/// Render an interactive color palette with named colors
fn render_color_palette(app: &App, frame: &mut Frame, area: Rect) {
    if app.colors.is_empty() {
        let empty_palette = Paragraph::new("No colors available").block(
            Block::default()
                .borders(Borders::ALL)
                .title("Color Palette"),
        );
        frame.render_widget(empty_palette, area);
        return;
    }

    let palette_block = Block::default()
        .borders(Borders::ALL)
        .title("Color Palette (Tab/Shift+Tab to navigate)");
    frame.render_widget(palette_block.clone(), area);

    let inner_area = area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });

    // Create color list items with names and visual indicators
    let color_items: Vec<ListItem> = app
        .colors
        .iter()
        .enumerate()
        .map(|(idx, color)| {
            let color_name = if color.name.trim().is_empty() {
                format!("Color {}", color.id)
            } else {
                color.name.clone()
            };

            let is_selected = app.art_editor_selected_color_id == color.id;
            let _is_highlighted = idx == app.art_editor_color_palette_index;

            // Create visual representation with color block and name
            let _color_display = format!("█ {} (ID: {})", color_name, color.id);

            let mut spans = vec![
                Span::styled(
                    "█ ",
                    Style::default().fg(Color::Rgb(color.red, color.green, color.blue)),
                ),
                Span::styled(
                    color_name.clone(),
                    if is_selected {
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Gray)
                    },
                ),
                Span::styled(
                    format!(" (ID: {})", color.id),
                    Style::default().fg(Color::DarkGray),
                ),
            ];

            if is_selected {
                spans.insert(
                    0,
                    Span::styled(
                        "→ ",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                );
            } else {
                spans.insert(0, Span::styled("  ", Style::default()));
            }

            ListItem::new(Line::from(spans))
        })
        .collect();

    let color_list = List::new(color_items)
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    let mut list_state = ListState::default();
    list_state.select(Some(app.art_editor_color_palette_index));

    frame.render_stateful_widget(color_list, inner_area, &mut list_state);
}
