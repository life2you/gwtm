use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

pub struct MenuState {
    pub title: String,
    pub subtitle: String,
    pub items: Vec<String>,
    pub details: Vec<Vec<String>>,
    pub list_state: ListState,
}

pub enum MenuAction {
    Select(usize),
    Back,
    Quit,
}

impl MenuState {
    pub fn new(title: &str, subtitle: &str, items: Vec<String>) -> Self {
        let mut list_state = ListState::default();
        if !items.is_empty() {
            list_state.select(Some(0));
        }
        Self {
            title: title.to_string(),
            subtitle: subtitle.to_string(),
            items,
            details: Vec::new(),
            list_state,
        }
    }

    pub fn with_details(mut self, details: Vec<Vec<String>>) -> Self {
        self.details = details;
        self
    }

    pub fn selected(&self) -> Option<usize> {
        self.list_state.selected()
    }

    fn move_up(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let next = match self.list_state.selected() {
            Some(0) => self.items.len() - 1,
            Some(index) => index.saturating_sub(1),
            None => 0,
        };
        self.list_state.select(Some(next));
    }

    fn move_down(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let next = match self.list_state.selected() {
            Some(index) if index + 1 >= self.items.len() => 0,
            Some(index) => index + 1,
            None => 0,
        };
        self.list_state.select(Some(next));
    }

    pub fn handle_key_event(&mut self) -> Option<MenuAction> {
        if let Ok(Event::Key(key)) = event::read() {
            if key.kind != KeyEventKind::Press {
                return None;
            }
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.move_up(),
                KeyCode::Down | KeyCode::Char('j') => self.move_down(),
                KeyCode::Enter => {
                    if let Some(index) = self.selected() {
                        return Some(MenuAction::Select(index));
                    }
                }
                KeyCode::Esc | KeyCode::Char('b') => return Some(MenuAction::Back),
                KeyCode::Char('q') => return Some(MenuAction::Quit),
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Some(MenuAction::Quit);
                }
                _ => {}
            }
        }
        None
    }

    pub fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(6),
                Constraint::Length(8),
                Constraint::Length(1),
            ])
            .split(area);

        self.render_header(frame, chunks[0]);
        self.render_list(frame, chunks[1]);
        self.render_details(frame, chunks[2]);
        self.render_footer(frame, chunks[3]);
    }

    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let header = Paragraph::new(vec![
            Line::from(Span::styled(
                format!("  {}", self.title),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                format!("  {}", self.subtitle),
                Style::default().fg(Color::DarkGray),
            )),
        ])
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(Color::Rgb(81, 81, 81))),
        );
        frame.render_widget(header, area);
    }

    fn render_list(&mut self, frame: &mut Frame, area: Rect) {
        let selected = self.list_state.selected().unwrap_or(0);
        let items: Vec<ListItem> = self
            .items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                if index == selected {
                    ListItem::new(Line::from(vec![
                        Span::styled("  ▶ ", Style::default().fg(Color::Cyan)),
                        Span::styled(
                            item.as_str(),
                            Style::default()
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]))
                } else {
                    ListItem::new(Line::from(vec![
                        Span::raw("    "),
                        Span::styled(
                            format!("{}. ", index + 1),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled(
                            item.as_str(),
                            Style::default().fg(Color::Rgb(153, 153, 200)),
                        ),
                    ]))
                }
            })
            .collect();

        let list = List::new(items).block(Block::default());
        frame.render_stateful_widget(list, area, &mut self.list_state);
    }

    fn render_details(&self, frame: &mut Frame, area: Rect) {
        let selected = self.list_state.selected().unwrap_or(0);
        let lines: Vec<Line> = if selected < self.details.len() {
            self.details[selected]
                .iter()
                .map(|line| {
                    Line::from(Span::styled(
                        format!("  {line}"),
                        Style::default().fg(Color::Rgb(153, 200, 200)),
                    ))
                })
                .collect()
        } else {
            Vec::new()
        };

        let details = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(Color::Rgb(81, 81, 81))),
        );
        frame.render_widget(details, area);
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let footer = Paragraph::new(Line::from(vec![
            Span::styled("  ↑/↓", Style::default().fg(Color::DarkGray)),
            Span::styled(" 移动  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Enter", Style::default().fg(Color::DarkGray)),
            Span::styled(" 确认  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Esc", Style::default().fg(Color::DarkGray)),
            Span::styled(" 返回  ", Style::default().fg(Color::DarkGray)),
            Span::styled("q", Style::default().fg(Color::DarkGray)),
            Span::styled(" 退出", Style::default().fg(Color::DarkGray)),
        ]));
        frame.render_widget(footer, area);
    }
}

pub struct InputState {
    pub title: String,
    pub subtitle: String,
    pub label: String,
    pub value: String,
    pub error: Option<String>,
    pub cursor_pos: usize,
}

pub enum InputAction {
    Submit(String),
    Back,
    Quit,
}

impl InputState {
    pub fn new(title: &str, subtitle: &str, label: &str, placeholder: &str) -> Self {
        let value = placeholder.to_string();
        let cursor_pos = value.len();
        Self {
            title: title.to_string(),
            subtitle: subtitle.to_string(),
            label: label.to_string(),
            value,
            error: None,
            cursor_pos,
        }
    }

    pub fn handle_key_event(&mut self) -> Option<InputAction> {
        if let Ok(Event::Key(key)) = event::read() {
            if key.kind != KeyEventKind::Press {
                return None;
            }
            match key.code {
                KeyCode::Enter => {
                    if self.value.trim().is_empty() {
                        self.error = Some("输入不能为空".to_string());
                    } else {
                        return Some(InputAction::Submit(self.value.trim().to_string()));
                    }
                }
                KeyCode::Esc => return Some(InputAction::Back),
                KeyCode::Char('q') => return Some(InputAction::Quit),
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Some(InputAction::Quit);
                }
                KeyCode::Char(c) => {
                    self.value.insert(self.cursor_pos, c);
                    self.cursor_pos += 1;
                    self.error = None;
                }
                KeyCode::Backspace => {
                    if self.cursor_pos > 0 {
                        self.cursor_pos -= 1;
                        self.value.remove(self.cursor_pos);
                        self.error = None;
                    }
                }
                KeyCode::Delete => {
                    if self.cursor_pos < self.value.len() {
                        self.value.remove(self.cursor_pos);
                        self.error = None;
                    }
                }
                KeyCode::Left => {
                    if self.cursor_pos > 0 {
                        self.cursor_pos -= 1;
                    }
                }
                KeyCode::Right => {
                    if self.cursor_pos < self.value.len() {
                        self.cursor_pos += 1;
                    }
                }
                KeyCode::Home => self.cursor_pos = 0,
                KeyCode::End => self.cursor_pos = self.value.len(),
                _ => {}
            }
        }
        None
    }

    pub fn render(&self, frame: &mut Frame) {
        let area = frame.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(2),
                Constraint::Length(2),
                Constraint::Length(3),
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(area);

        self.render_header(frame, chunks[0]);
        self.render_label(frame, chunks[1]);
        self.render_error(frame, chunks[2]);
        self.render_input(frame, chunks[3]);
        self.render_footer(frame, chunks[5]);
    }

    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let header = Paragraph::new(vec![
            Line::from(Span::styled(
                format!("  {}", self.title),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                format!("  {}", self.subtitle),
                Style::default().fg(Color::DarkGray),
            )),
        ])
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(Color::Rgb(81, 81, 81))),
        );
        frame.render_widget(header, area);
    }

    fn render_label(&self, frame: &mut Frame, area: Rect) {
        let label = Paragraph::new(Line::from(Span::styled(
            format!("  {}", self.label),
            Style::default().fg(Color::Rgb(153, 153, 200)),
        )));
        frame.render_widget(label, area);
    }

    fn render_error(&self, frame: &mut Frame, area: Rect) {
        if let Some(ref error) = self.error {
            let widget = Paragraph::new(Line::from(Span::styled(
                format!("  {error}"),
                Style::default().fg(Color::Red),
            )));
            frame.render_widget(widget, area);
        }
    }

    fn render_input(&self, frame: &mut Frame, area: Rect) {
        let input = Paragraph::new(Line::from(vec![
            Span::styled(
                "> ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(self.value.as_str(), Style::default().fg(Color::White)),
        ]))
        .block(Block::default());

        frame.render_widget(input, area);
        frame.set_cursor_position((area.x + 2 + self.cursor_pos as u16, area.y));
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let footer = Paragraph::new(Line::from(vec![
            Span::styled("  Enter", Style::default().fg(Color::DarkGray)),
            Span::styled(" 确认  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Esc", Style::default().fg(Color::DarkGray)),
            Span::styled(" 返回  ", Style::default().fg(Color::DarkGray)),
            Span::styled("q", Style::default().fg(Color::DarkGray)),
            Span::styled(" 退出", Style::default().fg(Color::DarkGray)),
        ]));
        frame.render_widget(footer, area);
    }
}

pub struct ResultState {
    pub title: String,
    pub subtitle: String,
    pub lines: Vec<String>,
    pub scroll: u16,
}

pub enum ResultAction {
    Back,
    Quit,
}

impl ResultState {
    pub fn new(title: &str, subtitle: &str, lines: Vec<String>) -> Self {
        Self {
            title: title.to_string(),
            subtitle: subtitle.to_string(),
            lines,
            scroll: 0,
        }
    }

    pub fn handle_key_event(&mut self) -> Option<ResultAction> {
        if let Ok(Event::Key(key)) = event::read() {
            if key.kind != KeyEventKind::Press {
                return None;
            }
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.scroll = self.scroll.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let max_scroll = self.lines.len().saturating_sub(1) as u16;
                    self.scroll = self.scroll.saturating_add(1).min(max_scroll);
                }
                KeyCode::Enter | KeyCode::Esc | KeyCode::Char('b') => {
                    return Some(ResultAction::Back);
                }
                KeyCode::Char('q') => return Some(ResultAction::Quit),
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Some(ResultAction::Quit);
                }
                _ => {}
            }
        }
        None
    }

    pub fn render(&self, frame: &mut Frame) {
        let area = frame.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(6),
                Constraint::Length(1),
            ])
            .split(area);

        let header = Paragraph::new(vec![
            Line::from(Span::styled(
                format!("  {}", self.title),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                format!("  {}", self.subtitle),
                Style::default().fg(Color::DarkGray),
            )),
        ])
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(Color::Rgb(81, 81, 81))),
        );
        frame.render_widget(header, chunks[0]);

        let content: Vec<Line> = self
            .lines
            .iter()
            .map(|line| Line::from(Span::raw(format!("  {line}"))))
            .collect();
        let paragraph = Paragraph::new(content)
            .scroll((self.scroll, 0))
            .wrap(Wrap { trim: false });
        frame.render_widget(paragraph, chunks[1]);

        let footer = Paragraph::new(Line::from(vec![
            Span::styled("  ↑/↓", Style::default().fg(Color::DarkGray)),
            Span::styled(" 滚动  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Enter", Style::default().fg(Color::DarkGray)),
            Span::styled(" 返回  ", Style::default().fg(Color::DarkGray)),
            Span::styled("q", Style::default().fg(Color::DarkGray)),
            Span::styled(" 退出", Style::default().fg(Color::DarkGray)),
        ]));
        frame.render_widget(footer, chunks[2]);
    }
}
