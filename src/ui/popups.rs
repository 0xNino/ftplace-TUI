use crate::app_state::App;
use crate::ui::helpers::centered_rect;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

pub fn render_help_popup(_app: &App, frame: &mut Frame) {
    let popup_area = centered_rect(60, 50, frame.size()); // Adjust size as needed

    let help_text = vec![
        Line::from(Span::styled(
            "--- General ---",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(" q: Quit application"),
        Line::from(" ?: Toggle this help screen"),
        Line::from(" c: Configure/Re-enter Access Token"),
        Line::from(" b: Change Base URL"),
        Line::from(" r: Refresh board data"),
        Line::from(" p: Fetch profile data"),
        Line::from(" i: Show user profile panel"),
        Line::from(" h: Show status log history"),
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

pub fn render_profile_popup(app: &App, frame: &mut Frame) {
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

        let available_pixels = if let Some(timers) = &user_info.timers {
            user_info.pixel_buffer - timers.len() as i32
        } else {
            user_info.pixel_buffer
        };
        lines.push(Line::from(vec![
            Span::styled(
                "Available Pixels: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                available_pixels.to_string(),
                Style::default().fg(Color::Green),
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

pub fn render_status_log_popup(app: &App, frame: &mut Frame) {
    let popup_area = centered_rect(80, 70, frame.size());

    let mut log_lines = vec![
        Line::from(Span::styled(
            "--- Status Log History ---",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Cyan),
        )),
        Line::from(""),
    ];

    if app.status_messages.is_empty() {
        log_lines.push(Line::from(Span::styled(
            "No status messages available.",
            Style::default().fg(Color::Gray),
        )));
    } else {
        // Show messages in reverse chronological order (newest first)
        // Use a fixed reference time to avoid "0 seconds ago" issues when popup is reopened
        let reference_time = std::time::Instant::now();
        for (message, timestamp) in app.status_messages.iter().rev() {
            // Format timestamp
            let elapsed = reference_time.duration_since(*timestamp);

            let time_str = if elapsed.as_secs() < 60 {
                format!("{}s ago", elapsed.as_secs())
            } else if elapsed.as_secs() < 3600 {
                format!("{}m ago", elapsed.as_secs() / 60)
            } else {
                format!("{}h ago", elapsed.as_secs() / 3600)
            };

            // Create a line with timestamp and message
            log_lines.push(Line::from(vec![
                Span::styled(
                    format!("[{}] ", time_str),
                    Style::default()
                        .fg(Color::Gray)
                        .add_modifier(Modifier::ITALIC),
                ),
                Span::styled(message, Style::default().fg(Color::White)),
            ]));
        }
    }

    log_lines.push(Line::from(""));
    log_lines.push(Line::from(Span::styled(
        "Press Esc, q, or h to close",
        Style::default()
            .fg(Color::Gray)
            .add_modifier(Modifier::ITALIC),
    )));

    let log_paragraph = Paragraph::new(log_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Status Log History (Press Esc, q, or h to close)"),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(Clear, popup_area);
    frame.render_widget(log_paragraph, popup_area);
}
