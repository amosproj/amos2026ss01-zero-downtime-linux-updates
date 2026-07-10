use crate::app::{App, ConnState};
use crate::client::level_str;
use amos_common::entities::{LogEvent, LogLevel};
use ratatui::{prelude::*, widgets::*};

pub fn draw(f: &mut Frame, app: &App) {
    let root =
        Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(f.area());
    let body =
        Layout::horizontal([Constraint::Length(30), Constraint::Min(0)]).split(root[0]);

    draw_sidebar(f, app, body[0]);
    draw_logs(f, app, body[1]);
    draw_footer(f, app, root[1]);
}

fn draw_sidebar(f: &mut Frame, app: &App, area: Rect) {
    let mut items: Vec<ListItem> = Vec::with_capacity(app.devices.len() + 1);
    items.push(ListItem::new("[ all devices ]"));
    for d in &app.devices {
        items.push(ListItem::new(format!("{} (#{})", d.serial_number, d.id)));
    }

    let mut state = ListState::default();
    state.select(Some(app.selected));

    let list = List::new(items)
        .block(Block::bordered().title("Devices"))
        .highlight_style(Style::new().reversed());
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_logs(f: &mut Frame, app: &App, area: Rect) {
    // Show the newest lines that fit inside the border.
    let visible = area.height.saturating_sub(2) as usize;
    let lines: Vec<Line> = app
        .logs
        .iter()
        .rev()
        .take(visible)
        .rev()
        .map(render_line)
        .collect();

    let title = format!(
        "Logs — level:{}  device:{}",
        app.min_level.map(level_str).unwrap_or("all"),
        app.selected_device_label()
    );
    f.render_widget(
        Paragraph::new(lines).block(Block::bordered().title(title)),
        area,
    );
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let key = |k: &'static str| Span::styled(k, Style::new().bold());
    let mut spans = vec![
        key("q"),
        Span::raw(" quit  "),
        key("0/1/2/3"),
        Span::raw(" level  "),
        key("j/k"),
        Span::raw(" device  "),
        key("a"),
        Span::raw(" all  "),
        key("c"),
        Span::raw(" clear   "),
    ];
    spans.push(match &app.conn {
        ConnState::Reconnecting => {
            Span::styled("● reconnecting", Style::new().fg(Color::Yellow))
        }
        ConnState::Live => Span::styled("● live", Style::new().fg(Color::Green)),
        ConnState::Error(e) => {
            Span::styled(format!("● error: {e}"), Style::new().fg(Color::Red))
        }
    });
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_line(e: &LogEvent) -> Line<'static> {
    let (time, level, message, source, tag) = match e {
        LogEvent::Device(m) => (
            m.time,
            m.level,
            m.message.clone(),
            m.source.clone(),
            format!("[dev {}] ", m.device_id),
        ),
        LogEvent::Application(m) => (
            m.time,
            m.level,
            m.message.clone(),
            m.source.clone(),
            format!("[dev {}/app {}] ", m.device_id, m.application_id),
        ),
    };

    let mut spans = vec![
        Span::styled(format!("{} ", time.format("%H:%M:%S")), Style::new().dim()),
        Span::styled(
            format!("{:<5} ", level_str(level).to_uppercase()),
            Style::new().fg(level_color(level)),
        ),
        Span::styled(tag, Style::new().dim()),
        Span::raw(message),
    ];
    if let Some(s) = source {
        spans.push(Span::styled(format!(" ({s})"), Style::new().dim()));
    }
    Line::from(spans)
}

fn level_color(level: LogLevel) -> Color {
    match level {
        LogLevel::Trace => Color::DarkGray,
        LogLevel::Debug => Color::Gray,
        LogLevel::Info => Color::Green,
        LogLevel::Warn => Color::Yellow,
        LogLevel::Error => Color::Red,
        LogLevel::Fatal => Color::Magenta,
    }
}
