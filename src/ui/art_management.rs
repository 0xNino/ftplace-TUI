use crate::app_state::App;
use crate::ui::helpers::get_ratatui_color;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};

/// Render the art selection UI with previews
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

    if art.pattern.is_empty() {
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
                .pattern
                .iter()
                .find(|p| p.x == art_pixel_x && p.y == art_pixel_y_top)
                .map(|p| get_ratatui_color(app, p.color, Color::DarkGray))
                .unwrap_or(Color::DarkGray);

            let bottom_pixel_color = art
                .pattern
                .iter()
                .find(|p| p.x == art_pixel_x && p.y == art_pixel_y_bottom)
                .map(|p| get_ratatui_color(app, p.color, Color::DarkGray))
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
pub fn render_art_queue_ui(app: &App, frame: &mut Frame, area: Rect) {
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

    let controls_text = vec![
        Line::from(Span::styled(
            "Queue Statistics",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(format!("Pending: {}", pending_count)),
        Line::from(format!("Paused: {}", paused_count)),
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
        Line::from("Space: Pause/Resume queue"),
        Line::from("s    : Suspend/Start item"),
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
