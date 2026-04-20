use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::DefaultTerminal;
use std::sync::mpsc;
use std::time::{Duration, Instant};

const AUTO_REFRESH_INTERVAL: Duration = Duration::from_secs(5);

use crate::mmcli::{self, ModemInfo, SmsMessage};
use crate::ui;

/// Messages sent from background threads back to the main UI thread.
enum BgResult {
    Messages(Result<Vec<SmsMessage>>),
    SendOk(String),
    SendErr(String),
    DeleteConvo { number: String, errors: usize },
    DeleteMsg { index: u32, result: Result<()> },
}

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
    bg_rx: mpsc::Receiver<BgResult>,
    bg_tx: mpsc::Sender<BgResult>,
    refreshing: bool,
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
        let (bg_tx, bg_rx) = mpsc::channel();
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
            bg_rx,
            bg_tx,
            refreshing: false,
        }
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        self.trigger_bg_refresh();

        while self.running {
            terminal.draw(|frame| ui::draw(frame, self))?;

            // Process any background results (non-blocking)
            self.process_bg_results();

            if event::poll(Duration::from_millis(250))? {
                if let Event::Key(key) = event::read()? {
                    self.handle_key(key);
                }
            }

            // Auto-refresh when no dialog/popup is active
            if self.last_refresh.elapsed() >= AUTO_REFRESH_INTERVAL
                && !self.refreshing
                && !self.menu_active
                && !self.show_help_popup
                && !self.confirm_delete
                && !self.confirm_delete_message
                && self.new_message_dialog.is_none()
                && self.modem_picker.is_none()
            {
                self.trigger_bg_refresh();
            }
        }

        Ok(())
    }

    /// Spawn a background thread to load messages.
    fn trigger_bg_refresh(&mut self) {
        if self.refreshing {
            return;
        }
        self.refreshing = true;
        self.last_refresh = Instant::now();
        let tx = self.bg_tx.clone();
        let modem_index = self.modem_index;
        std::thread::spawn(move || {
            let result = load_messages_bg(modem_index);
            let _ = tx.send(BgResult::Messages(result));
        });
    }

    /// Process all pending background results.
    fn process_bg_results(&mut self) {
        while let Ok(result) = self.bg_rx.try_recv() {
            match result {
                BgResult::Messages(Ok(messages)) => {
                    self.refreshing = false;
                    self.apply_messages(messages);
                    let count: usize = self.conversations.iter().map(|c| c.messages.len()).sum();
                    self.status_message = format!(
                        "Modem {} | {} conversations | {} messages",
                        self.modem_index,
                        self.conversations.len(),
                        count,
                    );
                }
                BgResult::Messages(Err(e)) => {
                    self.refreshing = false;
                    self.status_message = format!("Error: {}", e);
                }
                BgResult::SendOk(msg) => {
                    self.status_message = msg;
                    self.trigger_bg_refresh();
                }
                BgResult::SendErr(msg) => {
                    self.status_message = msg;
                }
                BgResult::DeleteConvo { number, errors } => {
                    if errors == 0 {
                        self.status_message =
                            format!("Deleted conversation with {}", number);
                    } else {
                        self.status_message = format!(
                            "Deleted conversation with {} ({} errors)",
                            number, errors
                        );
                    }
                    self.trigger_bg_refresh();
                }
                BgResult::DeleteMsg { index, result } => {
                    match result {
                        Ok(()) => {
                            self.status_message = format!("Deleted message {}", index);
                        }
                        Err(e) => {
                            self.status_message = format!("Failed to delete message: {}", e);
                        }
                    }
                    self.trigger_bg_refresh();
                    // Clamp selected_message after refresh arrives
                }
            }
        }
    }

    /// Apply loaded messages to the app state.
    fn apply_messages(&mut self, messages: Vec<SmsMessage>) {
        // Group messages by phone number.
        // Numbers like "+8610086" and "10086" should be in the same conversation
        // (country code prefix difference).
        let mut groups: Vec<(Vec<String>, Vec<SmsMessage>)> = Vec::new();

        for msg in messages {
            let raw = msg.number.trim().to_string();
            let mut found = false;
            for (numbers, msgs) in &mut groups {
                if numbers.iter().any(|n| numbers_match_raw(n, &raw)) {
                    if !numbers.contains(&raw) {
                        numbers.push(raw.clone());
                    }
                    msgs.push(msg.clone());
                    found = true;
                    break;
                }
            }
            if !found {
                groups.push((vec![raw], vec![msg]));
            }
        }

        self.conversations = groups
            .into_iter()
            .map(|(numbers, mut msgs)| {
                msgs.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
                // Prefer the number with '+' (international format) for display
                let display_number = numbers
                    .iter()
                    .find(|n| n.starts_with('+'))
                    .or_else(|| numbers.iter().max_by_key(|n| n.len()))
                    .cloned()
                    .unwrap_or_default();
                Conversation {
                    number: display_number,
                    messages: msgs,
                }
            })
            .collect();

        self.conversations
            .sort_by(|a, b| b.last_timestamp().cmp(a.last_timestamp()));

        if !self.conversations.is_empty() && self.selected_conversation >= self.conversations.len()
        {
            self.selected_conversation = self.conversations.len() - 1;
        }

        let msg_count = self.selected_convo().map(|c| c.messages.len()).unwrap_or(0);
        if msg_count > 0 && self.selected_message >= msg_count {
            self.selected_message = msg_count - 1;
        }
    }

    pub fn refresh_messages(&mut self) {
        self.trigger_bg_refresh();
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
                    self.scroll_offset = u16::MAX;
                    self.selected_message = self.conversations[self.selected_conversation]
                        .messages.len().saturating_sub(1);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected_conversation + 1 < self.conversations.len() {
                    self.selected_conversation += 1;
                    self.scroll_offset = u16::MAX;
                    self.selected_message = self.conversations[self.selected_conversation]
                        .messages.len().saturating_sub(1);
                }
            }
            KeyCode::Enter => {
                if !self.conversations.is_empty() {
                    self.focus = Focus::MessageView;
                    self.scroll_offset = u16::MAX;
                    self.selected_message = self.conversations[self.selected_conversation]
                        .messages.len().saturating_sub(1);
                }
            }
            KeyCode::PageUp => {
                let page = 10;
                self.selected_conversation = self.selected_conversation.saturating_sub(page);
                self.scroll_offset = u16::MAX;
                self.selected_message = self.conversations[self.selected_conversation]
                    .messages.len().saturating_sub(1);
            }
            KeyCode::PageDown => {
                if !self.conversations.is_empty() {
                    let page = 10;
                    self.selected_conversation = (self.selected_conversation + page)
                        .min(self.conversations.len() - 1);
                    self.scroll_offset = u16::MAX;
                    self.selected_message = self.conversations[self.selected_conversation]
                        .messages.len().saturating_sub(1);
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
                let page = 5;
                self.selected_message = self.selected_message.saturating_sub(page);
            }
            KeyCode::PageDown => {
                if msg_count > 0 {
                    let page = 5;
                    self.selected_message = (self.selected_message + page).min(msg_count - 1);
                }
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
            KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => self.send_reply(),
            _ => handle_text_input(&mut self.input_buffer, &mut self.input_cursor, key, true),
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

        self.input_buffer.clear();
        self.input_cursor = 0;
        self.status_message = "Sending...".to_string();

        let tx = self.bg_tx.clone();
        let modem_index = self.modem_index;
        std::thread::spawn(move || {
            let result = match mmcli::create_and_send_sms(modem_index, &number, &text) {
                Ok(()) => BgResult::SendOk("Message sent!".to_string()),
                Err(e) => BgResult::SendErr(format!("Send failed: {}", e)),
            };
            let _ = tx.send(result);
        });
    }

    fn do_delete_conversation(&mut self) {
        let convo = match self.selected_convo() {
            Some(c) => c.clone(),
            None => return,
        };

        self.status_message = "Deleting...".to_string();
        let tx = self.bg_tx.clone();
        let modem_index = self.modem_index;
        std::thread::spawn(move || {
            let mut errors = 0;
            for msg in &convo.messages {
                if let Err(e) = mmcli::delete_sms(modem_index, msg.index) {
                    log::error!("Failed to delete SMS {}: {}", msg.index, e);
                    errors += 1;
                }
            }
            let _ = tx.send(BgResult::DeleteConvo {
                number: convo.number,
                errors,
            });
        });
    }

    fn do_delete_message(&mut self) {
        let msg = match self
            .selected_convo()
            .and_then(|c| c.messages.get(self.selected_message))
        {
            Some(m) => m.clone(),
            None => return,
        };

        self.status_message = "Deleting...".to_string();
        let tx = self.bg_tx.clone();
        let modem_index = self.modem_index;
        let msg_index = msg.index;
        std::thread::spawn(move || {
            let result = mmcli::delete_sms(modem_index, msg_index);
            let _ = tx.send(BgResult::DeleteMsg {
                index: msg_index,
                result,
            });
        });
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
            KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
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

        self.status_message = "Sending...".to_string();
        let tx = self.bg_tx.clone();
        let modem_index = self.modem_index;
        std::thread::spawn(move || {
            let result = match mmcli::create_and_send_sms(modem_index, &number, &text) {
                Ok(()) => BgResult::SendOk(format!("Message sent to {}!", number)),
                Err(e) => BgResult::SendErr(format!("Send failed: {}", e)),
            };
            let _ = tx.send(result);
        });
    }

    fn open_modem_picker(&mut self) {
        let modem_indices = match mmcli::list_modems() {
            Ok(indices) => indices,
            Err(e) => {
                self.status_message = format!("Failed to list modems: {}", e);
                return;
            }
        };
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
        KeyCode::PageUp if multiline => {
            for _ in 0..10 {
                *cursor = move_cursor_vertically(buf, *cursor, -1);
            }
        }
        KeyCode::PageDown if multiline => {
            for _ in 0..10 {
                *cursor = move_cursor_vertically(buf, *cursor, 1);
            }
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

/// Load messages from mmcli in a background thread (no &self needed).
fn load_messages_bg(modem_index: u32) -> Result<Vec<SmsMessage>> {
    let sms_entries = mmcli::list_sms(modem_index)?;
    let mut messages = Vec::new();
    for (index, _state) in &sms_entries {
        match mmcli::get_sms(*index) {
            Ok(msg) => messages.push(msg),
            Err(e) => {
                log::warn!("Failed to load SMS {}: {}", index, e);
            }
        }
    }
    Ok(messages)
}

/// Extract only digits from a phone number.
fn digits_only(number: &str) -> String {
    number.chars().filter(|c| c.is_ascii_digit()).collect()
}

/// Check if two raw phone numbers refer to the same contact.
/// Handles the case where one has a country code prefix (e.g. "+8610086" vs "10086").
/// Country codes are 1-3 digits, so we only try stripping those lengths.
fn numbers_match_raw(a: &str, b: &str) -> bool {
    let da = digits_only(a);
    let db = digits_only(b);

    if da == db {
        return true;
    }

    // Only attempt country code stripping if one number had a '+' prefix
    let a_intl = a.trim().starts_with('+');
    let b_intl = b.trim().starts_with('+');

    if a_intl == b_intl {
        // Both international or both local — must match exactly
        return false;
    }

    let (intl_digits, local_digits) = if a_intl { (&da, &db) } else { (&db, &da) };

    // Try stripping 1, 2, or 3 digit country code from the international number
    for cc_len in 1..=3 {
        if intl_digits.len() > cc_len && &intl_digits[cc_len..] == local_digits.as_str() {
            return true;
        }
    }

    false
}
