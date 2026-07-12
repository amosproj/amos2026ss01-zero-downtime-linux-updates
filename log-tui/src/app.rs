use crate::client::{Client, Msg, MsgKind, StreamHandle};
use amos_common::entities::{Device, LogEvent, LogLevel, LogStreamQuery};
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use std::collections::VecDeque;
use tokio::sync::mpsc;

/// Connection state of the current SSE subscription (shown in the footer).
pub enum ConnState {
    Reconnecting,
    Live,
    Error(String),
}

pub struct App {
    client: Client,
    tx: mpsc::Sender<Msg>,

    pub devices: Vec<Device::Model>,
    /// 0 = "all devices"; otherwise `devices[selected - 1]`.
    pub selected: usize,
    pub min_level: Option<LogLevel>,

    pub logs: VecDeque<LogEvent>,
    max_logs: usize,

    /// Rows scrolled up from the bottom. 0 = follow the live tail.
    pub scroll: usize,
    /// Height (in rows) of the log pane on the last render; used to size a
    /// half-page scroll step. Set by the UI each frame.
    pub viewport: usize,

    pub conn: ConnState,
    pub last_warn: Option<String>,
    pub should_quit: bool,

    /// Identifies the current subscription; bumped on every reconnect so stale
    /// messages from an aborted stream task are ignored.
    epoch: u64,
    stream: Option<StreamHandle>,
}

impl App {
    pub fn new(
        client: Client,
        tx: mpsc::Sender<Msg>,
        min_level: Option<LogLevel>,
        max_logs: usize,
    ) -> Self {
        Self {
            client,
            tx,
            devices: Vec::new(),
            selected: 0,
            min_level,
            logs: VecDeque::new(),
            max_logs: max_logs.max(1),
            scroll: 0,
            viewport: 0,
            conn: ConnState::Reconnecting,
            last_warn: None,
            should_quit: false,
            epoch: 0,
            stream: None,
        }
    }

    fn selected_device_id(&self) -> Option<i32> {
        if self.selected == 0 {
            None
        } else {
            self.devices.get(self.selected - 1).map(|d| d.id)
        }
    }

    pub fn selected_device_label(&self) -> String {
        match self.selected_device_id() {
            Some(id) => format!("#{id}"),
            None => "all".to_string(),
        }
    }

    fn current_query(&self) -> LogStreamQuery {
        LogStreamQuery {
            device_id: self.selected_device_id(),
            application_id: None,
            level: self.min_level,
            kind: None,
        }
    }

    /// Tear down the current stream and open a fresh one with current filters.
    pub fn reconnect(&mut self) {
        self.epoch += 1;
        // Aborting the old task drops its response body and closes the connection.
        if let Some(handle) = self.stream.take() {
            handle.stop();
        }
        self.conn = ConnState::Reconnecting;
        self.scroll = 0; // snap back to the live tail on a new subscription
        self.stream = Some(self.client.spawn_stream(
            self.current_query(),
            self.epoch,
            self.tx.clone(),
        ));
    }

    pub fn on_msg(&mut self, msg: Msg) {
        if msg.epoch != self.epoch {
            return; // stale message from a torn-down stream
        }
        match msg.kind {
            MsgKind::Connected => self.conn = ConnState::Live,
            MsgKind::Log(ev) => {
                self.conn = ConnState::Live;
                self.push_log(*ev);
            }
            MsgKind::Disconnected(e) => self.conn = ConnState::Error(e),
            MsgKind::ParseWarn(e) => self.last_warn = Some(e),
        }
    }

    fn push_log(&mut self, ev: LogEvent) {
        self.logs.push_back(ev);
        while self.logs.len() > self.max_logs {
            self.logs.pop_front();
        }
        // When scrolled up, keep the viewed window roughly anchored as new lines
        // arrive (approximate: one row per entry; the UI clamps the final value).
        if self.scroll > 0 {
            self.scroll += 1;
        }
    }

    pub fn on_key(&mut self, code: KeyCode, mods: KeyModifiers) {
        // Ctrl-C quits regardless of the key below.
        if mods.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('0') => self.set_level(None),
            KeyCode::Char('1') => self.set_level(Some(LogLevel::Info)),
            KeyCode::Char('2') => self.set_level(Some(LogLevel::Warn)),
            KeyCode::Char('3') => self.set_level(Some(LogLevel::Error)),
            KeyCode::Char('c') => {
                self.logs.clear();
                self.scroll = 0;
            }
            KeyCode::Char('u') => self.scroll_up(),
            KeyCode::Char('d') => self.scroll_down(),
            KeyCode::Char('j') | KeyCode::Down => self.select_next(),
            KeyCode::Char('k') | KeyCode::Up => self.select_prev(),
            KeyCode::Char('a') | KeyCode::Left => self.select_all(),
            _ => {}
        }
    }

    /// Half-page scroll step, based on the last rendered pane height.
    fn scroll_step(&self) -> usize {
        (self.viewport / 2).max(1)
    }

    /// Scroll toward older logs. The UI clamps `scroll` to the available rows.
    fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_add(self.scroll_step());
    }

    /// Scroll toward the live tail; 0 resumes following.
    fn scroll_down(&mut self) {
        self.scroll = self.scroll.saturating_sub(self.scroll_step());
    }

    fn set_level(&mut self, level: Option<LogLevel>) {
        if self.min_level != level {
            self.min_level = level;
            self.reconnect();
        }
    }

    fn select_next(&mut self) {
        if self.selected < self.devices.len() {
            self.selected += 1;
            self.reconnect();
        }
    }

    fn select_prev(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.reconnect();
        }
    }

    fn select_all(&mut self) {
        if self.selected != 0 {
            self.selected = 0;
            self.reconnect();
        }
    }
}
