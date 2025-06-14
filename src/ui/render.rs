use crate::app_state::{App, InputMode};
use crate::ui::art_editor::render_art_editor_ui;
use crate::ui::art_management::{
    render_art_preview_fullscreen, render_art_preview_ui, render_art_queue_ui,
    render_art_selection_ui, render_share_selection_ui,
};
use crate::ui::helpers::{
    get_current_board_color_ui, get_ratatui_color, is_pixel_already_correct_ui,
};
use crate::ui::popups::{render_help_popup, render_profile_popup, render_status_log_popup};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};

pub fn render_ui(app: &mut App, frame: &mut Frame) {
    // Check if terminal is wide enough for side-by-side layout
    let use_wide_layout = frame.size().width >= 140;

    // Create different layouts based on width
    let main_layout = if use_wide_layout {
        // Wide layout: Input area + full content area (no status box)
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5), // Input area
                Constraint::Min(0),    // Content area (board + log) - takes all remaining space
            ])
            .split(frame.size())
    } else {
        // Standard layout: Input area + content area + status box
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5), // Input area
                Constraint::Min(0),    // Content area (board only)
                Constraint::Length(6), // Status box
            ])
            .split(frame.size())
    };

    // --- Input Area (Top) ---
    let input_area_rect = main_layout[0];
    match app.input_mode {
        InputMode::EnterBaseUrl => {
            let items: Vec<ListItem> = app
                .base_url_options
                .iter()
                .map(|opt| {
                    let display_text = if opt == "https://ftplace.42lwatch.ch" {
                        format!("{} (Polylan)", opt)
                    } else {
                        opt.clone()
                    };
                    ListItem::new(display_text)
                })
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
        | InputMode::ArtEditorNewArtName
        | InputMode::EnterShareMessage
        | InputMode::EnterShareString => {
            let title = match app.input_mode {
                InputMode::EnterCustomBaseUrlText => "Custom Base URL (Editing):",
                InputMode::EnterAccessToken => "Access Token (Editing):",
                InputMode::EnterRefreshToken => "Refresh Token (Editing):",
                InputMode::ArtEditorNewArtName => "New Pixel Art Name (Editing):",
                InputMode::EnterShareMessage => "Share Message (Optional):",
                InputMode::EnterShareString => "Share String (ftplace-share: NAME at (X, Y)):",
                _ => "Input:", // Should not happen if logic is correct
            };

            // For token inputs, show beginning and end for better visibility
            let display_text = match app.input_mode {
                InputMode::EnterAccessToken | InputMode::EnterRefreshToken => {
                    let buffer = &app.input_buffer;
                    if buffer.len() > 50 {
                        // Show first 20 and last 20 characters with "..." in between
                        let start = &buffer[..20];
                        let end = &buffer[buffer.len().saturating_sub(20)..];
                        format!("{}...{} ({})", start, end, buffer.len())
                    } else {
                        buffer.clone()
                    }
                }
                _ => app.input_buffer.clone(),
            };

            let input_widget = Paragraph::new(display_text.as_str())
                .block(Block::default().borders(Borders::ALL).title(title));
            frame.render_widget(input_widget, input_area_rect);

            // For cursor positioning, use fixed position for long tokens to avoid char counting
            let cursor_pos = match app.input_mode {
                InputMode::EnterAccessToken | InputMode::EnterRefreshToken => {
                    if app.input_buffer.len() > 50 {
                        // Fixed cursor position for long tokens - avoid expensive operations
                        45 // Position after "start...end (length)"
                    } else {
                        app.input_buffer.len() as u16 // Use byte length for short tokens
                    }
                }
                _ => app.input_buffer.len() as u16, // Use byte length instead of char count
            };

            frame.set_cursor(input_area_rect.x + cursor_pos + 1, input_area_rect.y + 1);
        }
        InputMode::ArtSelection => {
            render_art_selection_ui(app, frame, input_area_rect);
        }
        InputMode::ArtPreview => {
            // Art preview is rendered as a full-screen popup later,
            // so we show the art selection UI in the background
            render_art_selection_ui(app, frame, input_area_rect);
        }
        InputMode::ArtQueue => {
            render_art_queue_ui(app, frame, input_area_rect);
        }
        InputMode::ShareSelection => {
            render_share_selection_ui(app, frame, input_area_rect);
        }
        _ => {
            // For InputMode::None or ArtEditor modes, show current config (simplified)
            let mut display_text = format!("URL: {}", app.api_client.get_base_url_config_display());
            if let Some(token_preview) = app.api_client.get_auth_cookie_preview() {
                display_text.push_str(&format!("; Token: [{}...]", token_preview));
            } else {
                display_text.push_str("; Token: [not set]");
            }

            // Add shortcuts help on a new line
            display_text.push_str("\n\nq: Quit | ?: Help | c: Configure | r: Refresh | p: Profile | h: History | w: Queue | l: Load Art");

            let config_display_widget = Paragraph::new(display_text).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Current Config & Shortcuts (b/c to configure Base URL and tokens)"),
            );
            frame.render_widget(config_display_widget, input_area_rect);
        }
    }

    // --- Main Content Area ---
    let content_area = main_layout[1];
    match app.input_mode {
        InputMode::ArtEditor => {
            render_art_editor_ui(app, frame, content_area);
        }
        InputMode::ArtPreview => {
            // For art preview, we want to use the full screen, not just the board area
            // This will be handled after the status area rendering
        }
        _ => {
            if use_wide_layout {
                // Wide layout: Board on left (80%), Log history on right (20%)
                render_wide_layout(app, frame, content_area, Rect::default()); // No status area in wide layout
            } else {
                // Standard layout: Board in center, status at bottom
                render_board_display(app, frame, content_area);
            }
        }
    }

    // --- Status Message Area - Only render if not using wide layout ---
    if !use_wide_layout {
        let status_area = main_layout[2];
        render_status_area(app, frame, status_area);
    }

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

    // If ShowStatusLog mode is active, render the status log popup on top of everything else
    if app.input_mode == InputMode::ShowStatusLog {
        render_status_log_popup(app, frame);
    }

    // If ArtPreview mode is active, render the art preview popup on top of everything else
    if app.input_mode == InputMode::ArtPreview {
        render_art_preview_ui(app, frame, frame.size());
    }

    // If ArtSelection mode is active, also render the full-screen preview of the selected art
    if app.input_mode == InputMode::ArtSelection {
        if let Some(selected_art) = app.available_pixel_arts.get(app.art_selection_index) {
            render_art_preview_fullscreen(selected_art, app, frame, frame.size());
        }
    }

    // If ArtDeleteConfirmation mode is active, render the delete confirmation dialog
    if app.input_mode == InputMode::ArtDeleteConfirmation {
        render_delete_confirmation_dialog(app, frame);
    }
}

fn render_board_display(app: &mut App, frame: &mut Frame, area: Rect) {
    // Store board area bounds for mouse coordinate conversion
    let inner_board_area = area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });

    // Get actual board dimensions
    let board_pixel_width = app.board.len();
    let board_pixel_height = if board_pixel_width > 0 {
        app.board[0].len()
    } else {
        0
    };

    // Calculate how much terminal space the actual board needs
    let board_terminal_width = board_pixel_width as u16;
    let board_terminal_height = (board_pixel_height as u16 + 1) / 2; // Each terminal row shows 2 pixels

    // Center the board within the available area
    let board_offset_x = if board_terminal_width < inner_board_area.width {
        (inner_board_area.width - board_terminal_width) / 2
    } else {
        0
    };
    let board_offset_y = if board_terminal_height < inner_board_area.height {
        (inner_board_area.height - board_terminal_height) / 2
    } else {
        0
    };

    // Calculate the actual drawable board area (centered within inner_board_area)
    let drawable_board_area = Rect {
        x: inner_board_area.x + board_offset_x,
        y: inner_board_area.y + board_offset_y,
        width: board_terminal_width.min(inner_board_area.width),
        height: board_terminal_height.min(inner_board_area.height),
    };

    // Store the centered board area bounds for mouse coordinate conversion
    app.board_area_bounds = Some((
        drawable_board_area.x,
        drawable_board_area.y,
        drawable_board_area.width,
        drawable_board_area.height,
    ));

    let board_title = if app.board_loading {
        let elapsed = app
            .board_load_start
            .map(|start| start.elapsed().as_secs())
            .unwrap_or(0);
        format!("Board Display - Loading... ({}s)", elapsed)
    } else {
        format!(
            "Board Display (Viewport @ {},{} - Size {}x{})",
            app.board_viewport_x, app.board_viewport_y, board_pixel_width, board_pixel_height
        )
    };

    let board_block = Block::default().borders(Borders::ALL).title(board_title);
    frame.render_widget(board_block, area);

    // Clamp viewport coordinates to board bounds
    if board_pixel_height > (drawable_board_area.height * 2) as usize {
        let max_scroll_y_pixels =
            (board_pixel_height - (drawable_board_area.height * 2) as usize) as u16;
        app.board_viewport_y = app.board_viewport_y.min(max_scroll_y_pixels);
    } else {
        app.board_viewport_y = 0;
    }
    if board_pixel_width > drawable_board_area.width as usize {
        let max_scroll_x_pixels = (board_pixel_width - drawable_board_area.width as usize) as u16;
        app.board_viewport_x = app.board_viewport_x.min(max_scroll_x_pixels);
    } else {
        app.board_viewport_x = 0;
    }

    let default_board_color_info = app.colors.iter().find(|c| c.id == 1);
    let default_board_rgb =
        default_board_color_info.map_or(Color::Black, |ci| Color::Rgb(ci.red, ci.green, ci.blue));

    // Render only the actual board pixels within the centered area
    if !app.board.is_empty() && !app.colors.is_empty() {
        for y_screen_cell in 0..drawable_board_area.height {
            for x_screen_cell in 0..drawable_board_area.width {
                let board_px_x = app.board_viewport_x as usize + x_screen_cell as usize;
                let board_px_y_top = app.board_viewport_y as usize + (y_screen_cell * 2) as usize;
                let board_px_y_bottom = board_px_y_top + 1;

                // Only render if within actual board bounds
                if board_px_x < board_pixel_width && board_px_y_top < board_pixel_height {
                    let top_pixel_color = app.board[board_px_x][board_px_y_top]
                        .as_ref()
                        .map_or(default_board_rgb, |p| {
                            get_ratatui_color(app, p.c, default_board_rgb)
                        });

                    let bottom_pixel_color = if board_px_y_bottom < board_pixel_height {
                        app.board[board_px_x][board_px_y_bottom]
                            .as_ref()
                            .map_or(default_board_rgb, |p| {
                                get_ratatui_color(app, p.c, default_board_rgb)
                            })
                    } else {
                        default_board_rgb // Bottom half is out of bounds
                    };

                    let cell_char = '▀';
                    let style = Style::default().fg(top_pixel_color).bg(bottom_pixel_color);

                    frame
                        .buffer_mut()
                        .get_mut(
                            drawable_board_area.x + x_screen_cell,
                            drawable_board_area.y + y_screen_cell,
                        )
                        .set_char(cell_char)
                        .set_style(style);
                }
            }
        }
    }

    // Overlay loaded_art if present - use drawable_board_area instead of inner_board_area
    if let Some(art) = &app.loaded_art {
        render_loaded_art_overlay(app, frame, &drawable_board_area, art);
    }

    // Overlay queue previews with progress-aware visual feedback
    if !app.art_queue.is_empty() {
        render_queue_overlay(app, frame, &drawable_board_area);
    }

    // Render event timer overlay if waiting for event
    if app.waiting_for_event {
        render_event_timer_overlay(app, frame, &drawable_board_area);
    }
}

fn render_wide_layout(app: &mut App, frame: &mut Frame, content_area: Rect, _status_area: Rect) {
    // Create horizontal layout: Board on left (75%), Log history on right (25%)
    let wide_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(80), // Board area - increased from 60%
            Constraint::Percentage(20), // Log history area - reduced from 40%
        ])
        .split(content_area);

    let board_area = wide_layout[0];
    let log_area = wide_layout[1];

    // Render board on the left (left-aligned instead of centered)
    render_board_display_left_aligned(app, frame, board_area);

    // Render log history on the right
    render_log_history_panel(app, frame, log_area);
}

fn render_board_display_left_aligned(app: &mut App, frame: &mut Frame, area: Rect) {
    // Similar to render_board_display but left-aligned instead of centered
    let inner_board_area = area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });

    // Get actual board dimensions
    let board_pixel_width = app.board.len();
    let board_pixel_height = if board_pixel_width > 0 {
        app.board[0].len()
    } else {
        0
    };

    // Calculate how much terminal space the actual board needs
    let board_terminal_width = board_pixel_width as u16;
    let board_terminal_height = (board_pixel_height as u16 + 1) / 2; // Each terminal row shows 2 pixels

    // Left-align the board (no centering offset)
    let board_offset_x = 0;
    let board_offset_y = if board_terminal_height < inner_board_area.height {
        (inner_board_area.height - board_terminal_height) / 2
    } else {
        0
    };

    // Calculate the actual drawable board area (left-aligned within inner_board_area)
    let drawable_board_area = Rect {
        x: inner_board_area.x + board_offset_x,
        y: inner_board_area.y + board_offset_y,
        width: board_terminal_width.min(inner_board_area.width),
        height: board_terminal_height.min(inner_board_area.height),
    };

    // Store the board area bounds for mouse coordinate conversion
    app.board_area_bounds = Some((
        drawable_board_area.x,
        drawable_board_area.y,
        drawable_board_area.width,
        drawable_board_area.height,
    ));

    let board_title = if app.board_loading {
        let elapsed = app
            .board_load_start
            .map(|start| start.elapsed().as_secs())
            .unwrap_or(0);
        format!("Board Display - Loading... ({}s)", elapsed)
    } else {
        format!(
            "Board Display (Viewport @ {},{} - Size {}x{})",
            app.board_viewport_x, app.board_viewport_y, board_pixel_width, board_pixel_height
        )
    };

    let board_block = Block::default().borders(Borders::ALL).title(board_title);
    frame.render_widget(board_block, area);

    // Clamp viewport coordinates to board bounds
    if board_pixel_height > (drawable_board_area.height * 2) as usize {
        let max_scroll_y_pixels =
            (board_pixel_height - (drawable_board_area.height * 2) as usize) as u16;
        app.board_viewport_y = app.board_viewport_y.min(max_scroll_y_pixels);
    } else {
        app.board_viewport_y = 0;
    }
    if board_pixel_width > drawable_board_area.width as usize {
        let max_scroll_x_pixels = (board_pixel_width - drawable_board_area.width as usize) as u16;
        app.board_viewport_x = app.board_viewport_x.min(max_scroll_x_pixels);
    } else {
        app.board_viewport_x = 0;
    }

    let default_board_color_info = app.colors.iter().find(|c| c.id == 1);
    let default_board_rgb =
        default_board_color_info.map_or(Color::Black, |ci| Color::Rgb(ci.red, ci.green, ci.blue));

    // Render only the actual board pixels within the left-aligned area
    if !app.board.is_empty() && !app.colors.is_empty() {
        for y_screen_cell in 0..drawable_board_area.height {
            for x_screen_cell in 0..drawable_board_area.width {
                let board_px_x = app.board_viewport_x as usize + x_screen_cell as usize;
                let board_px_y_top = app.board_viewport_y as usize + (y_screen_cell * 2) as usize;
                let board_px_y_bottom = board_px_y_top + 1;

                // Only render if within actual board bounds
                if board_px_x < board_pixel_width && board_px_y_top < board_pixel_height {
                    let top_pixel_color = app.board[board_px_x][board_px_y_top]
                        .as_ref()
                        .map_or(default_board_rgb, |p| {
                            get_ratatui_color(app, p.c, default_board_rgb)
                        });

                    let bottom_pixel_color = if board_px_y_bottom < board_pixel_height {
                        app.board[board_px_x][board_px_y_bottom]
                            .as_ref()
                            .map_or(default_board_rgb, |p| {
                                get_ratatui_color(app, p.c, default_board_rgb)
                            })
                    } else {
                        default_board_rgb // Bottom half is out of bounds
                    };

                    let cell_char = '▀';
                    let style = Style::default().fg(top_pixel_color).bg(bottom_pixel_color);

                    frame
                        .buffer_mut()
                        .get_mut(
                            drawable_board_area.x + x_screen_cell,
                            drawable_board_area.y + y_screen_cell,
                        )
                        .set_char(cell_char)
                        .set_style(style);
                }
            }
        }
    }

    // Overlay loaded_art if present
    if let Some(art) = &app.loaded_art {
        render_loaded_art_overlay(app, frame, &drawable_board_area, art);
    }

    // Overlay queue previews with progress-aware visual feedback
    if !app.art_queue.is_empty() {
        render_queue_overlay(app, frame, &drawable_board_area);
    }

    // Render event timer overlay if waiting for event
    if app.waiting_for_event {
        render_event_timer_overlay(app, frame, &drawable_board_area);
    }
}

fn render_log_history_panel(app: &App, frame: &mut Frame, area: Rect) {
    // Build log history content similar to render_status_area but focused on history
    let mut log_lines = Vec::new();
    let max_lines = (area.height.saturating_sub(2)) as usize; // Account for borders

    // Add timer status as sticky header - now handles multi-line timeline
    let timer_status = app.get_formatted_timer_status();

    // Check if this is the new timeline format (contains newlines)
    if timer_status.contains('\n') {
        // Split timeline into separate lines and add each one
        for (i, line) in timer_status.lines().enumerate() {
            let style = if i == 0 {
                // First line (title) - bold yellow
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Yellow)
            } else if line.contains('█') || line.contains('░') || line.contains('●') {
                // Timeline bar - cyan for visual emphasis
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Cyan)
            } else {
                // Other lines (labels, status) - normal yellow
                Style::default().fg(Color::Yellow)
            };

            log_lines.push(Line::from(Span::styled(line, style)));
        }
    } else {
        // Fallback for single-line format
        log_lines.push(Line::from(Span::styled(
            format!("⏱️  {}", timer_status),
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Yellow),
        )));
    }

    // Add current status message if not empty
    if !app.status_message.is_empty() {
        log_lines.push(Line::from(Span::styled(
            format!("📢 {}", app.status_message),
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Green),
        )));
    }

    log_lines.push(Line::from(Span::styled(
        "─".repeat(area.width.saturating_sub(2) as usize), // Separator line
        Style::default().fg(Color::Gray),
    )));

    // Add recent status messages (newest first)
    let mut message_count = 0;
    let max_messages = max_lines.saturating_sub(log_lines.len() + 1); // Reserve space for what we already have

    for (message, _timestamp, utc2_timestamp) in app.status_messages.iter().rev() {
        if message_count >= max_messages {
            break;
        }

        // Skip pending API calls (messages with hourglass emoji)
        if message.contains("⏳") {
            continue;
        }

        // Extract only the time portion (HH:MM:SS) from the timestamp
        let time_only = if utc2_timestamp.len() >= 19 {
            // Extract "HH:MM:SS" from "YYYY-MM-DD HH:MM:SS"
            &utc2_timestamp[11..19]
        } else {
            utc2_timestamp // Fallback to full timestamp if format is unexpected
        };

        // Create a line with time-only timestamp and message
        log_lines.push(Line::from(vec![
            Span::styled(
                format!("[{}] ", time_only),
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::ITALIC),
            ),
            Span::styled(message, Style::default().fg(Color::White)),
        ]));

        message_count += 1;
    }

    // If no messages, show placeholder
    if app.status_messages.is_empty() {
        log_lines.push(Line::from(Span::styled(
            "No status messages yet...",
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
        )));
    }

    let log_paragraph = Paragraph::new(log_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Live Activity Log (h: Full History)"),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(log_paragraph, area);
}

fn render_loaded_art_overlay(
    app: &App,
    frame: &mut Frame,
    inner_board_area: &Rect,
    art: &crate::art::PixelArt,
) {
    for art_pixel in &art.pattern {
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
            if screen_cell_x < inner_board_area.width && screen_cell_y < inner_board_area.height {
                let art_color = get_ratatui_color(app, art_pixel.color, Color::Magenta);
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

fn render_queue_overlay(app: &App, frame: &mut Frame, inner_board_area: &Rect) {
    for queue_item in &app.art_queue {
        // Show all queue items (pending, in progress, complete)
        if queue_item.status == crate::app_state::QueueStatus::Failed
            || queue_item.status == crate::app_state::QueueStatus::Skipped
        {
            continue; // Don't show failed/skipped items
        }

        // Filter meaningful pixels for this queue item (same logic as queue processing)
        let meaningful_pixels =
            filter_meaningful_pixels_for_rendering(&queue_item.art, &app.colors);

        for (pixel_index, art_pixel) in meaningful_pixels.iter().enumerate() {
            let art_abs_x = queue_item.art.board_x + art_pixel.x;
            let art_abs_y = queue_item.art.board_y + art_pixel.y;

            // Is this art pixel visible in the current viewport?
            if art_abs_x >= app.board_viewport_x as i32
                && art_abs_x < (app.board_viewport_x + inner_board_area.width) as i32
                && art_abs_y >= app.board_viewport_y as i32
                && art_abs_y < (app.board_viewport_y + inner_board_area.height * 2) as i32
            {
                let screen_cell_x = (art_abs_x - app.board_viewport_x as i32) as u16;
                let screen_cell_y = ((art_abs_y - app.board_viewport_y as i32) / 2) as u16;

                let target_abs_screen_x = inner_board_area.x + screen_cell_x;
                let target_abs_screen_y = inner_board_area.y + screen_cell_y;

                // Ensure the target cell is within bounds
                if screen_cell_x < inner_board_area.width && screen_cell_y < inner_board_area.height
                {
                    let cell = frame
                        .buffer_mut()
                        .get_mut(target_abs_screen_x, target_abs_screen_y);

                    // Check if this pixel is already correct on the board
                    let is_already_correct = is_pixel_already_correct_ui(
                        &app.board,
                        art_abs_x,
                        art_abs_y,
                        art_pixel.color,
                    );

                    // Check if this pixel is actually correct on the backend board
                    // Only show as "placed" if it's actually the correct color on the board
                    let is_actually_placed = is_already_correct;

                    // Determine pixel state: placed, current, or pending
                    let is_placed = pixel_index < queue_item.pixels_placed && is_actually_placed;
                    let is_current = pixel_index == queue_item.pixels_placed
                        && queue_item.status == crate::app_state::QueueStatus::InProgress;
                    let is_pending = (pixel_index >= queue_item.pixels_placed
                        || !is_actually_placed)
                        && queue_item.status == crate::app_state::QueueStatus::Pending;

                    // Get the target color for this pixel
                    let target_color = get_ratatui_color(app, art_pixel.color, Color::White);

                    if is_placed {
                        // Show pixels that were actually placed by queue processing AND are correct on board
                        cell.set_char('▀');
                        if (art_abs_y - app.board_viewport_y as i32) % 2 == 0 {
                            cell.set_fg(target_color);
                        } else {
                            cell.set_bg(target_color);
                        }
                    } else if is_current {
                        // Show current pixel being processed with bright white
                        cell.set_char('▀');
                        if (art_abs_y - app.board_viewport_y as i32) % 2 == 0 {
                            cell.set_fg(Color::White);
                        } else {
                            cell.set_bg(Color::White);
                        }
                    } else if is_pending && !is_already_correct {
                        // Show pending pixels that need to be changed with blinking effect
                        // Blink between current board color and target color
                        if app.queue_blink_state {
                            // Show target color when blinking on
                            cell.set_char('▀');
                            if (art_abs_y - app.board_viewport_y as i32) % 2 == 0 {
                                cell.set_fg(target_color);
                            } else {
                                cell.set_bg(target_color);
                            }
                        } else {
                            // Show current board color when blinking off
                            let current_board_color = get_current_board_color_ui(
                                &app.board,
                                &app.colors,
                                art_abs_x,
                                art_abs_y,
                            );

                            cell.set_char('▀');
                            if (art_abs_y - app.board_viewport_y as i32) % 2 == 0 {
                                cell.set_fg(current_board_color);
                            } else {
                                cell.set_bg(current_board_color);
                            }
                        }
                    }
                    // If pixel is pending but already correct, we don't show any overlay
                }
            }
        }
    }
}

fn render_status_area(app: &App, frame: &mut Frame, area: Rect) {
    // Build multi-line status text
    let mut status_lines = Vec::new();
    let max_lines = (area.height.saturating_sub(2)) as usize; // Account for borders

    // Always show the current status_message as the first line (if not empty)
    if !app.status_message.is_empty() {
        let truncated_status = if app.status_message.len() > 80 {
            format!("{}...", &app.status_message[..77])
        } else {
            app.status_message.clone()
        };
        status_lines.push(truncated_status);
    }

    // Show buffer/timer status as the second line if we have user info
    if let Some(user_info) = &app.user_info {
        let available_pixels = if let Some(timers) = &user_info.timers {
            user_info.pixel_buffer - timers.len() as i32
        } else {
            user_info.pixel_buffer
        };

        // Use the new formatted timer status instead of the old progress bar format
        if !app.cooldown_status.is_empty() && app.cooldown_status != "Ready to place pixels" {
            status_lines.push(format!("🕐 {}", app.cooldown_status));
        } else if available_pixels > 0 {
            status_lines.push(format!("🟢 {} pixels available", available_pixels));
        } else {
            status_lines.push(format!("🔴 No pixels available"));
        }
    }

    // Add recent status messages (newest first, limit to remaining space)
    let remaining_lines = max_lines.saturating_sub(status_lines.len());
    if remaining_lines > 0 {
        for (message, _timestamp, _utc2_timestamp) in
            app.status_messages.iter().rev().take(remaining_lines)
        {
            // Truncate long messages to prevent overflow
            let truncated_message = if message.len() > 80 {
                format!("{}...", &message[..77])
            } else {
                message.clone()
            };
            status_lines.push(format!("• {}", truncated_message));
        }
    }

    let status_text = status_lines.join("\n");
    let status_widget = Paragraph::new(status_text)
        .wrap(Wrap { trim: true })
        .block(Block::default().borders(Borders::ALL).title("Status"));
    frame.render_widget(status_widget, area);
}

/// Filter meaningful pixels for rendering (same logic as queue processing)
fn filter_meaningful_pixels_for_rendering(
    art: &crate::art::PixelArt,
    colors: &[crate::api_client::ColorInfo],
) -> Vec<crate::art::ArtPixel> {
    let mut meaningful_pixels = Vec::new();
    let mut seen_positions = std::collections::HashSet::new();

    // Define background color IDs that should not be placed
    let mut background_color_ids = std::collections::HashSet::new();
    for color in colors {
        let name_lower = color.name.to_lowercase();
        if name_lower.contains("transparent")
            || name_lower.contains("background")
            || name_lower.contains("empty")
            || name_lower == "none"
            || name_lower.contains("alpha")
        {
            background_color_ids.insert(color.id);
        }
    }

    for pixel in &art.pattern {
        // Skip if this position was already processed (remove duplicates)
        let position = (pixel.x, pixel.y);
        if seen_positions.contains(&position) {
            continue;
        }

        // Skip background/transparent colors
        if background_color_ids.contains(&pixel.color) {
            continue;
        }

        meaningful_pixels.push(pixel.clone());
        seen_positions.insert(position);
    }

    meaningful_pixels
}

fn render_delete_confirmation_dialog(app: &App, frame: &mut Frame) {
    // Create a centered popup
    let popup_area = centered_rect(50, 20, frame.size());

    // Clear the area
    frame.render_widget(Clear, popup_area);

    // Get the art name
    let art_name = if let Some(index) = app.art_to_delete_index {
        app.available_pixel_arts
            .get(index)
            .map(|art| art.name.as_str())
            .unwrap_or("Unknown")
    } else {
        "Unknown"
    };

    // Create the dialog content
    let dialog_text = format!(
        "Delete '{}'?\n\nThis action cannot be undone.\n\n{}   {}",
        art_name,
        if app.delete_confirmation_selection {
            "> Yes <"
        } else {
            "  Yes  "
        },
        if !app.delete_confirmation_selection {
            "> No <"
        } else {
            "  No  "
        }
    );

    let dialog = Paragraph::new(dialog_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Confirm Deletion")
                .border_style(Style::default().fg(Color::Red)),
        )
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });

    frame.render_widget(dialog, popup_area);
}

/// Helper function to create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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

/// Render event timer overlay on top of the canvas
fn render_event_timer_overlay(app: &App, frame: &mut Frame, inner_board_area: &Rect) {
    if let Some(event_start_time) = app.event_start_time {
        // Calculate remaining time until event starts
        if let Ok(duration_until_start) =
            event_start_time.duration_since(std::time::SystemTime::now())
        {
            let seconds_remaining = duration_until_start.as_secs();

            // Format the countdown display
            let timer_text = if seconds_remaining > 3600 {
                let hours = seconds_remaining / 3600;
                let minutes = (seconds_remaining % 3600) / 60;
                let seconds = seconds_remaining % 60;
                if minutes > 0 {
                    format!("⏰ Event starts in {}h {}m {}s", hours, minutes, seconds)
                } else {
                    format!("⏰ Event starts in {}h {}s", hours, seconds)
                }
            } else if seconds_remaining > 60 {
                let minutes = seconds_remaining / 60;
                let seconds = seconds_remaining % 60;
                format!("⏰ Event starts in {}m {}s", minutes, seconds)
            } else {
                format!("⏰ Event starts in {}s", seconds_remaining)
            };

            // Create a small overlay in the top-center of the board area
            let timer_width = (timer_text.len() as u16 + 4).min(inner_board_area.width);
            let timer_area = Rect {
                x: inner_board_area.x + (inner_board_area.width.saturating_sub(timer_width)) / 2,
                y: inner_board_area.y,
                width: timer_width,
                height: 3,
            };

            // Render the timer background
            let timer_block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow))
                .style(Style::default().bg(Color::Black));
            frame.render_widget(timer_block, timer_area);

            // Render the timer text
            let timer_paragraph = Paragraph::new(timer_text)
                .style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
                .alignment(Alignment::Center);

            let inner_timer_area = timer_area.inner(Margin {
                vertical: 1,
                horizontal: 1,
            });
            frame.render_widget(timer_paragraph, inner_timer_area);
        }
    }
}
