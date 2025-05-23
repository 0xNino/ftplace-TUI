use crate::app_state::{App, InputMode};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};

pub fn render_ui(app: &mut App, frame: &mut Frame) {
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Increased height for Base URL selection list or input
            Constraint::Min(0),    // Board Display or Art Editor
            Constraint::Length(5), // Controls / Status
        ])
        .split(frame.size());

    // --- Input Area (Top) ---
    let input_area_rect = main_layout[0];
    match app.input_mode {
        InputMode::EnterBaseUrl => {
            let items: Vec<ListItem> = app
                .base_url_options
                .iter()
                .map(|opt| ListItem::new(opt.as_str()))
                .collect();

            let list_widget = List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Select API Base URL (Enter to confirm, q to quit):"),
                )
                .highlight_style(
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .bg(Color::LightBlue),
                )
                .highlight_symbol("> ");

            let mut list_state = ListState::default();
            list_state.select(Some(app.base_url_selection_index));

            frame.render_stateful_widget(list_widget, input_area_rect, &mut list_state);
        }
        InputMode::EnterCustomBaseUrlText
        | InputMode::EnterAccessToken
        | InputMode::EnterRefreshToken
        | InputMode::ArtEditorNewArtName => {
            let title = match app.input_mode {
                InputMode::EnterCustomBaseUrlText => "Custom Base URL (Editing):",
                InputMode::EnterAccessToken => "Access Token (Editing):",
                InputMode::EnterRefreshToken => "Refresh Token (Editing):",
                InputMode::ArtEditorNewArtName => "New Pixel Art Name (Editing):",
                _ => "Input:", // Should not happen if logic is correct
            };
            let input_widget = Paragraph::new(app.input_buffer.as_str())
                .block(Block::default().borders(Borders::ALL).title(title));
            frame.render_widget(input_widget, input_area_rect);
            frame.set_cursor(
                input_area_rect.x + app.input_buffer.chars().count() as u16 + 1,
                input_area_rect.y + 1,
            );
        }
        InputMode::ArtSelection => {
            render_art_selection_ui(app, frame, input_area_rect);
        }
        InputMode::ArtQueue => {
            render_art_queue_ui(app, frame, input_area_rect);
        }
        _ => {
            // For InputMode::None or ArtEditor modes, show current config (simplified)
            let mut display_text =
                format!("Base: {}", app.api_client.get_base_url_config_display());
            if let Some(token_preview) = app.api_client.get_auth_cookie_preview() {
                display_text.push_str(&format!("; Token: [{}...]", token_preview));
            } else {
                display_text.push_str("; Token: [not set]");
            }
            let config_display_widget = Paragraph::new(display_text).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Current Config (c to edit AccessToken)"),
            );
            frame.render_widget(config_display_widget, input_area_rect);
        }
    }

    // --- Board Display Area or Art Editor Area (main_layout[1]) ---
    match app.input_mode {
        InputMode::ArtEditor => {
            render_art_editor_ui(app, frame, main_layout[1]);
        }
        _ => {
            // Includes EnterBaseUrl, EnterCustomBaseUrlText, EnterAccessToken, EnterRefreshToken, None
            let board_area = main_layout[1];
            let board_title = if app.board_loading {
                let elapsed = app
                    .board_load_start
                    .map(|start| start.elapsed().as_secs())
                    .unwrap_or(0);
                format!(
                    "Board Display - Loading... ({}s) - Size {}x{}",
                    elapsed,
                    app.board.len(),
                    if app.board.is_empty() {
                        0
                    } else {
                        app.board[0].len()
                    }
                )
            } else {
                format!(
                    "Board Display (Viewport @ {},{} - Size {}x{})",
                    app.board_viewport_x,
                    app.board_viewport_y,
                    app.board.len(),
                    if app.board.is_empty() {
                        0
                    } else {
                        app.board[0].len()
                    }
                )
            };

            let board_block = Block::default().borders(Borders::ALL).title(board_title);
            frame.render_widget(board_block, board_area);

            let inner_board_area = main_layout[1].inner(Margin {
                vertical: 1,
                horizontal: 1,
            });

            // Clamp viewport coordinates
            let board_pixel_width = app.board.len();
            let board_pixel_height = if board_pixel_width > 0 {
                app.board[0].len()
            } else {
                0
            };

            if board_pixel_height > (inner_board_area.height * 2) as usize {
                let max_scroll_y_pixels =
                    (board_pixel_height - (inner_board_area.height * 2) as usize) as u16;
                app.board_viewport_y = app.board_viewport_y.min(max_scroll_y_pixels);
            } else {
                app.board_viewport_y = 0;
            }
            if board_pixel_width > inner_board_area.width as usize {
                let max_scroll_x_pixels =
                    (board_pixel_width - inner_board_area.width as usize) as u16;
                app.board_viewport_x = app.board_viewport_x.min(max_scroll_x_pixels);
            } else {
                app.board_viewport_x = 0;
            }

            let default_board_color_info = app.colors.iter().find(|c| c.id == 1);
            let default_board_rgb = default_board_color_info
                .map_or(Color::Black, |ci| Color::Rgb(ci.red, ci.green, ci.blue)); // Fallback to Black if color 1 not found

            if !app.board.is_empty() && !app.colors.is_empty() {
                for y_screen_cell in 0..inner_board_area.height {
                    for x_screen_cell in 0..inner_board_area.width {
                        let board_px_x = app.board_viewport_x as usize + x_screen_cell as usize;
                        let board_px_y_top =
                            app.board_viewport_y as usize + (y_screen_cell * 2) as usize;
                        let board_px_y_bottom = board_px_y_top + 1;

                        let top_pixel_color = if board_px_x < app.board.len()
                            && board_px_y_top < app.board[board_px_x].len()
                        {
                            app.board[board_px_x][board_px_y_top]
                                .as_ref()
                                .map_or(default_board_rgb, |p| {
                                    get_ratatui_color(app, p.c, default_board_rgb)
                                })
                        } else {
                            default_board_rgb // Out of bounds for top pixel
                        };

                        let bottom_pixel_color = if board_px_x < app.board.len()
                            && board_px_y_bottom < app.board[board_px_x].len()
                        {
                            app.board[board_px_x][board_px_y_bottom]
                                .as_ref()
                                .map_or(default_board_rgb, |p| {
                                    get_ratatui_color(app, p.c, default_board_rgb)
                                })
                        } else {
                            default_board_rgb // Out of bounds for bottom pixel, or if board has odd height and this is the last cell row
                        };

                        let cell_char = '▀';
                        let style = Style::default().fg(top_pixel_color).bg(bottom_pixel_color);

                        frame
                            .buffer_mut()
                            .get_mut(
                                inner_board_area.x + x_screen_cell,
                                inner_board_area.y + y_screen_cell,
                            )
                            .set_char(cell_char)
                            .set_style(style);
                    }
                }
            }

            // Overlay loaded_art if present - this needs to be aware of the half-blocks and viewport
            if let Some(art) = &app.loaded_art {
                for art_pixel in &art.pixels {
                    let art_abs_x = art.board_x + art_pixel.x;
                    let art_abs_y = art.board_y + art_pixel.y;

                    // Is this art pixel visible in the current viewport?
                    if art_abs_x >= app.board_viewport_x as i32
                        && art_abs_x < (app.board_viewport_x + inner_board_area.width) as i32
                        && art_abs_y >= app.board_viewport_y as i32
                        && art_abs_y < (app.board_viewport_y + inner_board_area.height * 2) as i32
                    {
                        let screen_cell_x = (art_abs_x - app.board_viewport_x as i32) as u16;
                        // art_abs_y is the pixel row. The cell row is (art_abs_y - viewport_y) / 2
                        let screen_cell_y = ((art_abs_y - app.board_viewport_y as i32) / 2) as u16;

                        let target_abs_screen_x = inner_board_area.x + screen_cell_x;
                        let target_abs_screen_y = inner_board_area.y + screen_cell_y;

                        // Ensure the target cell is within the drawable inner_board_area bounds
                        if screen_cell_x < inner_board_area.width
                            && screen_cell_y < inner_board_area.height
                        {
                            let art_color =
                                get_ratatui_color(app, art_pixel.color_id, Color::Magenta);
                            let cell = frame
                                .buffer_mut()
                                .get_mut(target_abs_screen_x, target_abs_screen_y);

                            cell.set_char('▀');
                            if (art_abs_y - app.board_viewport_y as i32) % 2 == 0 {
                                cell.set_fg(art_color);
                            } else {
                                cell.set_bg(art_color);
                            }
                        }
                    }
                }
            }

            // Overlay queue previews with progress-aware visual feedback
            if !app.art_queue.is_empty() {
                for queue_item in &app.art_queue {
                    // Show all queue items (pending, in progress, complete)
                    if queue_item.status == crate::app_state::QueueStatus::Failed
                        || queue_item.status == crate::app_state::QueueStatus::Skipped
                    {
                        continue; // Don't show failed/skipped items
                    }

                    // Filter meaningful pixels for this queue item
                    let meaningful_pixels: Vec<_> =
                        queue_item.art.pixels.iter().enumerate().collect();

                    for (pixel_index, art_pixel) in meaningful_pixels {
                        let art_abs_x = queue_item.art.board_x + art_pixel.x;
                        let art_abs_y = queue_item.art.board_y + art_pixel.y;

                        // Is this art pixel visible in the current viewport?
                        if art_abs_x >= app.board_viewport_x as i32
                            && art_abs_x < (app.board_viewport_x + inner_board_area.width) as i32
                            && art_abs_y >= app.board_viewport_y as i32
                            && art_abs_y
                                < (app.board_viewport_y + inner_board_area.height * 2) as i32
                        {
                            let screen_cell_x = (art_abs_x - app.board_viewport_x as i32) as u16;
                            let screen_cell_y =
                                ((art_abs_y - app.board_viewport_y as i32) / 2) as u16;

                            let target_abs_screen_x = inner_board_area.x + screen_cell_x;
                            let target_abs_screen_y = inner_board_area.y + screen_cell_y;

                            // Ensure the target cell is within bounds
                            if screen_cell_x < inner_board_area.width
                                && screen_cell_y < inner_board_area.height
                            {
                                let cell = frame
                                    .buffer_mut()
                                    .get_mut(target_abs_screen_x, target_abs_screen_y);

                                // Determine pixel state: placed, current, or pending
                                let is_placed = pixel_index < queue_item.pixels_placed;
                                let is_current = pixel_index == queue_item.pixels_placed
                                    && queue_item.status
                                        == crate::app_state::QueueStatus::InProgress;
                                let is_pending = pixel_index >= queue_item.pixels_placed
                                    && queue_item.status == crate::app_state::QueueStatus::Pending;

                                if is_placed {
                                    // Show placed pixels as dimmed (low intensity)
                                    let preview_color = match queue_item.priority {
                                        1 => Color::Indexed(88), // Dark red
                                        2 => Color::Indexed(94), // Dark yellow
                                        3 => Color::Indexed(23), // Dark cyan
                                        4 => Color::Indexed(22), // Dark green
                                        5 => Color::Indexed(18), // Dark blue
                                        _ => Color::DarkGray,    // Default
                                    };

                                    cell.set_char('▀');
                                    if (art_abs_y - app.board_viewport_y as i32) % 2 == 0 {
                                        cell.set_fg(preview_color);
                                    } else {
                                        cell.set_bg(preview_color);
                                    }
                                } else if is_current {
                                    // Show current pixel being processed with bright white
                                    cell.set_char('▀');
                                    if (art_abs_y - app.board_viewport_y as i32) % 2 == 0 {
                                        cell.set_fg(Color::White);
                                    } else {
                                        cell.set_bg(Color::White);
                                    }
                                } else if is_pending {
                                    // Show pending pixels with blinking effect
                                    if app.queue_blink_state {
                                        // Priority-based colors for blink
                                        let preview_color = match queue_item.priority {
                                            1 => Color::Red,     // High priority - red
                                            2 => Color::Yellow,  // High-medium - yellow
                                            3 => Color::Cyan,    // Medium - cyan
                                            4 => Color::Green,   // Low-medium - green
                                            5 => Color::Blue,    // Low priority - blue
                                            _ => Color::Magenta, // Default
                                        };

                                        cell.set_char('▀');
                                        if (art_abs_y - app.board_viewport_y as i32) % 2 == 0 {
                                            cell.set_fg(preview_color);
                                        } else {
                                            cell.set_bg(preview_color);
                                        }
                                    }
                                    // When blink_state is false, show original content
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // --- Status Message Area (main_layout[2]) ---
    let status_widget = Paragraph::new(app.status_message.as_str())
        .wrap(Wrap { trim: true })
        .block(Block::default().borders(Borders::ALL).title("Status"));
    frame.render_widget(status_widget, main_layout[2]);

    // Cursor handling is now within specific input mode rendering logic above for text input
    // or handled by ListState for selection.

    // If ShowHelp mode is active, render the help popup on top of everything else
    if app.input_mode == InputMode::ShowHelp {
        render_help_popup(app, frame);
    }

    // If ShowProfile mode is active, render the profile popup on top of everything else
    if app.input_mode == InputMode::ShowProfile {
        render_profile_popup(app, frame);
    }
}

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
            let is_highlighted = idx == app.art_editor_color_palette_index;

            // Create visual representation with color block and name
            let color_display = format!("█ {} (ID: {})", color_name, color.id);

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

fn render_help_popup(app: &App, frame: &mut Frame) {
    let popup_area = centered_rect(60, 50, frame.size()); // Adjust size as needed

    let help_text = vec![
        Line::from(Span::styled(
            "--- General ---",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(" q: Quit application"),
        Line::from(" ?: Toggle this help screen"),
        Line::from(" c: Configure/Re-enter Access Token"),
        Line::from(" r: Refresh board data"),
        Line::from(" p: Fetch profile data"),
        Line::from(" i: Show user profile panel"),
        Line::from(" w: Work queue management"),
        Line::from(" Arrows: Scroll board viewport"),
        Line::from(""),
        Line::from(Span::styled(
            "--- Pixel Art Placement ---",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(" l: Open art selection"),
        Line::from(" Arrows: Navigate available arts"),
        Line::from(" Enter: Load selected art for positioning"),
        Line::from(""),
        Line::from(Span::styled(
            "--- Loaded Art (positioning & placement) ---",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(" Arrows: Move loaded art on board"),
        Line::from(" Enter: Add positioned art to queue & start processing"),
        Line::from(" Esc: Cancel loaded art or stop queue processing"),
        Line::from(""),
        Line::from(Span::styled(
            "--- Pixel Art Editor (enter with 'e') ---",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(" Arrows: Move cursor on canvas"),
        Line::from(" Space: Draw pixel with selected color"),
        Line::from(" Tab/Shift+Tab: Navigate color palette"),
        Line::from(" s: Save current art to file (prompts for name)"),
        Line::from(" Esc: Exit editor (changes not saved automatically)"),
        Line::from(""),
        Line::from(Span::styled(
            "--- Work Queue System (enter with 'w') ---",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(" w: Open work queue management"),
        Line::from(" ↑/↓: Navigate queue items"),
        Line::from(" u/k: Move item up in queue"),
        Line::from(" j/n: Move item down in queue"),
        Line::from(" Enter: Start automated queue processing"),
        Line::from(" 1-5: Set priority for selected queue item"),
        Line::from(" d/Del: Remove item from queue"),
        Line::from(" c: Clear entire queue"),
        Line::from(""),
        Line::from(Span::styled(
            "--- Input Fields (Tokens, Filenames, etc.) ---",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(" Enter: Confirm input"),
        Line::from(" Esc: Cancel input / Go back"),
        Line::from(" Backspace: Delete last character"),
    ];

    let help_paragraph = Paragraph::new(help_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Help - Available Commands (Press Esc, q, or ? to close)"),
        )
        .wrap(Wrap { trim: false }); // trim: false to keep blank lines for spacing

    frame.render_widget(Clear, popup_area); // Clear the area under the popup
    frame.render_widget(help_paragraph, popup_area);
}

fn render_profile_popup(app: &App, frame: &mut Frame) {
    let popup_area = centered_rect(70, 60, frame.size());

    let profile_text = if let Some(user_info) = &app.user_info {
        let mut lines = vec![
            Line::from(Span::styled(
                "--- User Profile ---",
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Cyan),
            )),
            Line::from(""),
        ];

        // User basic info
        if let Some(username) = &user_info.username {
            lines.push(Line::from(vec![
                Span::styled("Username: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(username, Style::default().fg(Color::Green)),
            ]));
        }

        if let Some(id) = user_info.id {
            lines.push(Line::from(vec![
                Span::styled("User ID: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(id.to_string(), Style::default().fg(Color::Yellow)),
            ]));
        }

        if let Some(campus_name) = &user_info.campus_name {
            lines.push(Line::from(vec![
                Span::styled("Campus: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(campus_name, Style::default().fg(Color::Magenta)),
            ]));
        }

        lines.push(Line::from(""));

        // Stats section
        lines.push(Line::from(Span::styled(
            "--- Statistics ---",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Cyan),
        )));

        if let Some(num) = user_info.num {
            lines.push(Line::from(vec![
                Span::styled(
                    "Total Pixels Placed: ",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(num.to_string(), Style::default().fg(Color::Green)),
            ]));
        }

        if let Some(min_px) = user_info.min_px {
            lines.push(Line::from(vec![
                Span::styled(
                    "Min Pixels Required: ",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(min_px.to_string(), Style::default().fg(Color::Yellow)),
            ]));
        }

        lines.push(Line::from(vec![
            Span::styled(
                "Pixel Buffer: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                user_info.pixel_buffer.to_string(),
                Style::default().fg(Color::Blue),
            ),
        ]));

        lines.push(Line::from(vec![
            Span::styled(
                "Pixel Timer: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}ms", user_info.pixel_timer),
                Style::default().fg(Color::Cyan),
            ),
        ]));

        lines.push(Line::from(""));

        // Permissions section
        lines.push(Line::from(Span::styled(
            "--- Permissions ---",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Cyan),
        )));

        if let Some(is_admin) = user_info.soft_is_admin {
            let status = if is_admin { "Yes" } else { "No" };
            let color = if is_admin { Color::Red } else { Color::Gray };
            lines.push(Line::from(vec![
                Span::styled("Admin: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(status, Style::default().fg(color)),
            ]));
        }

        if let Some(is_banned) = user_info.soft_is_banned {
            let status = if is_banned { "Yes" } else { "No" };
            let color = if is_banned { Color::Red } else { Color::Green };
            lines.push(Line::from(vec![
                Span::styled("Banned: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(status, Style::default().fg(color)),
            ]));
        }

        lines.push(Line::from(""));

        // JWT info section if available
        if user_info.iat.is_some() || user_info.exp.is_some() {
            lines.push(Line::from(Span::styled(
                "--- Token Info ---",
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Cyan),
            )));

            if let Some(iat) = user_info.iat {
                let iat_time = chrono::DateTime::from_timestamp(iat, 0)
                    .map(|dt| {
                        (dt + chrono::Duration::hours(2))
                            .format("%Y-%m-%d %H:%M:%S UTC+2")
                            .to_string()
                    })
                    .unwrap_or_else(|| "Invalid timestamp".to_string());
                lines.push(Line::from(vec![
                    Span::styled(
                        "Token Issued: ",
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(iat_time, Style::default().fg(Color::Gray)),
                ]));
            }

            if let Some(exp) = user_info.exp {
                let exp_time = chrono::DateTime::from_timestamp(exp, 0)
                    .map(|dt| {
                        (dt + chrono::Duration::hours(2))
                            .format("%Y-%m-%d %H:%M:%S UTC+2")
                            .to_string()
                    })
                    .unwrap_or_else(|| "Invalid timestamp".to_string());
                lines.push(Line::from(vec![
                    Span::styled(
                        "Token Expires: ",
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(exp_time, Style::default().fg(Color::Gray)),
                ]));
            }

            lines.push(Line::from(""));
        }

        // Timers section if available
        if let Some(timers) = &user_info.timers {
            if !timers.is_empty() {
                lines.push(Line::from(Span::styled(
                    "--- Active Timers ---",
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .fg(Color::Cyan),
                )));

                for (i, timer) in timers.iter().enumerate() {
                    let timer_time = chrono::DateTime::from_timestamp(*timer, 0)
                        .map(|dt| {
                            (dt + chrono::Duration::hours(2))
                                .format("%Y-%m-%d %H:%M:%S UTC+2")
                                .to_string()
                        })
                        .unwrap_or_else(|| "Invalid timestamp".to_string());
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("Timer {}: ", i + 1),
                            Style::default().add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(timer_time, Style::default().fg(Color::Yellow)),
                    ]));
                }
                lines.push(Line::from(""));
            }
        }

        lines.push(Line::from(Span::styled(
            "Press Esc, q, or i to close",
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
        )));

        lines
    } else {
        vec![
            Line::from(Span::styled(
                "--- User Profile ---",
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Cyan),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "No user profile data available.",
                Style::default().fg(Color::Red),
            )),
            Line::from(""),
            Line::from("Please fetch profile data first with 'p' key."),
            Line::from(""),
            Line::from(Span::styled(
                "Press Esc, q, or i to close",
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::ITALIC),
            )),
        ]
    };

    let profile_paragraph = Paragraph::new(profile_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("User Profile (Press Esc, q, or i to close)"),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(Clear, popup_area);
    frame.render_widget(profile_paragraph, popup_area);
}

/// Render the art selection UI with previews
fn render_art_selection_ui(app: &App, frame: &mut Frame, area: Rect) {
    if app.available_pixel_arts.is_empty() {
        let empty_message = Paragraph::new("No pixel arts available").block(
            Block::default()
                .borders(Borders::ALL)
                .title("Art Selection"),
        );
        frame.render_widget(empty_message, area);
        return;
    }

    let selection_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50), // Art list
            Constraint::Percentage(50), // Preview
        ])
        .split(area);

    // Render art list on the left
    let art_items: Vec<ListItem> = app
        .available_pixel_arts
        .iter()
        .enumerate()
        .map(|(idx, art)| {
            let dimensions = crate::art::get_art_dimensions(art);
            let item_text = format!(
                "{} ({}x{}, {} pixels)",
                art.name,
                dimensions.0,
                dimensions.1,
                art.pixels.len()
            );

            if idx == app.art_selection_index {
                ListItem::new(item_text).style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                ListItem::new(item_text)
            }
        })
        .collect();

    let art_list = List::new(art_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Select Pixel Art (Enter to load for positioning, Esc to cancel)"),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    let mut list_state = ListState::default();
    list_state.select(Some(app.art_selection_index));

    frame.render_stateful_widget(art_list, selection_layout[0], &mut list_state);

    // Render preview on the right
    if let Some(selected_art) = app.available_pixel_arts.get(app.art_selection_index) {
        render_art_preview(selected_art, app, frame, selection_layout[1]);
    }
}

/// Render a preview of a pixel art
fn render_art_preview(art: &crate::art::PixelArt, app: &App, frame: &mut Frame, area: Rect) {
    let preview_block = Block::default()
        .borders(Borders::ALL)
        .title(format!("Preview: {}", art.name));
    frame.render_widget(preview_block.clone(), area);

    let inner_area = area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });

    if art.pixels.is_empty() {
        let empty_preview = Paragraph::new("(Empty art)").style(Style::default().fg(Color::Gray));
        frame.render_widget(empty_preview, inner_area);
        return;
    }

    // Calculate art bounds
    let dimensions = crate::art::get_art_dimensions(art);
    let art_width = dimensions.0 as u16;
    let art_height = dimensions.1 as u16;

    // Scale preview to fit available space
    let max_preview_width = inner_area.width;
    let max_preview_height = inner_area.height * 2; // Since we use half-blocks

    let scale_x = if art_width == 0 {
        1.0
    } else {
        max_preview_width as f32 / art_width as f32
    };
    let scale_y = if art_height == 0 {
        1.0
    } else {
        max_preview_height as f32 / art_height as f32
    };
    let scale = scale_x.min(scale_y).min(1.0); // Don't scale up, only down

    let preview_width = (art_width as f32 * scale) as u16;
    let preview_height = ((art_height as f32 * scale) / 2.0) as u16; // Divide by 2 for half-blocks

    // Center the preview
    let start_x = inner_area.x + (inner_area.width.saturating_sub(preview_width)) / 2;
    let start_y = inner_area.y + (inner_area.height.saturating_sub(preview_height)) / 2;

    // Render the art preview using half-blocks
    for screen_y in 0..preview_height {
        for screen_x in 0..preview_width {
            let art_pixel_y_top = ((screen_y * 2) as f32 / scale) as i32;
            let art_pixel_y_bottom = art_pixel_y_top + (1.0 / scale) as i32;
            let art_pixel_x = (screen_x as f32 / scale) as i32;

            let top_pixel_color = art
                .pixels
                .iter()
                .find(|p| p.x == art_pixel_x && p.y == art_pixel_y_top)
                .map(|p| get_ratatui_color(app, p.color_id, Color::DarkGray))
                .unwrap_or(Color::DarkGray);

            let bottom_pixel_color = art
                .pixels
                .iter()
                .find(|p| p.x == art_pixel_x && p.y == art_pixel_y_bottom)
                .map(|p| get_ratatui_color(app, p.color_id, Color::DarkGray))
                .unwrap_or(Color::DarkGray);

            let cell_char = '▀';
            let style = Style::default().fg(top_pixel_color).bg(bottom_pixel_color);

            if start_x + screen_x < frame.size().width && start_y + screen_y < frame.size().height {
                frame
                    .buffer_mut()
                    .get_mut(start_x + screen_x, start_y + screen_y)
                    .set_char(cell_char)
                    .set_style(style);
            }
        }
    }
}

/// Render the art queue management UI
fn render_art_queue_ui(app: &App, frame: &mut Frame, area: Rect) {
    if app.art_queue.is_empty() {
        let empty_message = Paragraph::new(vec![
            Line::from("Queue is empty"),
            Line::from(""),
            Line::from("Controls:"),
            Line::from("  l : Open art selection to add arts to queue"),
            Line::from("  Esc : Return to main view"),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Art Queue Management"),
        );
        frame.render_widget(empty_message, area);
        return;
    }

    let queue_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(70), // Queue list
            Constraint::Percentage(30), // Controls/Info
        ])
        .split(area);

    // Render queue list
    let queue_items: Vec<ListItem> = app
        .art_queue
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            let status_symbol = match item.status {
                crate::app_state::QueueStatus::Pending => "⏳",
                crate::app_state::QueueStatus::InProgress => "🚀",
                crate::app_state::QueueStatus::Complete => "✅",
                crate::app_state::QueueStatus::Skipped => "⏭️",
                crate::app_state::QueueStatus::Failed => "❌",
            };

            let priority_color = match item.priority {
                1 => Color::Red,
                2 => Color::Yellow,
                3 => Color::Cyan,
                4 => Color::Green,
                5 => Color::Blue,
                _ => Color::White,
            };

            let progress = if item.pixels_total > 0 {
                format!(" {}/{}", item.pixels_placed, item.pixels_total)
            } else {
                String::new()
            };

            let item_text = format!(
                "{} P{} '{}' @ ({},{}){}",
                status_symbol,
                item.priority,
                item.art.name,
                item.art.board_x,
                item.art.board_y,
                progress
            );

            let mut list_item = ListItem::new(item_text);
            if idx == app.queue_selection_index {
                list_item = list_item.style(
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                );
            }

            list_item.style(Style::default().fg(priority_color))
        })
        .collect();

    let queue_list = List::new(queue_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("Art Queue ({} items)", app.art_queue.len())),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    let mut list_state = ListState::default();
    list_state.select(Some(app.queue_selection_index));

    frame.render_stateful_widget(queue_list, queue_layout[0], &mut list_state);

    // Render controls and info panel
    let pending_count = app
        .art_queue
        .iter()
        .filter(|item| item.status == crate::app_state::QueueStatus::Pending)
        .count();

    let total_pixels = app
        .art_queue
        .iter()
        .filter(|item| item.status == crate::app_state::QueueStatus::Pending)
        .map(|item| item.pixels_total)
        .sum::<usize>();

    let controls_text = vec![
        Line::from(Span::styled(
            "Queue Statistics",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(format!("Pending: {}", pending_count)),
        Line::from(format!("Total Pixels: {}", total_pixels)),
        Line::from(""),
        Line::from(Span::styled(
            "Controls",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from("↑/↓  : Navigate queue"),
        Line::from("u/k  : Move item up"),
        Line::from("j/n  : Move item down"),
        Line::from("Enter: Start processing"),
        Line::from("1-5  : Set priority"),
        Line::from("d/Del: Remove item"),
        Line::from("c    : Clear queue"),
        Line::from("l    : Add more arts"),
        Line::from("Esc  : Exit queue view"),
        Line::from(""),
        Line::from(Span::styled(
            "Priority Colors",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("1-High ", Style::default().fg(Color::Red)),
            Span::styled("2-Med+ ", Style::default().fg(Color::Yellow)),
            Span::styled("3-Med ", Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled("4-Low+ ", Style::default().fg(Color::Green)),
            Span::styled("5-Low ", Style::default().fg(Color::Blue)),
        ]),
    ];

    let controls_paragraph = Paragraph::new(controls_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Info & Controls"),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(controls_paragraph, queue_layout[1]);
}
