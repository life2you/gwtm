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
    pub search_enabled: bool,
    pub search_mode: bool,
    pub search_query: String,
    pub search_hint: String,
    filtered_indices: Vec<usize>,
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
            search_enabled: false,
            search_mode: false,
            search_query: String::new(),
            search_hint: "输入关键词过滤列表".to_string(),
            filtered_indices: Vec::new(),
        }
        .reset_filter()
    }

    pub fn with_details(mut self, details: Vec<Vec<String>>) -> Self {
        self.details = details;
        self
    }

    pub fn with_search(mut self, hint: &str) -> Self {
        self.search_enabled = true;
        self.search_hint = hint.to_string();
        self
    }

    pub fn selected(&self) -> Option<usize> {
        self.list_state
            .selected()
            .and_then(|index| self.filtered_indices.get(index).copied())
    }

    fn move_up(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }
        let next = match self.list_state.selected() {
            Some(0) => self.filtered_indices.len() - 1,
            Some(index) => index.saturating_sub(1),
            None => 0,
        };
        self.list_state.select(Some(next));
    }

    fn move_down(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }
        let next = match self.list_state.selected() {
            Some(index) if index + 1 >= self.filtered_indices.len() => 0,
            Some(index) => index + 1,
            None => 0,
        };
        self.list_state.select(Some(next));
    }

    fn reset_filter(mut self) -> Self {
        self.refresh_filter();
        self
    }

    fn refresh_filter(&mut self) {
        let query = self.search_query.trim().to_lowercase();
        self.filtered_indices = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                if query.is_empty() {
                    return Some(index);
                }

                let mut haystack = item.to_lowercase();
                if let Some(lines) = self.details.get(index) {
                    haystack.push('\n');
                    haystack.push_str(&lines.join("\n").to_lowercase());
                }

                haystack.contains(&query).then_some(index)
            })
            .collect();

        if self.filtered_indices.is_empty() {
            self.list_state.select(None);
        } else {
            let next = self
                .list_state
                .selected()
                .filter(|index| *index < self.filtered_indices.len())
                .unwrap_or(0);
            self.list_state.select(Some(next));
        }
    }

    pub fn handle_key_event(&mut self) -> Option<MenuAction> {
        if let Ok(Event::Key(key)) = event::read() {
            if key.kind != KeyEventKind::Press {
                return None;
            }
            if self.search_enabled && self.search_mode {
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => self.move_up(),
                    KeyCode::Down | KeyCode::Char('j') => self.move_down(),
                    KeyCode::Enter => {
                        if let Some(index) = self.selected() {
                            return Some(MenuAction::Select(index));
                        }
                    }
                    KeyCode::Backspace => {
                        if self.search_query.is_empty() {
                            self.search_mode = false;
                        } else {
                            self.search_query.pop();
                            self.refresh_filter();
                        }
                    }
                    KeyCode::Esc => {
                        if self.search_query.is_empty() {
                            return Some(MenuAction::Back);
                        } else {
                            self.search_query.clear();
                            self.refresh_filter();
                        }
                    }
                    KeyCode::Char('b') if self.search_query.is_empty() => {
                        return Some(MenuAction::Back);
                    }
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Some(MenuAction::Quit);
                    }
                    KeyCode::Char(c) => {
                        self.search_query.push(c);
                        self.refresh_filter();
                    }
                    _ => {}
                }
                return None;
            }
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.move_up(),
                KeyCode::Down | KeyCode::Char('j') => self.move_down(),
                KeyCode::Char('/') if self.search_enabled => {
                    self.search_mode = true;
                }
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
        let header_height = if self.search_enabled { 4 } else { 3 };
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(header_height),
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
        let mut lines = vec![
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
        ];

        if self.search_enabled {
            let prompt = if self.search_query.is_empty() {
                self.search_hint.clone()
            } else {
                self.search_query.clone()
            };
            let status = if self.search_mode {
                format!(
                    "  搜索: {prompt}  [{} / {}]",
                    self.filtered_indices.len(),
                    self.items.len()
                )
            } else {
                format!(
                    "  / 搜索: {prompt}  [{} / {}]",
                    self.filtered_indices.len(),
                    self.items.len()
                )
            };
            lines.push(Line::from(Span::styled(
                status,
                Style::default().fg(if self.search_mode {
                    Color::Yellow
                } else {
                    Color::DarkGray
                }),
            )));
        }

        let header = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(Color::Rgb(81, 81, 81))),
        );
        frame.render_widget(header, area);
    }

    fn render_list(&mut self, frame: &mut Frame, area: Rect) {
        let selected = self.list_state.selected();
        let items: Vec<ListItem> = if self.filtered_indices.is_empty() {
            vec![ListItem::new(Line::from(Span::styled(
                "  没有匹配结果",
                Style::default().fg(Color::DarkGray),
            )))]
        } else {
            self.filtered_indices
                .iter()
                .enumerate()
                .map(|(filtered_index, original_index)| {
                    let item = self.items[*original_index].as_str();
                    if selected == Some(filtered_index) {
                        ListItem::new(Line::from(vec![
                            Span::styled("  ▶ ", Style::default().fg(Color::Cyan)),
                            Span::styled(
                                item,
                                Style::default()
                                    .fg(Color::White)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]))
                    } else {
                        ListItem::new(Line::from(vec![
                            Span::raw("    "),
                            Span::styled(
                                format!("{}. ", filtered_index + 1),
                                Style::default().fg(Color::DarkGray),
                            ),
                            Span::styled(item, Style::default().fg(Color::Rgb(153, 153, 200))),
                        ]))
                    }
                })
                .collect()
        };

        let list = List::new(items).block(Block::default());
        if self.filtered_indices.is_empty() {
            frame.render_widget(list, area);
        } else {
            frame.render_stateful_widget(list, area, &mut self.list_state);
        }
    }

    fn render_details(&self, frame: &mut Frame, area: Rect) {
        let lines: Vec<Line> = if self.filtered_indices.is_empty() {
            vec![Line::from(Span::styled(
                "  调整搜索关键词后重试",
                Style::default().fg(Color::DarkGray),
            ))]
        } else if let Some(selected) = self.selected() {
            self.details
                .get(selected)
                .map(|detail_lines| {
                    detail_lines
                        .iter()
                        .map(|line| {
                            Line::from(Span::styled(
                                format!("  {line}"),
                                Style::default().fg(Color::Rgb(153, 200, 200)),
                            ))
                        })
                        .collect()
                })
                .unwrap_or_default()
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
        let search_label = if self.search_mode && !self.search_query.is_empty() {
            " Esc 清空  "
        } else if self.search_enabled {
            " / 搜索  "
        } else {
            ""
        };

        let footer = Paragraph::new(Line::from(vec![
            Span::styled("  ↑/↓", Style::default().fg(Color::DarkGray)),
            Span::styled(" 移动  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Enter", Style::default().fg(Color::DarkGray)),
            Span::styled(" 确认  ", Style::default().fg(Color::DarkGray)),
            Span::styled(search_label, Style::default().fg(Color::DarkGray)),
            Span::styled("Esc", Style::default().fg(Color::DarkGray)),
            Span::styled(" 返回  ", Style::default().fg(Color::DarkGray)),
            Span::styled("b", Style::default().fg(Color::DarkGray)),
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
    pub file_picker_enabled: bool,
}

pub enum InputAction {
    Submit(String),
    PickFolder,
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
            file_picker_enabled: false,
        }
    }

    pub fn with_file_picker(mut self) -> Self {
        self.file_picker_enabled = true;
        self
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
                KeyCode::Esc | KeyCode::Char('b') => return Some(InputAction::Back),
                KeyCode::Char('q') => return Some(InputAction::Quit),
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Some(InputAction::Quit);
                }
                KeyCode::Char('f') if self.file_picker_enabled => {
                    return Some(InputAction::PickFolder);
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
        let picker_hint = if self.file_picker_enabled {
            vec![
                Span::styled("f", Style::default().fg(Color::DarkGray)),
                Span::styled(" 选文件夹  ", Style::default().fg(Color::DarkGray)),
            ]
        } else {
            Vec::new()
        };

        let mut spans = vec![
            Span::styled("  Enter", Style::default().fg(Color::DarkGray)),
            Span::styled(" 确认  ", Style::default().fg(Color::DarkGray)),
        ];
        spans.extend(picker_hint);
        spans.extend(vec![
            Span::styled("Esc", Style::default().fg(Color::DarkGray)),
            Span::styled(" 返回  ", Style::default().fg(Color::DarkGray)),
            Span::styled("b", Style::default().fg(Color::DarkGray)),
            Span::styled(" 返回  ", Style::default().fg(Color::DarkGray)),
            Span::styled("q", Style::default().fg(Color::DarkGray)),
            Span::styled(" 退出", Style::default().fg(Color::DarkGray)),
        ]);

        let footer = Paragraph::new(Line::from(spans));
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
            Span::styled("Esc", Style::default().fg(Color::DarkGray)),
            Span::styled(" 返回  ", Style::default().fg(Color::DarkGray)),
            Span::styled("b", Style::default().fg(Color::DarkGray)),
            Span::styled(" 返回  ", Style::default().fg(Color::DarkGray)),
            Span::styled("q", Style::default().fg(Color::DarkGray)),
            Span::styled(" 退出", Style::default().fg(Color::DarkGray)),
        ]));
        frame.render_widget(footer, chunks[2]);
    }
}
