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
        | InputMode::EnterRefreshToken => {
            let title = match app.input_mode {
                InputMode::EnterCustomBaseUrlText => "Custom Base URL (Editing):",
                InputMode::EnterAccessToken => "Access Token (Editing):",
                InputMode::EnterRefreshToken => "Refresh Token (Editing):",
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
        _ => {
            // For InputMode::None or ArtEditor modes, show current config (simplified)
            let mut display_text = format!("Base: {}", app.api_client.get_base_url_preview());
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
        InputMode::ArtEditor | InputMode::ArtEditorFileName => {
            // ArtEditorFileName should also show editor
            render_art_editor_ui(app, frame, main_layout[1]);
        }
        _ => {
            // Includes EnterBaseUrl, EnterCustomBaseUrlText, EnterAccessToken, EnterRefreshToken, None
            let board_area = main_layout[1];
            let board_block = Block::default().borders(Borders::ALL).title(format!(
                "Board Display (Viewport @ {},{} - Size {}x{})",
                app.board_viewport_x,
                app.board_viewport_y,
                app.board.len(),
                if app.board.is_empty() {
                    0
                } else {
                    app.board[0].len()
                }
            ));
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

    let editor_block = Block::default()
        .borders(Borders::ALL)
        .title(format!(
            "Pixel Art Editor (Canvas: {}x{}, Cursor: {},{}, Color: {}) - Arrows, Space, s:Save, Esc:Exit",
            app.art_editor_canvas_width,
            app.art_editor_canvas_height,
            app.art_editor_cursor_x,
            app.art_editor_cursor_y,
            app.art_editor_selected_color_id
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
                    if art_px_y_top == app.art_editor_cursor_y {
                        // Cursor is on the top art pixel of this cell
                        // If the original top_pixel_color was the default DarkGray (empty),
                        // make the yellow more prominent. Otherwise, blend with existing color.
                        if top_pixel_color == Color::DarkGray {
                            cell_style = Style::default().fg(Color::Yellow).bg(bottom_pixel_color);
                        } else {
                            cell_style = cell_style.fg(Color::Yellow); // Highlight by changing foreground
                        }
                    } else if art_px_y_bottom == app.art_editor_cursor_y {
                        // Cursor is on the bottom art pixel of this cell
                        if bottom_pixel_color == Color::DarkGray {
                            cell_style = Style::default().fg(top_pixel_color).bg(Color::Yellow);
                        } else {
                            cell_style = cell_style.bg(Color::Yellow); // Highlight by changing background
                        }
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

    // Placeholder for Color Palette
    let palette_block = Block::default()
        .borders(Borders::ALL)
        .title("Colors (TODO)");
    frame.render_widget(palette_block, palette_area);
    let colors_text = app
        .colors
        .iter()
        .map(|c| {
            format!(
                "ID {}: RGB({},{},{})
",
                c.id, c.red, c.green, c.blue
            )
        })
        .collect::<String>();
    let palette_content = Paragraph::new(colors_text).wrap(Wrap { trim: true });
    frame.render_widget(
        palette_content,
        palette_area.inner(Margin {
            vertical: 1,
            horizontal: 1,
        }),
    );

    // If in filename input mode, render that input field over the palette or status bar
    if app.input_mode == InputMode::ArtEditorFileName {
        let popup_area = centered_rect(60, 20, frame.size()); // Adjust size as needed
        let filename_input_area = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3)].as_ref())
            .split(popup_area)[0]; // Take the top part for the input box

        let filename_input_widget = Paragraph::new(app.art_editor_filename_buffer.as_str()).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Save Art As (Enter to Save, Esc to Cancel):"),
        );
        frame.render_widget(Clear, filename_input_area); // Clear the area first
        frame.render_widget(filename_input_widget, filename_input_area);
        frame.set_cursor(
            filename_input_area.x + app.art_editor_filename_buffer.chars().count() as u16 + 1,
            filename_input_area.y + 1,
        );
    }
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
        Line::from(" Arrows: Scroll board viewport"),
        Line::from(""),
        Line::from(Span::styled(
            "--- Pixel Art Placement ---",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(" l: Load default art (shows art controls)"),
        Line::from(" Arrows: Move loaded art on board"),
        Line::from(" Enter: Place loaded art at current position"),
        Line::from(""),
        Line::from(Span::styled(
            "--- Pixel Art Editor (enter with 'e') ---",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(" Arrows: Move cursor on canvas"),
        Line::from(" Space: Draw pixel with selected color"),
        Line::from(" s: Save current art to file (prompts for name)"),
        Line::from(" Esc: Exit editor (changes not saved automatically)"),
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
