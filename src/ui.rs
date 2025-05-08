use codesnake::{CodeWidth, Label, LineIndex};
use itertools::{Itertools, repeat_n};
use ratatui::{
    layout::Constraint::{Fill, Length},
    prelude::*,
    widgets::{
        Block, Borders, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
        Wrap,
    },
};

use crate::{
    crumbs::Crumbs,
    jq::Jaqqer,
    model::{Filter, Mode, Model},
};

pub(crate) const SCROLL_BOTTOM_OFFSET: usize = 32;
pub(crate) const PROMPT: &str = "❯ ";

pub fn render(frame: &mut Frame, model: &Model) {
    let border = Block::bordered().title(Line::raw("MqtTUI").centered());
    let area = border.inner(frame.area());
    frame.render_widget(border, frame.area());
    match model.mode() {
        Mode::Topics { filter } => render_topics(frame, area, model, filter.as_ref()),
        Mode::Detail {
            topic,
            scroll,
            index,
            jq,
        } => render_details(frame, area, model, topic, jq, *scroll, *index),
    }
}

fn connection_status(model: &Model) -> Paragraph {
    Paragraph::new("● ")
        .style(Style::new().fg(if model.connected {
            Color::Green
        } else {
            Color::Red
        }))
        .right_aligned()
}

fn render_topics(frame: &mut Frame, area: Rect, model: &Model, filter: Option<&Filter>) {
    let [top, overview, prompt] = Layout::vertical([
        Length(1),
        Fill(0),
        Length(if filter.is_some() { 3 } else { 0 }),
    ])
    .areas(area);

    let [counter, host, indicator] = Layout::horizontal([Length(6), Fill(0), Length(6)]).areas(top);

    // Top header
    frame.render_widget(
        Paragraph::new(format!("{}", model.counter)).dark_gray(),
        counter,
    );
    frame.render_widget(
        Paragraph::new(model.broker().to_string()).centered().bold(),
        host,
    );
    frame.render_widget(connection_status(model), indicator);

    // Topic overview
    let list = List::new(model.topics().map(|(topic, message)| {
        let config = model.config();
        let style = if model.selection().is_some_and(|s| topic.as_str() == s) {
            let mut style = Style::new().bg(config.colors.selection).fg(Color::Black);
            if model.is_copy() {
                style = style.reversed();
            }
            style
        } else {
            Style::new().fg(message.freshness(config))
        };
        ListItem::new(message.topic.line(style)).style(style)
    }))
    .block(
        Block::new()
            .title(Line::raw("Topics").italic().dark_gray())
            .borders(Borders::TOP),
    );

    frame.render_widget(list, overview);

    if let Some(filter) = filter {
        let input = format!("{PROMPT}{}", filter.pattern());
        frame.render_widget(
            Paragraph::new(input.as_str()).block(
                Block::new()
                    .title(Line::raw(filter.kind()).centered())
                    .borders(Borders::TOP),
            ),
            prompt,
        );
        let x = PROMPT.chars().count() as u16 + filter.cursor();
        frame.set_cursor_position((prompt.x + x, prompt.y + 1));
    }
}

fn render_details(
    frame: &mut Frame,
    area: Rect,
    model: &Model,
    topic: &str,
    jq: &Jaqqer,
    scroll: u16,
    index: Option<usize>,
) {
    let message = model.message(topic, index).unwrap_or_default();
    let error = model.error(topic, index);

    let [header, pane, crumbs, warning, footer] = Layout::vertical([
        Length(1),
        Fill(0),
        Length(2),
        Length(error.map(|e| e.lines().count() + 1).unwrap_or_default() as u16),
        Length(if jq.is_dormant() { 0 } else { 6 }),
    ])
    .areas(area);

    let [header, indicator] = Layout::horizontal([Fill(0), Length(2)]).areas(header);
    let [details, mut scroller] = Layout::horizontal([Fill(0), Length(1)]).areas(pane);

    // Top header with topic name
    frame.render_widget(Paragraph::new(topic).bold().centered(), header);
    frame.render_widget(connection_status(model), indicator);

    // in case of error we wrap the message so we always show a scrollbar since we don't know
    // how many lines will end up in the text box
    let scrollable = message.lines().count() as u16 > details.height || error.is_some();
    let scroll = if scrollable { scroll } else { 0 };

    let mut style = Style::new();
    if model.is_copy() {
        style = style.reversed();
    }
    frame.render_widget(
        Paragraph::new(
            if error.is_none() {
                model.highlight(&message, details, scroll)
            } else {
                message.clone().into()
            }
            .style(style),
        )
        .block(Block::new().title("Message").borders(Borders::TOP))
        .scroll((scroll, 0))
        .wrap(Wrap { trim: false }),
        details,
    );

    let count = model.message_count(topic) + 1;
    frame.render_widget(
        Crumbs::new(index, count).block(
            Block::new()
                .title(Line::raw("History").italic().dark_gray())
                .borders(Borders::TOP),
        ),
        crumbs,
    );
    if let Some(error) = error {
        frame.render_widget(
            Paragraph::new(Text::from(error).dark_gray()).block(
                Block::new()
                    .title(Line::raw("Warning").italic().yellow())
                    .borders(Borders::TOP),
            ),
            warning,
        )
    }

    if scrollable {
        scroller.y += 1;
        scroller.height -= 1;
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None),
            scroller,
            &mut ScrollbarState::new(message.lines().count().saturating_sub(SCROLL_BOTTOM_OFFSET))
                .position(scroll as usize),
        );
    }
    let filter = match jq {
        Jaqqer::Dormant => Default::default(),
        Jaqqer::Prompt {
            prompt,
            cursor,
            errors,
            ..
        } => {
            let mut input = format!("{PROMPT}{prompt}");
            let x = ((input.chars().count() - prompt.chars().count()) as u16 + cursor)
                .min(footer.width - 1);
            frame.set_cursor_position((footer.x + x, footer.y + 1));

            if !prompt.is_empty() && !errors.is_empty() {
                let idx = LineIndex::new(prompt);
                let block = codesnake::Block::new(
                    &idx,
                    errors
                        .iter()
                        .map(|e| Label::new(e.span.clone()).with_text(&e.message)),
                );
                if let Some(block) = block {
                    let block = block.map_code(|c| CodeWidth::new(c, c.len()));
                    input = format!("{block}")
                        .replacen("1 │ ", PROMPT, 1)
                        .lines()
                        .skip(1)
                        .map(|line| {
                            line.replacen(
                                "  ┆ ",
                                &repeat_n(' ', PROMPT.chars().count()).collect::<String>(),
                                1,
                            )
                        })
                        .join("\n");
                }
            }

            Paragraph::new(input).style(if errors.is_empty() {
                Color::LightBlue
            } else {
                Color::Yellow
            })
        }
        Jaqqer::Active { prompt, errors, .. } => {
            let mut input = format!("{PROMPT}{prompt}");

            for error in errors {
                input.push('\n');
                input.push_str(&error.display());
            }
            Paragraph::new(input).style(
                Style::new()
                    .fg(if errors.is_empty() {
                        Color::White
                    } else {
                        Color::Red
                    })
                    .bold(),
            )
        }
    };

    frame.render_widget(
        filter.block(
            Block::new()
                .title(Line::raw("JQ-Filter").centered())
                .borders(Borders::TOP),
        ),
        footer,
    );
}
