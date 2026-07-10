use crate::app::{App, ConnState};
use crate::client::level_str;
use amos_common::entities::{LogEvent, LogLevel};
use ratatui::{prelude::*, widgets::*};

pub fn draw(f: &mut Frame, app: &mut App) {
    let root = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(f.area());
    let body = Layout::horizontal([Constraint::Length(30), Constraint::Min(0)]).split(root[0]);

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

/// Hanging indent applied to wrapped continuation rows of a log entry.
const CONT_INDENT: usize = 2;

fn draw_logs(f: &mut Frame, app: &mut App, area: Rect) {
    // Width/height available inside the border.
    let inner_w = area.width.saturating_sub(2) as usize;
    let inner_h = area.height.saturating_sub(2) as usize;
    app.viewport = inner_h; // let the key handler size a half-page step

    // Wrap entries into physical rows, newest last. Walk from the newest entry
    // backwards, collecting enough to fill the pane plus the scroll offset.
    let need = inner_h.saturating_add(app.scroll);
    let mut rows: Vec<Line> = Vec::new();
    for e in app.logs.iter().rev() {
        let mut entry = render_entry(e, inner_w);
        entry.extend(std::mem::take(&mut rows));
        rows = entry;
        if rows.len() >= need {
            break;
        }
    }

    // Clamp the scroll offset to what actually exists, then take the window.
    let max_scroll = rows.len().saturating_sub(inner_h);
    app.scroll = app.scroll.min(max_scroll);
    let end = rows.len() - app.scroll;
    let start = end.saturating_sub(inner_h);
    let visible: Vec<Line> = rows[start..end].to_vec();

    let mut title = format!(
        "Logs — level:{}  device:{}",
        app.min_level.map(level_str).unwrap_or("all"),
        app.selected_device_label()
    );
    if app.scroll > 0 {
        title.push_str(&format!("  ↑{} (u/d scroll)", app.scroll));
    }
    f.render_widget(
        Paragraph::new(visible).block(Block::bordered().title(title)),
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
        key("u/d"),
        Span::raw(" scroll  "),
        key("c"),
        Span::raw(" clear   "),
    ];
    spans.push(match &app.conn {
        ConnState::Reconnecting => Span::styled("● reconnecting", Style::new().fg(Color::Yellow)),
        ConnState::Live => Span::styled("● live", Style::new().fg(Color::Green)),
        ConnState::Error(e) => Span::styled(format!("● error: {e}"), Style::new().fg(Color::Red)),
    });
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// Render one log entry as one or more physical rows, wrapping the message to
/// `width` cells. The first row carries the `time / level / [dev…]` prefix; any
/// continuation rows are hanging-indented so they read as the same entry.
fn render_entry(e: &LogEvent, width: usize) -> Vec<Line<'static>> {
    let (time, level, mut body, source, tag) = match e {
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
    if let Some(s) = source {
        body.push_str(&format!(" ({s})"));
    }

    let time_str = format!("{} ", time.format("%H:%M:%S"));
    let level_str_up = format!("{:<5} ", level_str(level).to_uppercase());
    let prefix_w = time_str.chars().count() + level_str_up.chars().count() + tag.chars().count();

    let width = width.max(1);
    let first_w = width.saturating_sub(prefix_w).max(1);
    let cont_w = width.saturating_sub(CONT_INDENT).max(1);
    let chunks = wrap_text(&body, first_w, cont_w);

    let color = level_color(level);
    let mut lines = Vec::with_capacity(chunks.len().max(1));
    let first = chunks.first().cloned().unwrap_or_default();
    lines.push(Line::from(vec![
        Span::styled(time_str, Style::new().dim()),
        Span::styled(level_str_up, Style::new().fg(color)),
        Span::styled(tag, Style::new().dim()),
        Span::raw(first),
    ]));
    for chunk in chunks.iter().skip(1) {
        let mut s = " ".repeat(CONT_INDENT);
        s.push_str(chunk);
        lines.push(Line::from(Span::raw(s)));
    }
    lines
}

/// Greedy word-wrap on whitespace (collapsing runs of it). Words longer than a
/// line are hard-split. The first line may have a different width than the rest
/// (to account for the prefix). Treats one char as one cell — fine for logs.
fn wrap_text(text: &str, first_width: usize, cont_width: usize) -> Vec<String> {
    let first_width = first_width.max(1);
    let cont_width = cont_width.max(1);
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();

    let limit_for = |line_idx: usize| {
        if line_idx == 0 {
            first_width
        } else {
            cont_width
        }
    };

    for word in text.split_whitespace() {
        let mut word = word.to_string();
        loop {
            let limit = limit_for(out.len());
            let cur_len = cur.chars().count();
            let sep = usize::from(!cur.is_empty());
            let word_len = word.chars().count();

            if cur_len + sep + word_len <= limit {
                if !cur.is_empty() {
                    cur.push(' ');
                }
                cur.push_str(&word);
                break;
            } else if cur.is_empty() {
                // Word doesn't fit on an empty line: take what we can, recurse.
                let head: String = word.chars().take(limit).collect();
                out.push(head);
                word = word.chars().skip(limit).collect();
                if word.is_empty() {
                    break;
                }
            } else {
                // Flush the current line and retry the word on a fresh one.
                out.push(std::mem::take(&mut cur));
            }
        }
    }
    if !cur.is_empty() || out.is_empty() {
        out.push(cur);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::wrap_text;

    #[test]
    fn wraps_on_word_boundaries() {
        // first line width 5, continuation width 10
        let out = wrap_text("hello there world", 5, 10);
        assert_eq!(out, vec!["hello", "there", "world"]);
    }

    #[test]
    fn hard_splits_overlong_words() {
        let out = wrap_text("abcdefghij", 4, 4);
        assert_eq!(out, vec!["abcd", "efgh", "ij"]);
    }

    #[test]
    fn empty_text_yields_one_empty_line() {
        assert_eq!(wrap_text("", 8, 8), vec![String::new()]);
    }

    #[test]
    fn packs_multiple_words_per_line() {
        let out = wrap_text("a b c d e f", 5, 5);
        assert_eq!(out, vec!["a b c", "d e f"]);
    }
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
