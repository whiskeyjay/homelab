use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
};
use unicode_width::UnicodeWidthStr;

use crate::app::{App, Focus, NewMsgFocus, MENU_ITEMS};

pub fn draw(frame: &mut Frame, app: &mut App) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // menu bar
            Constraint::Min(5),   // main content
            Constraint::Length(1), // status bar
        ])
        .split(frame.area());

    draw_menu_bar(frame, app, outer[0]);
    draw_main_panels(frame, app, outer[1]);
    draw_status_bar(frame, app, outer[2]);

    if app.menu_active {
        draw_menu_dropdown(frame, app);
    }

    if app.show_help_popup {
        draw_help_popup(frame);
    }

    if app.confirm_delete {
        draw_confirm_delete(frame, app);
    }

    if app.confirm_delete_message {
        draw_confirm_delete_message(frame, app);
    }

    if app.new_message_dialog.is_some() {
        draw_new_message_dialog(frame, app);
    }

    if app.modem_picker.is_some() {
        draw_modem_picker(frame, app);
    }
}

fn draw_menu_bar(frame: &mut Frame, app: &App, area: Rect) {
    let mut spans = Vec::new();
    spans.push(Span::raw(" "));
    for (i, menu) in MENU_ITEMS.iter().enumerate() {
        let base_style = if app.menu_active && app.menu_selected == i {
            Style::default().bg(Color::White).fg(Color::Black)
        } else {
            Style::default().fg(Color::White).bold()
        };

        let label = menu.label();
        let hotkey = menu.hotkey();

        // Split label to underline the hotkey character
        spans.push(Span::styled(" ", base_style));
        let mut found = false;
        for ch in label.chars() {
            if !found && ch.to_ascii_lowercase() == hotkey {
                spans.push(Span::styled(
                    ch.to_string(),
                    base_style.add_modifier(Modifier::UNDERLINED),
                ));
                found = true;
            } else {
                spans.push(Span::styled(ch.to_string(), base_style));
            }
        }
        spans.push(Span::styled(" ", base_style));
        spans.push(Span::raw("  "));
    }

    let bar = Paragraph::new(Line::from(spans))
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));
    frame.render_widget(bar, area);
}

fn draw_menu_dropdown(frame: &mut Frame, app: &App) {
    let menu = MENU_ITEMS[app.menu_selected];
    let actions = menu.actions();

    // Calculate position under the selected menu item
    let x_offset: u16 = 1
        + MENU_ITEMS[..app.menu_selected]
            .iter()
            .map(|m| m.label().len() as u16 + 4)
            .sum::<u16>();

    let width = actions.iter().map(|a| a.len()).max().unwrap_or(10) as u16 + 4;
    let height = actions.len() as u16 + 2;

    let area = Rect::new(x_offset, 1, width, height);

    frame.render_widget(Clear, area);

    let items: Vec<ListItem> = actions
        .iter()
        .enumerate()
        .map(|(i, action)| {
            if *action == "---" {
                ListItem::new(Span::styled(
                    "─".repeat((width as usize).saturating_sub(2)),
                    Style::default().fg(Color::Gray),
                ))
            } else {
                let style = if i == app.menu_action_selected {
                    Style::default().bg(Color::White).fg(Color::Black)
                } else {
                    Style::default()
                };
                ListItem::new(Span::styled(format!(" {} ", action), style))
            }
        })
        .collect();

    let dropdown = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().bg(Color::DarkGray)),
    );
    frame.render_widget(dropdown, area);
}

fn draw_main_panels(frame: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40), // conversation list
            Constraint::Percentage(60), // message view
        ])
        .split(area);

    draw_conversation_list(frame, app, chunks[0]);
    draw_message_panel(frame, app, chunks[1]);
}

fn draw_conversation_list(frame: &mut Frame, app: &App, area: Rect) {
    let border_style = if app.focus == Focus::ConversationList {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let block = Block::default()
        .title(" Conversations ")
        .borders(Borders::ALL)
        .border_style(border_style);

    if app.conversations.is_empty() {
        let empty = Paragraph::new("  No messages found.\n  Press 'r' to refresh.")
            .block(block)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(empty, area);
        return;
    }

    let items: Vec<ListItem> = app
        .conversations
        .iter()
        .enumerate()
        .map(|(i, convo)| {
            let selected = i == app.selected_conversation;
            let marker = if selected { "▸ " } else { "  " };

            let preview = truncate_str(convo.last_message_preview(), (area.width as usize).saturating_sub(6));

            let style = if selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let preview_style = if selected {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let lines = vec![
                Line::from(Span::styled(
                    format!("{}{}", marker, convo.number),
                    style,
                )),
                Line::from(Span::styled(
                    format!("  {}", preview),
                    preview_style,
                )),
            ];
            ListItem::new(Text::from(lines))
        })
        .collect();

    let list = List::new(items).block(block);
    let mut state = ratatui::widgets::ListState::default().with_selected(Some(app.selected_conversation));
    frame.render_stateful_widget(list, area, &mut state);

    // Scrollbar (only when content overflows)
    let inner_height = area.height.saturating_sub(2) as usize;
    let total_items = app.conversations.len();
    let items_visible = inner_height / 2; // each item is 2 lines
    if total_items > items_visible {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None);
        let mut scrollbar_state = ScrollbarState::new(total_items.saturating_sub(items_visible))
            .position(state.offset());
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}

fn draw_message_panel(frame: &mut Frame, app: &mut App, area: Rect) {
    let convo = match app.selected_convo() {
        Some(c) => c.clone(),
        None => {
            let block = Block::default()
                .title(" Messages ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray));
            let hint = Paragraph::new("\n  Select a conversation to view messages.")
                .block(block)
                .style(Style::default().fg(Color::DarkGray));
            frame.render_widget(hint, area);
            return;
        }
    };

    let title = format!(" {} ", convo.number);

    // Calculate reply box height based on content (min 3, max 10)
    let input_line_count = if app.input_buffer.is_empty() {
        1
    } else {
        app.input_buffer.lines().count().max(1)
            + if app.input_buffer.ends_with('\n') { 1 } else { 0 }
    };
    let reply_height = (input_line_count as u16 + 2).clamp(3, 10); // +2 for borders

    // Split: messages area + reply input
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),          // messages
            Constraint::Length(reply_height), // reply box
        ])
        .split(area);

    // Messages area
    let msg_border_style = if app.focus == Focus::MessageView {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let msg_block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(msg_border_style);

    app.message_area_height = chunks[0].height;

    let inner_height = chunks[0].height.saturating_sub(2) as usize;
    let inner_width = chunks[0].width.saturating_sub(2) as usize;

    let mut lines: Vec<Line> = Vec::new();
    let in_message_view = app.focus == Focus::MessageView;
    let mut selected_wrapped_start: usize = 0;
    let mut selected_wrapped_end: usize = 0;
    // Track the Line index where each message starts
    let mut msg_line_ranges: Vec<(usize, usize)> = Vec::new();

    for (msg_idx, msg) in convo.messages.iter().enumerate() {
        let is_selected = in_message_view && msg_idx == app.selected_message;
        let msg_start = lines.len();
        let direction_label = if msg.state.is_outgoing() {
            "You"
        } else {
            &convo.number
        };

        let ts = if msg.timestamp.len() > 16 {
            &msg.timestamp[..16]
        } else {
            &msg.timestamp
        };

        let header_style = if is_selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else if msg.state.is_outgoing() {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::Blue)
        };

        let body_style = if is_selected {
            Style::default().bg(Color::DarkGray)
        } else {
            Style::default()
        };

        let marker = if is_selected { "▸" } else { " " };

        lines.push(Line::from(Span::styled(
            format!("{} [{}] {}:", marker, ts, direction_label),
            header_style,
        )));

        for text_line in msg.text.lines() {
            lines.push(Line::from(Span::styled(
                format!("  {}", text_line),
                body_style,
            )));
        }
        lines.push(Line::from(""));
        msg_line_ranges.push((msg_start, lines.len()));
    }

    // Use ratatui's own line_count to get accurate wrapped total
    let total_wrapped = Paragraph::new(Text::from(lines.clone()))
        .wrap(Wrap { trim: false })
        .line_count(inner_width as u16);

    // Compute wrapped line positions for each message
    if in_message_view && !msg_line_ranges.is_empty() {
        let sel = app.selected_message.min(msg_line_ranges.len() - 1);
        let (sel_start_line, sel_end_line) = msg_line_ranges[sel];

        // Count wrapped lines before the selected message
        if sel_start_line > 0 {
            selected_wrapped_start = Paragraph::new(Text::from(lines[..sel_start_line].to_vec()))
                .wrap(Wrap { trim: false })
                .line_count(inner_width as u16);
        }
        // Count wrapped lines up to end of selected message
        selected_wrapped_end = Paragraph::new(Text::from(lines[..sel_end_line].to_vec()))
            .wrap(Wrap { trim: false })
            .line_count(inner_width as u16);
    }

    // Auto-scroll to keep selected message visible
    let max_scroll = total_wrapped.saturating_sub(inner_height) as u16;
    if in_message_view {
        // If selected message is below the visible area, scroll down
        if selected_wrapped_end > app.scroll_offset as usize + inner_height {
            app.scroll_offset = (selected_wrapped_end - inner_height) as u16;
        }
        // If selected message is above the visible area, scroll up
        if (selected_wrapped_start as u16) < app.scroll_offset {
            app.scroll_offset = selected_wrapped_start as u16;
        }
    }
    let scroll = app.scroll_offset.min(max_scroll);

    let messages = Paragraph::new(Text::from(lines))
        .block(msg_block)
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    frame.render_widget(messages, chunks[0]);

    // Message view scrollbar (only when content overflows)
    if total_wrapped > inner_height {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None);
        let mut scrollbar_state = ScrollbarState::new(total_wrapped.saturating_sub(inner_height))
            .position(scroll as usize);
        frame.render_stateful_widget(scrollbar, chunks[0], &mut scrollbar_state);
    }

    // Reply input
    let reply_border_style = if app.focus == Focus::ReplyInput {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let sms_info = sms_segment_info(&app.input_buffer);
    let reply_block = Block::default()
        .title(" Reply (Ctrl+Enter to send) ")
        .title_bottom(Line::from(sms_info).right_aligned())
        .borders(Borders::ALL)
        .border_style(reply_border_style);

    let input_text = if app.input_buffer.is_empty() && app.focus != Focus::ReplyInput {
        Text::styled(
            "Type a message...",
            Style::default().fg(Color::DarkGray).italic(),
        )
    } else {
        Text::raw(&app.input_buffer)
    };

    // Scroll the reply input so the cursor line is always visible
    let inner_reply_height = chunks[1].height.saturating_sub(2) as usize; // subtract borders
    let inner_reply_width = chunks[1].width.saturating_sub(2) as usize;
    let (cursor_row, cursor_col) = cursor_position_wrapped(&app.input_buffer, app.input_cursor, inner_reply_width);
    let input_scroll = if cursor_row >= inner_reply_height {
        (cursor_row - inner_reply_height + 1) as u16
    } else {
        0
    };

    let reply_paragraph = Paragraph::new(input_text.clone())
        .wrap(Wrap { trim: false });
    let total_reply_lines = reply_paragraph.line_count(inner_reply_width as u16);

    let reply = Paragraph::new(input_text)
        .block(reply_block)
        .wrap(Wrap { trim: false })
        .scroll((input_scroll, 0));
    frame.render_widget(reply, chunks[1]);

    // Reply scrollbar (only when content overflows)
    if total_reply_lines > inner_reply_height {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None);
        let mut scrollbar_state = ScrollbarState::new(total_reply_lines.saturating_sub(inner_reply_height))
            .position(input_scroll as usize);
        frame.render_stateful_widget(scrollbar, chunks[1], &mut scrollbar_state);
    }

    // Show cursor in reply input when focused
    if app.focus == Focus::ReplyInput {
        let visible_row = (cursor_row as u16).saturating_sub(input_scroll);
        let cursor_x = chunks[1].x + 1 + cursor_col as u16;
        let cursor_y = chunks[1].y + 1 + visible_row;
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let focus_label = match app.focus {
        Focus::ConversationList => "List",
        Focus::MessageView => "Messages",
        Focus::ReplyInput => "Reply",
    };

    let left = Span::styled(
        format!(" {} ", app.status_message),
        Style::default().fg(Color::White),
    );
    let right = Span::styled(
        format!(" {} | Tab: switch | F10: menu | q: quit ", focus_label),
        Style::default().fg(Color::DarkGray),
    );

    let available = area.width as usize;
    let right_len = right.width();
    let left_len = available.saturating_sub(right_len);

    let line = Line::from(vec![
        Span::styled(
            format!("{:<width$}", left.content, width = left_len),
            left.style,
        ),
        right,
    ]);

    let bar = Paragraph::new(line).style(Style::default().bg(Color::DarkGray));
    frame.render_widget(bar, area);
}

fn draw_help_popup(frame: &mut Frame) {
    let area = centered_rect(50, 60, frame.area());
    frame.render_widget(Clear, area);

    let help_text = vec![
        Line::from(Span::styled(
            " Key Bindings",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(" Tab / Shift+Tab    Switch focus"),
        Line::from(" Up/Down or j/k     Navigate / scroll"),
        Line::from(" Enter              Select / send message"),
        Line::from(" Esc                Go back"),
        Line::from(" r                  Refresh messages"),
        Line::from(" n                  New message"),
        Line::from(" d                  Delete (conversation/message)"),
        Line::from(" F10                Open menu"),
        Line::from(" Ctrl+C / q         Quit"),
        Line::from(""),
        Line::from(Span::styled(
            " Press any key to close",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let popup = Paragraph::new(Text::from(help_text)).block(
        Block::default()
            .title(" Help ")
            .borders(Borders::ALL)
            .style(Style::default().bg(Color::Black)),
    );
    frame.render_widget(popup, area);
}

fn draw_confirm_delete(frame: &mut Frame, app: &App) {
    let area = centered_rect(40, 20, frame.area());
    frame.render_widget(Clear, area);

    let number = app
        .selected_convo()
        .map(|c| c.number.as_str())
        .unwrap_or("?");

    let text = vec![
        Line::from(""),
        Line::from(format!("  Delete all messages with {}?", number)),
        Line::from(""),
        Line::from(Span::styled(
            "  [Y]es / [N]o",
            Style::default().fg(Color::White).bold(),
        )),
    ];

    let popup = Paragraph::new(Text::from(text)).block(
        Block::default()
            .title(" Confirm Delete ")
            .borders(Borders::ALL)
            .style(Style::default().bg(Color::Red).fg(Color::White)),
    );
    frame.render_widget(popup, area);
}

fn draw_confirm_delete_message(frame: &mut Frame, app: &App) {
    let area = centered_rect(50, 20, frame.area());
    frame.render_widget(Clear, area);

    let preview = app
        .selected_convo()
        .and_then(|c| c.messages.get(app.selected_message))
        .map(|m| truncate_str(&m.text.replace('\n', " "), 40))
        .unwrap_or_else(|| "?".to_string());

    let text = vec![
        Line::from(""),
        Line::from(format!("  Delete this message?")),
        Line::from(Span::styled(
            format!("  \"{}\"", preview),
            Style::default().fg(Color::White).italic(),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  [Y]es / [N]o",
            Style::default().fg(Color::White).bold(),
        )),
    ];

    let popup = Paragraph::new(Text::from(text)).block(
        Block::default()
            .title(" Confirm Delete Message ")
            .borders(Borders::ALL)
            .style(Style::default().bg(Color::Red).fg(Color::White)),
    );
    frame.render_widget(popup, area);
}

fn draw_new_message_dialog(frame: &mut Frame, app: &App) {
    let dlg = match &app.new_message_dialog {
        Some(d) => d,
        None => return,
    };

    let area = centered_rect(60, 60, frame.area());
    frame.render_widget(Clear, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // number field
            Constraint::Min(5),   // body field
            Constraint::Length(1), // hint line
        ])
        .split(area);

    // Outer border
    let outer_block = Block::default()
        .title(" New Message (Ctrl+Enter to send, Esc to cancel) ")
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black));
    frame.render_widget(outer_block, area);

    // Number field
    let number_border = if dlg.focus == NewMsgFocus::Number {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let number_block = Block::default()
        .title(" To ")
        .borders(Borders::ALL)
        .border_style(number_border);

    let number_text = if dlg.number.is_empty() && dlg.focus != NewMsgFocus::Number {
        Text::styled(
            "Phone number...",
            Style::default().fg(Color::DarkGray).italic(),
        )
    } else {
        Text::raw(&dlg.number)
    };
    let number_widget = Paragraph::new(number_text).block(number_block);
    frame.render_widget(number_widget, chunks[0]);

    // Body field
    let body_border = if dlg.focus == NewMsgFocus::Body {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let body_sms_info = sms_segment_info(&dlg.body);
    let body_block = Block::default()
        .title(" Message ")
        .title_bottom(Line::from(body_sms_info).right_aligned())
        .borders(Borders::ALL)
        .border_style(body_border);

    let body_text = if dlg.body.is_empty() && dlg.focus != NewMsgFocus::Body {
        Text::styled(
            "Type your message...",
            Style::default().fg(Color::DarkGray).italic(),
        )
    } else {
        Text::raw(&dlg.body)
    };

    let inner_body_height = chunks[1].height.saturating_sub(2) as usize;
    let inner_body_width = chunks[1].width.saturating_sub(2) as usize;
    let (body_cursor_row, _) = cursor_position_wrapped(&dlg.body, dlg.body_cursor, inner_body_width);
    let body_scroll = if body_cursor_row >= inner_body_height {
        (body_cursor_row - inner_body_height + 1) as u16
    } else {
        0
    };

    let body_paragraph = Paragraph::new(body_text.clone())
        .wrap(Wrap { trim: false });
    let total_body_lines = body_paragraph.line_count(inner_body_width as u16);

    let body_widget = Paragraph::new(body_text)
        .block(body_block)
        .wrap(Wrap { trim: false })
        .scroll((body_scroll, 0));
    frame.render_widget(body_widget, chunks[1]);

    // Body scrollbar (only when content overflows)
    if total_body_lines > inner_body_height {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None);
        let mut scrollbar_state = ScrollbarState::new(total_body_lines.saturating_sub(inner_body_height))
            .position(body_scroll as usize);
        frame.render_stateful_widget(scrollbar, chunks[1], &mut scrollbar_state);
    }

    // Hint
    let hint = Paragraph::new(Line::from(Span::styled(
        " Tab: switch field | Ctrl+Enter: send | Esc: cancel",
        Style::default().fg(Color::DarkGray),
    )));
    frame.render_widget(hint, chunks[2]);

    // Cursor
    match dlg.focus {
        NewMsgFocus::Number => {
            let cx = chunks[0].x + 1 + display_width_up_to(&dlg.number, dlg.number_cursor) as u16;
            let cy = chunks[0].y + 1;
            frame.set_cursor_position((cx, cy));
        }
        NewMsgFocus::Body => {
            let (row, col) = cursor_position_wrapped(&dlg.body, dlg.body_cursor, inner_body_width);
            let visible_row = (row as u16).saturating_sub(body_scroll);
            let cx = chunks[1].x + 1 + col as u16;
            let cy = chunks[1].y + 1 + visible_row;
            frame.set_cursor_position((cx, cy));
        }
    }
}

fn draw_modem_picker(frame: &mut Frame, app: &App) {
    let picker = match &app.modem_picker {
        Some(p) => p,
        None => return,
    };

    // Each modem entry takes 3 lines (model, number, state) + 1 blank
    let height = (picker.modems.len() as u16 * 4 + 3).min(20);
    let width = 50u16;
    let area = frame.area();
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let rect = Rect::new(x, y, width, height);

    frame.render_widget(Clear, rect);

    let items: Vec<ListItem> = picker
        .modems
        .iter()
        .enumerate()
        .map(|(i, modem)| {
            let is_current = modem.index == app.modem_index;
            let marker = if is_current { "●" } else { " " };

            let model_line = if modem.model.is_empty() {
                format!(" {} Modem {}", marker, modem.index)
            } else if modem.manufacturer.is_empty() {
                format!(" {} Modem {} - {}", marker, modem.index, modem.model)
            } else {
                format!(
                    " {} Modem {} - {} {}",
                    marker, modem.index, modem.manufacturer, modem.model
                )
            };

            let number_display = if modem.own_number.is_empty() {
                "unknown".to_string()
            } else {
                modem.own_number.clone()
            };
            let detail_line = format!("     {} | {}", number_display, modem.state);

            let style = if i == picker.selected {
                Style::default().bg(Color::White).fg(Color::Black)
            } else if is_current {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            };

            let detail_style = if i == picker.selected {
                Style::default().bg(Color::White).fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            ListItem::new(Text::from(vec![
                Line::from(Span::styled(model_line, style)),
                Line::from(Span::styled(detail_line, detail_style)),
                Line::from(""),
            ]))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title(" Select Modem ")
            .borders(Borders::ALL)
            .style(Style::default().bg(Color::Black)),
    );
    frame.render_widget(list, rect);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

fn cursor_position_in_text(text: &str, byte_cursor: usize) -> (usize, usize) {
    let before = &text[..byte_cursor.min(text.len())];
    let row = before.matches('\n').count();
    let last_line = before.rsplit('\n').next().unwrap_or("");
    let col = UnicodeWidthStr::width(last_line);
    (row, col)
}

/// Compute wrapped visual (row, col) for a cursor position, given inner widget width.
fn cursor_position_wrapped(text: &str, byte_cursor: usize, width: usize) -> (usize, usize) {
    if width == 0 {
        return cursor_position_in_text(text, byte_cursor);
    }
    let before = &text[..byte_cursor.min(text.len())];
    let mut visual_row: usize = 0;

    for (i, logical_line) in text.split('\n').enumerate() {
        let line_start = if i == 0 {
            0
        } else {
            text[..].match_indices('\n').nth(i - 1).map(|(idx, _)| idx + 1).unwrap_or(0)
        };
        let line_end = line_start + logical_line.len();

        if byte_cursor >= line_start && byte_cursor <= line_end {
            // Cursor is on this logical line
            let before_on_line = &text[line_start..byte_cursor.min(line_end)];
            let col_w = UnicodeWidthStr::width(before_on_line);
            let wrap_row = col_w / width;
            let wrap_col = col_w % width;
            return (visual_row + wrap_row, wrap_col);
        }

        // Count how many visual rows this logical line takes
        let line_w = UnicodeWidthStr::width(logical_line);
        if line_w == 0 {
            visual_row += 1;
        } else {
            visual_row += (line_w + width - 1) / width;
        }
    }

    // Fallback
    let _ = before;
    (visual_row, 0)
}

/// Get the display width of text up to a byte offset (single-line).
fn display_width_up_to(text: &str, byte_cursor: usize) -> usize {
    let before = &text[..byte_cursor.min(text.len())];
    UnicodeWidthStr::width(before)
}

fn truncate_str(s: &str, max_len: usize) -> String {
    let char_count: usize = s.chars().count();
    if char_count <= max_len {
        s.to_string()
    } else if max_len > 3 {
        let truncated: String = s.chars().take(max_len - 3).collect();
        format!("{}...", truncated)
    } else {
        s.chars().take(max_len).collect()
    }
}

/// Returns true if all characters are in the GSM 7-bit default alphabet.
fn is_gsm7(text: &str) -> bool {
    const GSM7_CHARS: &str = "@£$¥èéùìòÇ\nØø\rÅåΔ_ΦΓΛΩΠΨΣΘΞ \x1b\
        !\"#¤%&'()*+,-./0123456789:;<=>?\
        ¡ABCDEFGHIJKLMNOPQRSTUVWXYZ\
        ÄÖÑÜabcdefghijklmnopqrstuvwxyz\
        äöñüà§";
    // Extended GSM7 characters (each takes 2 code points)
    const GSM7_EXT: &str = "^{}\\[~]|€";
    text.chars().all(|c| GSM7_CHARS.contains(c) || GSM7_EXT.contains(c))
}

/// Compute character count and segment info for SMS display.
fn sms_segment_info(text: &str) -> Span<'static> {
    let text = text.trim();
    if text.is_empty() {
        return Span::styled(" 0/160 (1) ", Style::default().fg(Color::DarkGray));
    }

    let gsm7 = is_gsm7(text);
    let char_count = text.chars().count();

    // GSM7 extended chars count as 2
    let effective_len = if gsm7 {
        text.chars()
            .map(|c| if "^{}\\[~]|€".contains(c) { 2 } else { 1 })
            .sum::<usize>()
    } else {
        char_count
    };

    let (single_limit, multi_limit) = if gsm7 { (160, 153) } else { (70, 67) };

    let segments = if effective_len <= single_limit {
        1
    } else {
        (effective_len + multi_limit - 1) / multi_limit
    };

    let encoding = if gsm7 { "GSM" } else { "UCS-2" };
    let remaining = if segments == 1 {
        single_limit - effective_len
    } else {
        (segments * multi_limit) - effective_len
    };
    let label = format!(" {} left | {} seg | {} ", remaining, segments, encoding);

    let color = if segments > 1 { Color::Yellow } else { Color::DarkGray };
    Span::styled(label, Style::default().fg(color))
}
