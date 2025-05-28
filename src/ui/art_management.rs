use crate::app_state::App;
use crate::ui::helpers::get_ratatui_color;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};

/// Render the art selection UI (full width, no small preview)
pub fn render_art_selection_ui(app: &App, frame: &mut Frame, area: Rect) {
    if app.available_pixel_arts.is_empty() {
        let empty_message = Paragraph::new("No pixel arts available").block(
            Block::default()
                .borders(Borders::ALL)
                .title("Art Selection"),
        );
        frame.render_widget(empty_message, area);
        return;
    }

    // Render art list using full width
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
                art.pattern.len()
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
                .title("Select Pixel Art (Enter to load, Esc to cancel)"),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    let mut list_state = ListState::default();
    list_state.select(Some(app.art_selection_index));

    frame.render_stateful_widget(art_list, area, &mut list_state);
}

/// Render the art queue management UI
pub fn render_art_queue_ui(app: &App, frame: &mut Frame, area: Rect) {
    if app.art_queue.is_empty() {
        let empty_message = Paragraph::new(vec![
            Line::from("Queue is empty"),
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

            let pause_indicator = if item.paused { " ⏸️" } else { "" };

            let item_text = format!(
                "{} P{} '{}' @ ({},{}){}{}",
                status_symbol,
                item.priority,
                item.art.name,
                item.art.board_x,
                item.art.board_y,
                progress,
                pause_indicator
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
        .filter(|item| item.status == crate::app_state::QueueStatus::Pending && !item.paused)
        .count();

    let paused_count = app.art_queue.iter().filter(|item| item.paused).count();

    let total_pixels = app
        .art_queue
        .iter()
        .filter(|item| item.status == crate::app_state::QueueStatus::Pending && !item.paused)
        .map(|item| item.pixels_total)
        .sum::<usize>();

    let mut controls_text = vec![
        Line::from(Span::styled(
            "Queue Statistics",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(format!("Pending: {}", pending_count)),
        Line::from(format!("Paused: {}", paused_count)),
        Line::from(format!("Total Pixels: {}", total_pixels)),
    ];

    // Add hint if selected item is failed
    if !app.art_queue.is_empty() && app.queue_selection_index < app.art_queue.len() {
        let selected_item = &app.art_queue[app.queue_selection_index];
        if selected_item.status == crate::app_state::QueueStatus::Failed {
            controls_text.push(Line::from(""));
            controls_text.push(Line::from(Span::styled(
                "💡 Selected item failed - Press Enter to resume & start",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )));
        }
    }

    controls_text.extend(vec![
        Line::from(""),
        Line::from(Span::styled(
            "Controls",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from("↑/↓  : Navigate queue"),
        Line::from("u/k  : Move item up"),
        Line::from("j/n  : Move item down"),
        Line::from("Enter: Resume failed/Start processing"),
        Line::from("p/s  : Pause/Resume item"),
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
    ]);

    let controls_paragraph = Paragraph::new(controls_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Info & Controls"),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(controls_paragraph, queue_layout[1]);
}

/// Render the share selection UI for viewing and loading shared arts
pub fn render_share_selection_ui(app: &App, frame: &mut Frame, area: Rect) {
    if app.available_shares.is_empty() {
        let empty_message = Paragraph::new(vec![
            Line::from("No shared arts available"),
            Line::from(""),
            Line::from("Shared arts are stored in the 'shares/' directory."),
            Line::from("You can receive shares from other users or create them"),
            Line::from("by sharing your own arts with 'x' key."),
            Line::from(""),
            Line::from("Press Esc to return to main view."),
        ])
        .block(Block::default().borders(Borders::ALL).title("Shared Arts"));
        frame.render_widget(empty_message, area);
        return;
    }

    let share_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(60), // Share list
            Constraint::Percentage(40), // Details
        ])
        .split(area);

    // Render share list on the left
    let share_items: Vec<ListItem> = app
        .available_shares
        .iter()
        .enumerate()
        .map(|(idx, shareable)| {
            let art = &shareable.art;
            let dimensions = crate::art::get_art_dimensions(art);

            let share_info = if let Some(msg) = &shareable.share_message {
                format!(" - {}", msg)
            } else {
                String::new()
            };

            let item_text = format!(
                "{} @ ({}, {}) ({}x{}){}",
                art.name,
                shareable.board_x,
                shareable.board_y,
                dimensions.0,
                dimensions.1,
                share_info
            );

            if idx == app.share_selection_index {
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

    let share_list = List::new(share_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Shared Arts (Enter to load, Esc to cancel)"),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    let mut list_state = ListState::default();
    list_state.select(Some(app.share_selection_index));

    frame.render_stateful_widget(share_list, share_layout[0], &mut list_state);

    // Render details on the right
    if let Some(selected_share) = app.available_shares.get(app.share_selection_index) {
        render_share_details(selected_share, app, frame, share_layout[1]);
    }
}

/// Render details of a selected shared art
fn render_share_details(
    shareable: &crate::art::ShareablePixelArt,
    _app: &App,
    frame: &mut Frame,
    area: Rect,
) {
    let art = &shareable.art;
    let dimensions = crate::art::get_art_dimensions(art);

    let mut details_lines = vec![
        Line::from(Span::styled(
            format!("Art: {}", art.name),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(format!(
            "Position: ({}, {})",
            shareable.board_x, shareable.board_y
        )),
        Line::from(format!(
            "Size: {}x{} ({} pixels)",
            dimensions.0,
            dimensions.1,
            art.pattern.len()
        )),
        Line::from(""),
    ];

    if let Some(description) = &art.description {
        details_lines.push(Line::from(format!("Description: {}", description)));
    }

    if let Some(author) = &art.author {
        details_lines.push(Line::from(format!("Author: {}", author)));
    }

    if let Some(shared_by) = &shareable.shared_by {
        details_lines.push(Line::from(format!("Shared by: {}", shared_by)));
    }

    if let Some(share_message) = &shareable.share_message {
        details_lines.push(Line::from(""));
        details_lines.push(Line::from(Span::styled(
            "Share Message:",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        details_lines.push(Line::from(share_message.clone()));
    }

    details_lines.push(Line::from(""));
    details_lines.push(Line::from(format!(
        "Shared: {}",
        chrono::DateTime::parse_from_rfc3339(&shareable.shared_at)
            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|_| shareable.shared_at.clone())
    )));

    if let Some(tags) = &art.tags {
        if !tags.is_empty() {
            details_lines.push(Line::from(""));
            details_lines.push(Line::from(format!("Tags: {}", tags.join(", "))));
        }
    }

    details_lines.push(Line::from(""));
    details_lines.push(Line::from("Share String:"));
    let share_string = crate::art::generate_share_string(art, shareable.board_x, shareable.board_y);
    details_lines.push(Line::from(Span::styled(
        share_string,
        Style::default().fg(Color::Cyan),
    )));

    let details_paragraph = Paragraph::new(details_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Share Details"),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(details_paragraph, area);
}

/// Render a full-screen art preview for art selection (always visible)
pub fn render_art_preview_fullscreen(
    art: &crate::art::PixelArt,
    app: &App,
    frame: &mut Frame,
    area: Rect,
) {
    // Create a popup that takes most of the screen
    let popup_area = centered_rect(90, 85, area);

    // Clear the background
    frame.render_widget(
        Block::default()
            .style(Style::default().bg(Color::Black))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::White))
            .title(format!(
                "Preview: {} (Enter to load, Esc to cancel)",
                art.name
            )),
        popup_area,
    );

    let inner_area = popup_area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });

    if art.pattern.is_empty() {
        let empty_preview = Paragraph::new("(Empty art)")
            .style(Style::default().fg(Color::Gray))
            .block(Block::default());
        frame.render_widget(empty_preview, inner_area);
        return;
    }

    // Calculate art bounds
    let dimensions = crate::art::get_art_dimensions(art);
    let art_width = dimensions.0 as u16;
    let art_height = dimensions.1 as u16;

    // Scale preview to fit available space, but allow scaling up for small arts
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

    // For full-screen preview, allow scaling up to a reasonable limit
    let scale = scale_x.min(scale_y).min(8.0); // Max 8x scaling

    let preview_width = (art_width as f32 * scale) as u16;
    let preview_height = ((art_height as f32 * scale) / 2.0) as u16; // Divide by 2 for half-blocks

    // First, fill the entire inner area with black background
    for y in 0..inner_area.height {
        for x in 0..inner_area.width {
            if inner_area.x + x < frame.size().width && inner_area.y + y < frame.size().height {
                frame
                    .buffer_mut()
                    .get_mut(inner_area.x + x, inner_area.y + y)
                    .set_char(' ')
                    .set_style(Style::default().bg(Color::Black));
            }
        }
    }

    // Center the preview
    let start_x = inner_area.x + (inner_area.width.saturating_sub(preview_width)) / 2;
    let start_y = inner_area.y + (inner_area.height.saturating_sub(preview_height)) / 2;

    // Render the art preview using half-blocks
    for screen_y in 0..preview_height {
        for screen_x in 0..preview_width {
            let art_pixel_y_top = ((screen_y * 2) as f32 / scale) as i32;
            let art_pixel_y_bottom = art_pixel_y_top + (1.0 / scale) as i32;
            let art_pixel_x = (screen_x as f32 / scale) as i32;

            let top_pixel = art
                .pattern
                .iter()
                .find(|p| p.x == art_pixel_x && p.y == art_pixel_y_top);

            let bottom_pixel = art
                .pattern
                .iter()
                .find(|p| p.x == art_pixel_x && p.y == art_pixel_y_bottom);

            let top_pixel_color = top_pixel
                .map(|p| get_ratatui_color(app, p.color, Color::Black))
                .unwrap_or(Color::Black); // Use Black for empty areas

            let bottom_pixel_color = bottom_pixel
                .map(|p| get_ratatui_color(app, p.color, Color::Black))
                .unwrap_or(Color::Black); // Use Black for empty areas

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

    // Add info text at the bottom
    let info_area = Rect {
        x: popup_area.x + 2,
        y: popup_area.y + popup_area.height - 3,
        width: popup_area.width - 4,
        height: 1,
    };

    let info_text = format!(
        "Size: {}x{} pixels | Scale: {:.1}x | Use ↑↓ to browse, Enter to load",
        art_width, art_height, scale
    );

    frame.render_widget(
        Paragraph::new(info_text)
            .style(Style::default().fg(Color::Cyan))
            .alignment(Alignment::Center),
        info_area,
    );
}

/// Render a full-screen art preview popup
pub fn render_art_preview_ui(app: &App, frame: &mut Frame, area: Rect) {
    if let Some(art) = &app.art_preview_art {
        // Create a popup that takes most of the screen
        let popup_area = centered_rect(90, 85, area);

        // Clear the background
        frame.render_widget(
            Block::default()
                .style(Style::default().bg(Color::Black))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::White))
                .title(format!(
                    "Full Preview: {} (Enter to load, Esc to return)",
                    art.name
                )),
            popup_area,
        );

        let inner_area = popup_area.inner(Margin {
            vertical: 1,
            horizontal: 1,
        });

        if art.pattern.is_empty() {
            let empty_preview = Paragraph::new("(Empty art)")
                .style(Style::default().fg(Color::Gray))
                .block(Block::default());
            frame.render_widget(empty_preview, inner_area);
            return;
        }

        // Calculate art bounds
        let dimensions = crate::art::get_art_dimensions(art);
        let art_width = dimensions.0 as u16;
        let art_height = dimensions.1 as u16;

        // Scale preview to fit available space, but allow scaling up for small arts
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

        // For full-screen preview, allow scaling up to a reasonable limit
        let scale = scale_x.min(scale_y).min(8.0); // Max 8x scaling

        let preview_width = (art_width as f32 * scale) as u16;
        let preview_height = ((art_height as f32 * scale) / 2.0) as u16; // Divide by 2 for half-blocks

        // First, fill the entire inner area with black background
        for y in 0..inner_area.height {
            for x in 0..inner_area.width {
                if inner_area.x + x < frame.size().width && inner_area.y + y < frame.size().height {
                    frame
                        .buffer_mut()
                        .get_mut(inner_area.x + x, inner_area.y + y)
                        .set_char(' ')
                        .set_style(Style::default().bg(Color::Black));
                }
            }
        }

        // Center the preview
        let start_x = inner_area.x + (inner_area.width.saturating_sub(preview_width)) / 2;
        let start_y = inner_area.y + (inner_area.height.saturating_sub(preview_height)) / 2;

        // Render the art preview using half-blocks
        for screen_y in 0..preview_height {
            for screen_x in 0..preview_width {
                let art_pixel_y_top = ((screen_y * 2) as f32 / scale) as i32;
                let art_pixel_y_bottom = art_pixel_y_top + (1.0 / scale) as i32;
                let art_pixel_x = (screen_x as f32 / scale) as i32;

                let top_pixel = art
                    .pattern
                    .iter()
                    .find(|p| p.x == art_pixel_x && p.y == art_pixel_y_top);

                let bottom_pixel = art
                    .pattern
                    .iter()
                    .find(|p| p.x == art_pixel_x && p.y == art_pixel_y_bottom);

                let top_pixel_color = top_pixel
                    .map(|p| get_ratatui_color(app, p.color, Color::Black))
                    .unwrap_or(Color::Black); // Use Black for empty areas

                let bottom_pixel_color = bottom_pixel
                    .map(|p| get_ratatui_color(app, p.color, Color::Black))
                    .unwrap_or(Color::Black); // Use Black for empty areas

                let cell_char = '▀';
                let style = Style::default().fg(top_pixel_color).bg(bottom_pixel_color);

                if start_x + screen_x < frame.size().width
                    && start_y + screen_y < frame.size().height
                {
                    frame
                        .buffer_mut()
                        .get_mut(start_x + screen_x, start_y + screen_y)
                        .set_char(cell_char)
                        .set_style(style);
                }
            }
        }

        // Add info text at the bottom
        let info_area = Rect {
            x: popup_area.x + 2,
            y: popup_area.y + popup_area.height - 3,
            width: popup_area.width - 4,
            height: 1,
        };

        let info_text = format!(
            "Size: {}x{} pixels | Scale: {:.1}x | Controls: Enter=Load, Esc=Return",
            art_width, art_height, scale
        );

        frame.render_widget(
            Paragraph::new(info_text)
                .style(Style::default().fg(Color::Cyan))
                .alignment(Alignment::Center),
            info_area,
        );
    }
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
