use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::DefaultTerminal;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

const AUTO_REFRESH_INTERVAL: Duration = Duration::from_secs(5);

use crate::mmcli::{self, ModemInfo, SmsMessage};
use crate::ui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    ConversationList,
    MessageView,
    ReplyInput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuId {
    File,
    Messages,
    Help,
}

pub static MENU_ITEMS: &[MenuId] = &[MenuId::File, MenuId::Messages, MenuId::Help];

impl MenuId {
    pub fn label(self) -> &'static str {
        match self {
            Self::File => "File",
            Self::Messages => "Messages",
            Self::Help => "Help",
        }
    }

    pub fn hotkey(self) -> char {
        match self {
            Self::File => 'f',
            Self::Messages => 'm',
            Self::Help => 'h',
        }
    }

    pub fn actions(self) -> &'static [&'static str] {
        match self {
            Self::File => &["Select Modem...", "---", "Exit"],
            Self::Messages => &[
                "Refresh",
                "---",
                "New Message",
                "---",
                "Delete Message",
                "Delete Conversation",
            ],
            Self::Help => &["Key bindings", "About"],
        }
    }
}

#[derive(Debug, Clone)]
pub struct Conversation {
    pub number: String,
    pub messages: Vec<SmsMessage>,
}

impl Conversation {
    pub fn last_message_preview(&self) -> &str {
        self.messages.last().map(|m| m.text.as_str()).unwrap_or("")
    }

    pub fn last_timestamp(&self) -> &str {
        self.messages
            .last()
            .map(|m| m.timestamp.as_str())
            .unwrap_or("")
    }
}

pub struct App {
    pub running: bool,
    pub modem_index: u32,
    pub conversations: Vec<Conversation>,
    pub selected_conversation: usize,
    pub scroll_offset: u16,
    pub input_buffer: String,
    pub input_cursor: usize,
    pub focus: Focus,
    pub menu_active: bool,
    pub menu_selected: usize,
    pub menu_action_selected: usize,
    pub status_message: String,
    pub show_help_popup: bool,
    pub confirm_delete: bool,
    pub confirm_delete_message: bool,
    pub selected_message: usize,
    pub message_area_height: u16,
    pub new_message_dialog: Option<NewMessageDialog>,
    pub modem_picker: Option<ModemPicker>,
    pub last_refresh: Instant,
}

#[derive(Debug, Clone)]
pub struct ModemPicker {
    pub modems: Vec<ModemInfo>,
    pub selected: usize,
}

#[derive(Debug, Clone)]
pub struct NewMessageDialog {
    pub number: String,
    pub number_cursor: usize,
    pub body: String,
    pub body_cursor: usize,
    pub focus: NewMsgFocus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewMsgFocus {
    Number,
    Body,
}

impl App {
    pub fn new(modem_index: u32) -> Self {
        Self {
            running: true,
            modem_index,
            conversations: Vec::new(),
            selected_conversation: 0,
            scroll_offset: 0,
            input_buffer: String::new(),
            input_cursor: 0,
            focus: Focus::ConversationList,
            menu_active: false,
            menu_selected: 0,
            menu_action_selected: 0,
            status_message: String::from("Loading..."),
            show_help_popup: false,
            confirm_delete: false,
            confirm_delete_message: false,
            selected_message: 0,
            message_area_height: 20,
            new_message_dialog: None,
            modem_picker: None,
            last_refresh: Instant::now(),
        }
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        self.refresh_messages();

        while self.running {
            terminal.draw(|frame| ui::draw(frame, self))?;

            if event::poll(Duration::from_millis(250))? {
                if let Event::Key(key) = event::read()? {
                    self.handle_key(key);
                }
            }

            // Auto-refresh when no dialog/popup is active
            if self.last_refresh.elapsed() >= AUTO_REFRESH_INTERVAL
                && !self.menu_active
                && !self.show_help_popup
                && !self.confirm_delete
                && !self.confirm_delete_message
                && self.new_message_dialog.is_none()
                && self.modem_picker.is_none()
            {
                self.refresh_messages();
            }
        }

        Ok(())
    }

    pub fn refresh_messages(&mut self) {
        self.last_refresh = Instant::now();
        match self.load_messages() {
            Ok(()) => {
                let count: usize = self.conversations.iter().map(|c| c.messages.len()).sum();
                self.status_message = format!(
                    "Modem {} | {} conversations | {} messages",
                    self.modem_index,
                    self.conversations.len(),
                    count,
                );
            }
            Err(e) => {
                self.status_message = format!("Error: {}", e);
            }
        }
    }

    fn load_messages(&mut self) -> Result<()> {
        let sms_entries = mmcli::list_sms(self.modem_index)?;

        let mut messages = Vec::new();
        for (index, _state) in &sms_entries {
            match mmcli::get_sms(*index) {
                Ok(msg) => messages.push(msg),
                Err(e) => {
                    log::warn!("Failed to load SMS {}: {}", index, e);
                }
            }
        }

        // Group by phone number into conversations
        let mut by_number: BTreeMap<String, Vec<SmsMessage>> = BTreeMap::new();
        for msg in messages {
            by_number.entry(msg.number.clone()).or_default().push(msg);
        }

        // Sort messages within each conversation by timestamp
        self.conversations = by_number
            .into_iter()
            .map(|(number, mut msgs)| {
                msgs.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
                Conversation {
                    number,
                    messages: msgs,
                }
            })
            .collect();

        // Sort conversations by last message timestamp (most recent first)
        self.conversations
            .sort_by(|a, b| b.last_timestamp().cmp(a.last_timestamp()));

        // Clamp selection
        if !self.conversations.is_empty() && self.selected_conversation >= self.conversations.len()
        {
            self.selected_conversation = self.conversations.len() - 1;
        }

        Ok(())
    }

    pub fn selected_convo(&self) -> Option<&Conversation> {
        self.conversations.get(self.selected_conversation)
    }

    fn handle_key(&mut self, key: KeyEvent) {
        // Global keys
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.running = false;
            return;
        }

        if self.show_help_popup {
            self.show_help_popup = false;
            return;
        }

        if self.confirm_delete {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.do_delete_conversation();
                    self.confirm_delete = false;
                }
                _ => self.confirm_delete = false,
            }
            return;
        }

        if self.confirm_delete_message {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.do_delete_message();
                    self.confirm_delete_message = false;
                }
                _ => self.confirm_delete_message = false,
            }
            return;
        }

        if self.modem_picker.is_some() {
            self.handle_modem_picker_key(key);
            return;
        }

        if self.new_message_dialog.is_some() {
            self.handle_new_message_key(key);
            return;
        }

        if self.menu_active {
            self.handle_menu_key(key);
            return;
        }

        match key.code {
            KeyCode::F(10) => {
                self.menu_active = true;
                self.menu_selected = 0;
                self.menu_action_selected = 0;
            }
            KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::ALT) => {
                let lower = c.to_ascii_lowercase();
                if let Some(idx) = MENU_ITEMS.iter().position(|m| m.hotkey() == lower) {
                    self.menu_active = true;
                    self.menu_selected = idx;
                    self.menu_action_selected = 0;
                }
            }
            KeyCode::Tab => self.cycle_focus(),
            KeyCode::BackTab => self.cycle_focus_back(),
            _ => match self.focus {
                Focus::ConversationList => self.handle_conversation_list_key(key),
                Focus::MessageView => self.handle_message_view_key(key),
                Focus::ReplyInput => self.handle_reply_input_key(key),
            },
        }
    }

    fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::ConversationList if !self.conversations.is_empty() => Focus::MessageView,
            Focus::MessageView => Focus::ReplyInput,
            Focus::ReplyInput => Focus::ConversationList,
            _ => Focus::ConversationList,
        };
    }

    fn cycle_focus_back(&mut self) {
        self.focus = match self.focus {
            Focus::ConversationList => Focus::ReplyInput,
            Focus::MessageView => Focus::ConversationList,
            Focus::ReplyInput => Focus::MessageView,
        };
        if self.focus != Focus::ConversationList && self.conversations.is_empty() {
            self.focus = Focus::ConversationList;
        }
    }

    fn handle_menu_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::F(10) => self.menu_active = false,
            KeyCode::Left => {
                if self.menu_selected > 0 {
                    self.menu_selected -= 1;
                } else {
                    self.menu_selected = MENU_ITEMS.len() - 1;
                }
                self.menu_action_selected = 0;
            }
            KeyCode::Right => {
                self.menu_selected = (self.menu_selected + 1) % MENU_ITEMS.len();
                self.menu_action_selected = 0;
            }
            KeyCode::Up => {
                let actions = MENU_ITEMS[self.menu_selected].actions();
                loop {
                    if self.menu_action_selected == 0 {
                        break;
                    }
                    self.menu_action_selected -= 1;
                    if actions[self.menu_action_selected] != "---" {
                        break;
                    }
                }
            }
            KeyCode::Down => {
                let actions = MENU_ITEMS[self.menu_selected].actions();
                let max = actions.len().saturating_sub(1);
                loop {
                    if self.menu_action_selected >= max {
                        break;
                    }
                    self.menu_action_selected += 1;
                    if actions[self.menu_action_selected] != "---" {
                        break;
                    }
                }
            }
            KeyCode::Enter => {
                let actions = MENU_ITEMS[self.menu_selected].actions();
                if actions[self.menu_action_selected] != "---" {
                    self.execute_menu_action();
                    self.menu_active = false;
                }
            }
            _ => {}
        }
    }

    fn execute_menu_action(&mut self) {
        let menu = MENU_ITEMS[self.menu_selected];
        match (menu, self.menu_action_selected) {
            (MenuId::File, 0) => self.open_modem_picker(),    // Select Modem...
            (MenuId::File, 2) => self.running = false,        // Exit
            (MenuId::Messages, 0) => self.refresh_messages(),  // Refresh
            (MenuId::Messages, 2) => self.open_new_message(), // New Message
            (MenuId::Messages, 4) => self.confirm_delete_message = true, // Delete Message
            (MenuId::Messages, 5) => self.confirm_delete = true, // Delete Conversation
            (MenuId::Help, 0) => self.show_help_popup = true, // Key bindings
            (MenuId::Help, 1) => self.show_help_popup = true, // About
            _ => {}
        }
    }

    fn handle_conversation_list_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') => self.running = false,
            KeyCode::Char('r') => self.refresh_messages(),
            KeyCode::Char('n') => self.open_new_message(),
            KeyCode::Char('d') => self.confirm_delete = true,
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_conversation > 0 {
                    self.selected_conversation -= 1;
                    self.scroll_offset = 0;
                    self.selected_message = 0;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected_conversation + 1 < self.conversations.len() {
                    self.selected_conversation += 1;
                    self.scroll_offset = 0;
                    self.selected_message = 0;
                }
            }
            KeyCode::Enter => {
                if !self.conversations.is_empty() {
                    self.focus = Focus::MessageView;
                    self.scroll_offset = 0;
                    self.selected_message = 0;
                }
            }
            _ => {}
        }
    }

    fn handle_message_view_key(&mut self, key: KeyEvent) {
        let msg_count = self.selected_convo().map(|c| c.messages.len()).unwrap_or(0);

        match key.code {
            KeyCode::Char('q') => self.running = false,
            KeyCode::Char('r') => self.refresh_messages(),
            KeyCode::Char('d') => self.confirm_delete_message = true,
            KeyCode::Esc => self.focus = Focus::ConversationList,
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_message > 0 {
                    self.selected_message -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if msg_count > 0 && self.selected_message + 1 < msg_count {
                    self.selected_message += 1;
                }
            }
            KeyCode::PageUp => {
                let page = self.message_area_height.saturating_sub(2);
                self.scroll_offset = self.scroll_offset.saturating_sub(page);
            }
            KeyCode::PageDown => {
                let page = self.message_area_height.saturating_sub(2);
                self.scroll_offset = self.scroll_offset.saturating_add(page);
            }
            KeyCode::Home => {
                self.selected_message = 0;
            }
            KeyCode::End => {
                if msg_count > 0 {
                    self.selected_message = msg_count - 1;
                }
            }
            KeyCode::Enter => {
                self.focus = Focus::ReplyInput;
            }
            _ => {}
        }
    }

    fn handle_reply_input_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.focus = Focus::MessageView,
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::CONTROL) => self.send_reply(),
            KeyCode::Enter => {
                self.input_buffer.insert(self.input_cursor, '\n');
                self.input_cursor += 1;
            }
            KeyCode::Char(c) => {
                self.input_buffer.insert(self.input_cursor, c);
                self.input_cursor += c.len_utf8();
            }
            KeyCode::Backspace => {
                if self.input_cursor > 0 {
                    let prev = self.input_buffer[..self.input_cursor]
                        .chars()
                        .last()
                        .map(|c| c.len_utf8())
                        .unwrap_or(0);
                    self.input_cursor -= prev;
                    self.input_buffer.remove(self.input_cursor);
                }
            }
            KeyCode::Delete => {
                if self.input_cursor < self.input_buffer.len() {
                    self.input_buffer.remove(self.input_cursor);
                }
            }
            KeyCode::Left => {
                if self.input_cursor > 0 {
                    let prev = self.input_buffer[..self.input_cursor]
                        .chars()
                        .last()
                        .map(|c| c.len_utf8())
                        .unwrap_or(0);
                    self.input_cursor -= prev;
                }
            }
            KeyCode::Right => {
                if self.input_cursor < self.input_buffer.len() {
                    let next = self.input_buffer[self.input_cursor..]
                        .chars()
                        .next()
                        .map(|c| c.len_utf8())
                        .unwrap_or(0);
                    self.input_cursor += next;
                }
            }
            KeyCode::Up => {
                self.input_cursor =
                    move_cursor_vertically(&self.input_buffer, self.input_cursor, -1);
            }
            KeyCode::Down => {
                self.input_cursor =
                    move_cursor_vertically(&self.input_buffer, self.input_cursor, 1);
            }
            KeyCode::Home => self.input_cursor = 0,
            KeyCode::End => self.input_cursor = self.input_buffer.len(),
            _ => {}
        }
    }

    fn send_reply(&mut self) {
        let text = self.input_buffer.trim().to_string();
        if text.is_empty() {
            return;
        }

        let number = match self.selected_convo() {
            Some(c) => c.number.clone(),
            None => return,
        };

        match mmcli::create_and_send_sms(self.modem_index, &number, &text) {
            Ok(()) => {
                self.input_buffer.clear();
                self.input_cursor = 0;
                self.status_message = "Message sent!".to_string();
                self.refresh_messages();
            }
            Err(e) => {
                self.status_message = format!("Send failed: {}", e);
            }
        }
    }

    fn do_delete_conversation(&mut self) {
        let convo = match self.selected_convo() {
            Some(c) => c.clone(),
            None => return,
        };

        let mut errors = 0;
        for msg in &convo.messages {
            if let Err(e) = mmcli::delete_sms(self.modem_index, msg.index) {
                log::error!("Failed to delete SMS {}: {}", msg.index, e);
                errors += 1;
            }
        }

        if errors == 0 {
            self.status_message = format!("Deleted conversation with {}", convo.number);
        } else {
            self.status_message = format!(
                "Deleted conversation with {} ({} errors)",
                convo.number, errors
            );
        }
        self.refresh_messages();
    }

    fn do_delete_message(&mut self) {
        let msg = match self
            .selected_convo()
            .and_then(|c| c.messages.get(self.selected_message))
        {
            Some(m) => m.clone(),
            None => return,
        };

        match mmcli::delete_sms(self.modem_index, msg.index) {
            Ok(()) => {
                self.status_message = format!("Deleted message {}", msg.index);
            }
            Err(e) => {
                self.status_message = format!("Failed to delete message: {}", e);
            }
        }

        self.refresh_messages();

        // Clamp selected_message after refresh
        let msg_count = self.selected_convo().map(|c| c.messages.len()).unwrap_or(0);
        if msg_count > 0 && self.selected_message >= msg_count {
            self.selected_message = msg_count - 1;
        }
    }

    fn open_new_message(&mut self) {
        self.new_message_dialog = Some(NewMessageDialog {
            number: String::new(),
            number_cursor: 0,
            body: String::new(),
            body_cursor: 0,
            focus: NewMsgFocus::Number,
        });
    }

    fn handle_new_message_key(&mut self, key: KeyEvent) {
        let dlg = match self.new_message_dialog.as_mut() {
            Some(d) => d,
            None => return,
        };

        match key.code {
            KeyCode::Esc => {
                self.new_message_dialog = None;
                return;
            }
            KeyCode::Tab => {
                dlg.focus = match dlg.focus {
                    NewMsgFocus::Number => NewMsgFocus::Body,
                    NewMsgFocus::Body => NewMsgFocus::Number,
                };
                return;
            }
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.send_new_message();
                return;
            }
            _ => {}
        }

        let dlg = self.new_message_dialog.as_mut().unwrap();
        match dlg.focus {
            NewMsgFocus::Number => {
                handle_text_input(&mut dlg.number, &mut dlg.number_cursor, key, false)
            }
            NewMsgFocus::Body => handle_text_input(&mut dlg.body, &mut dlg.body_cursor, key, true),
        }
    }

    fn send_new_message(&mut self) {
        let dlg = match self.new_message_dialog.take() {
            Some(d) => d,
            None => return,
        };

        let number = dlg.number.trim().to_string();
        let text = dlg.body.trim().to_string();

        if number.is_empty() {
            self.status_message = "No number specified".to_string();
            self.new_message_dialog = Some(dlg);
            return;
        }
        if text.is_empty() {
            self.status_message = "No message text".to_string();
            self.new_message_dialog = Some(dlg);
            return;
        }

        match mmcli::create_and_send_sms(self.modem_index, &number, &text) {
            Ok(()) => {
                self.status_message = format!("Message sent to {}!", number);
                self.refresh_messages();
            }
            Err(e) => {
                self.status_message = format!("Send failed: {}", e);
            }
        }
    }

    fn open_modem_picker(&mut self) {
        let modem_indices = mmcli::list_modems().unwrap_or_default();
        if modem_indices.is_empty() {
            self.status_message = "No modems found".to_string();
            return;
        }
        let modems: Vec<ModemInfo> = modem_indices
            .iter()
            .filter_map(|&idx| mmcli::get_modem_info(idx).ok())
            .collect();
        if modems.is_empty() {
            self.status_message = "Failed to query modem info".to_string();
            return;
        }
        let selected = modems
            .iter()
            .position(|m| m.index == self.modem_index)
            .unwrap_or(0);
        self.modem_picker = Some(ModemPicker { modems, selected });
    }

    fn handle_modem_picker_key(&mut self, key: KeyEvent) {
        let picker = match self.modem_picker.as_mut() {
            Some(p) => p,
            None => return,
        };

        match key.code {
            KeyCode::Esc => {
                self.modem_picker = None;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if picker.selected > 0 {
                    picker.selected -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if picker.selected + 1 < picker.modems.len() {
                    picker.selected += 1;
                }
            }
            KeyCode::Enter => {
                let new_modem = picker.modems[picker.selected].index;
                self.modem_picker = None;
                if new_modem != self.modem_index {
                    self.modem_index = new_modem;
                    self.selected_conversation = 0;
                    self.selected_message = 0;
                    self.scroll_offset = 0;
                    self.input_buffer.clear();
                    self.input_cursor = 0;
                    self.focus = Focus::ConversationList;
                    self.refresh_messages();
                    self.status_message =
                        format!("Switched to modem {}", new_modem);
                }
            }
            _ => {}
        }
    }
}

fn handle_text_input(buf: &mut String, cursor: &mut usize, key: KeyEvent, multiline: bool) {
    match key.code {
        KeyCode::Enter if multiline => {
            buf.insert(*cursor, '\n');
            *cursor += 1;
        }
        KeyCode::Char(c) => {
            buf.insert(*cursor, c);
            *cursor += c.len_utf8();
        }
        KeyCode::Backspace => {
            if *cursor > 0 {
                let prev = buf[..*cursor]
                    .chars()
                    .last()
                    .map(|c| c.len_utf8())
                    .unwrap_or(0);
                *cursor -= prev;
                buf.remove(*cursor);
            }
        }
        KeyCode::Delete => {
            if *cursor < buf.len() {
                buf.remove(*cursor);
            }
        }
        KeyCode::Left => {
            if *cursor > 0 {
                let prev = buf[..*cursor]
                    .chars()
                    .last()
                    .map(|c| c.len_utf8())
                    .unwrap_or(0);
                *cursor -= prev;
            }
        }
        KeyCode::Right => {
            if *cursor < buf.len() {
                let next = buf[*cursor..]
                    .chars()
                    .next()
                    .map(|c| c.len_utf8())
                    .unwrap_or(0);
                *cursor += next;
            }
        }
        KeyCode::Up if multiline => {
            *cursor = move_cursor_vertically(buf, *cursor, -1);
        }
        KeyCode::Down if multiline => {
            *cursor = move_cursor_vertically(buf, *cursor, 1);
        }
        KeyCode::Home => *cursor = 0,
        KeyCode::End => *cursor = buf.len(),
        _ => {}
    }
}

/// Move the byte cursor up (direction = -1) or down (direction = 1) by one line,
/// preserving the character column where possible.
fn move_cursor_vertically(text: &str, byte_cursor: usize, direction: i32) -> usize {
    let before = &text[..byte_cursor.min(text.len())];

    // Find current line start and column (in chars)
    let current_line_start = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let current_col: usize = before[current_line_start..].chars().count();

    if direction < 0 {
        // Move up
        if current_line_start == 0 {
            return byte_cursor; // already on first line
        }
        // Previous line ends at current_line_start - 1 (the '\n')
        let prev_content = &text[..current_line_start - 1];
        let prev_line_start = prev_content.rfind('\n').map(|i| i + 1).unwrap_or(0);
        let prev_line = &text[prev_line_start..current_line_start - 1];
        let target_col = current_col.min(prev_line.chars().count());
        byte_offset_at_char(text, prev_line_start, target_col)
    } else {
        // Move down
        let line_end = text[current_line_start..].find('\n');
        let next_line_start = match line_end {
            Some(offset) => current_line_start + offset + 1,
            None => return byte_cursor, // already on last line
        };
        if next_line_start > text.len() {
            return byte_cursor;
        }
        let next_line_end = text[next_line_start..]
            .find('\n')
            .map(|i| next_line_start + i)
            .unwrap_or(text.len());
        let next_line = &text[next_line_start..next_line_end];
        let target_col = current_col.min(next_line.chars().count());
        byte_offset_at_char(text, next_line_start, target_col)
    }
}

/// Get the byte offset after advancing `char_count` characters from `start_byte`.
fn byte_offset_at_char(text: &str, start_byte: usize, char_count: usize) -> usize {
    text[start_byte..]
        .char_indices()
        .nth(char_count)
        .map(|(i, _)| start_byte + i)
        .unwrap_or_else(|| {
            // Past end of line — clamp to end of line
            text[start_byte..]
                .find('\n')
                .map(|i| start_byte + i)
                .unwrap_or(text.len())
        })
}
