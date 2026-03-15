use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Clear, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Wrap,
    },
    Frame,
};

use crate::app::App;
use crate::types::{AppState, AppTab, Focus};

// ── Color palette ──────────────────────────────────────────────────────────────
const GOLD: Color = Color::Yellow; // #FFD700 — focused border
const BRIGHT_YELLOW: Color = Color::LightYellow; // #FFFF00 — selected item
const AMBER: Color = Color::Rgb(255, 165, 0); // #FFA500 — buttons / titles
const GREY: Color = Color::DarkGray; // unfocused border
const SELECTED_BG: Color = Color::Rgb(60, 50, 0);
const INSTALLED_GREEN: Color = Color::LightGreen;
const ACTIVE_GREEN: Color = Color::Green;
const BODY_TEXT: Color = Color::White;
const DIM_TEXT: Color = Color::DarkGray;
const ERROR_COLOR: Color = Color::LightRed;

/// Yellow when `focused`, grey otherwise.
fn border_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(GOLD)
    } else {
        Style::default().fg(GREY)
    }
}

fn title_style() -> Style {
    Style::default().fg(AMBER).add_modifier(Modifier::BOLD)
}

// ── Top-level render ──────────────────────────────────────────────────────────

pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // Root layout: title(1) | tab-bar(1) | content(fill) | hints(1)
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // title bar
            Constraint::Length(1), // tab bar
            Constraint::Min(0),    // content
            Constraint::Length(1), // hint bar
        ])
        .split(area);

    render_title_bar(frame, root[0], app);
    render_tab_bar(frame, root[1], app);

    match app.active_tab {
        AppTab::Skills => render_skills_tab(frame, root[2], app),
        AppTab::Providers => render_providers_tab(frame, root[2], app),
    }

    render_hints_bar(frame, root[3], app);

    // Status toast — floats at the bottom of the content area
    if let Some((msg, is_err)) = &app.status_message.clone() {
        render_status_toast(frame, root[2], msg, *is_err);
    }

    if app.show_help {
        render_help_overlay(frame, area);
    }
}

// ── Title bar ─────────────────────────────────────────────────────────────────

fn render_title_bar(frame: &mut Frame, area: Rect, app: &App) {
    let provider = &app.providers[app.active_provider_idx];
    let provider_label = if provider.is_configured() {
        format!("  {}  ·  {}", provider.display, provider.model)
    } else {
        format!("  {}  ·  ⚠ no key — press 2", provider.display)
    };

    let bar = Line::from(vec![
        Span::styled(
            " SkillForge CLI v1.0 ",
            Style::default()
                .fg(Color::Black)
                .bg(AMBER)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(provider_label, Style::default().fg(DIM_TEXT)),
    ]);
    frame.render_widget(
        Paragraph::new(bar).style(Style::default().bg(Color::Rgb(20, 20, 20))),
        area,
    );
}

// ── Tab bar ───────────────────────────────────────────────────────────────────

fn render_tab_bar(frame: &mut Frame, area: Rect, app: &App) {
    let make_tab = |label: &str, active: bool| {
        if active {
            Span::styled(
                format!("  {}  ", label),
                Style::default()
                    .fg(Color::Black)
                    .bg(GOLD)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(
                format!("  {}  ", label),
                Style::default().fg(DIM_TEXT).bg(Color::Rgb(28, 28, 28)),
            )
        }
    };

    let skills_active = app.active_tab == AppTab::Skills;
    let bar = Line::from(vec![
        Span::raw(" "),
        make_tab("1  Skills", skills_active),
        Span::raw(" "),
        make_tab("2  Providers", !skills_active),
    ]);

    frame.render_widget(
        Paragraph::new(bar).style(Style::default().bg(Color::Rgb(28, 28, 28))),
        area,
    );
}

// ── Skills tab ────────────────────────────────────────────────────────────────

fn render_skills_tab(frame: &mut Frame, area: Rect, app: &mut App) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(area);

    render_tool_list_panel(frame, cols[0], app);
    render_skill_panel(frame, cols[1], app);
}

// ── Tool list panel ───────────────────────────────────────────────────────────

fn render_tool_list_panel(frame: &mut Frame, area: Rect, app: &mut App) {
    let focused = matches!(app.focus, Focus::ToolList | Focus::SearchBar);

    let title = Line::from(vec![Span::styled(" AI Coding Tools ", title_style())]);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style(focused));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // search
            Constraint::Min(0),    // list
            Constraint::Length(3), // button
        ])
        .split(inner);

    render_search_bar(frame, layout[0], app);
    render_tool_list(frame, layout[1], app);
    render_generate_button(frame, layout[2], app);
}

fn render_search_bar(frame: &mut Frame, area: Rect, app: &App) {
    let focused = app.focus == Focus::SearchBar;
    let content = if app.filter.is_empty() {
        Span::styled("/ Filter...", Style::default().fg(DIM_TEXT))
    } else {
        Span::styled(format!("/ {}", app.filter), Style::default().fg(BODY_TEXT))
    };
    let widget = Paragraph::new(content).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style(focused)),
    );
    frame.render_widget(widget, area);

    if focused {
        let cx = area.x + 3 + app.filter.len() as u16;
        frame.set_cursor_position((cx.min(area.x + area.width - 2), area.y + 1));
    }
}

fn render_tool_list(frame: &mut Frame, area: Rect, app: &mut App) {
    let home = std::env::var("HOME").unwrap_or_default();
    let items: Vec<ListItem> = app
        .filtered_indices
        .iter()
        .map(|&i| {
            let tool = &app.tools[i];
            let checked = app.selected_tools.contains(&tool.name);
            let checkbox = if checked { "[x]" } else { "[ ]" };
            let indicator = if tool.has_skill {
                Span::styled(" ✓", Style::default().fg(INSTALLED_GREEN))
            } else {
                Span::raw("  ")
            };
            let name_style = if checked {
                Style::default()
                    .fg(BRIGHT_YELLOW)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(BODY_TEXT)
            };
            let path_str = tool.skill_path.to_str().unwrap_or("").replace(&home, "~");
            let line1 = Line::from(vec![
                Span::styled(format!("{} ", checkbox), Style::default().fg(DIM_TEXT)),
                Span::styled(tool.name.clone(), name_style),
                indicator,
            ]);
            let line2 = Line::from(vec![
                Span::raw("    "),
                Span::styled(path_str, Style::default().fg(DIM_TEXT)),
            ]);
            ListItem::new(ratatui::text::Text::from(vec![line1, line2]))
        })
        .collect();

    let list = List::new(items)
        .highlight_style(
            Style::default()
                .bg(SELECTED_BG)
                .fg(BRIGHT_YELLOW)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    frame.render_stateful_widget(list, area, &mut app.list_state);
}

fn render_generate_button(frame: &mut Frame, area: Rect, app: &App) {
    let count = app.selected_tools.len();
    let label = if count == 0 {
        "  [Enter] Generate Skill".to_string()
    } else {
        format!("  [Enter] Generate {} Skill(s)", count)
    };
    let btn = Paragraph::new(Span::styled(
        label,
        Style::default()
            .fg(Color::Black)
            .bg(AMBER)
            .add_modifier(Modifier::BOLD),
    ))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(AMBER)),
    );
    frame.render_widget(btn, area);
}

// ── Skill panel ───────────────────────────────────────────────────────────────

fn render_skill_panel(frame: &mut Frame, area: Rect, app: &mut App) {
    let focused = matches!(
        app.focus,
        Focus::SkillName | Focus::RequirementInput | Focus::SkillOutput
    );
    let block = Block::default()
        .title(Span::styled(" Skill Generator ", title_style()))
        .borders(Borders::ALL)
        .border_style(border_style(focused));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // skill name input
            Constraint::Min(4),    // requirement input
            Constraint::Min(0),    // skill output
        ])
        .split(inner);

    render_skill_name_input(frame, right[0], app);
    render_requirement_input(frame, right[1], app);
    render_skill_output(frame, right[2], app);
}

fn render_skill_name_input(frame: &mut Frame, area: Rect, app: &App) {
    let focused = app.focus == Focus::SkillName;
    let content = if app.skill_name.is_empty() && !focused {
        Span::styled(
            "optional — defaults to tool name",
            Style::default().fg(DIM_TEXT),
        )
    } else {
        Span::styled(app.skill_name.as_str(), Style::default().fg(BODY_TEXT))
    };
    let widget = Paragraph::new(Line::from(vec![Span::raw(" "), content])).block(
        Block::default()
            .title(Span::styled(" Skill Name ", title_style()))
            .borders(Borders::ALL)
            .border_style(border_style(focused)),
    );
    frame.render_widget(widget, area);

    if focused {
        let cx = area.x + 2 + app.skill_name_cursor as u16;
        frame.set_cursor_position((cx.min(area.x + area.width - 2), area.y + 1));
    }
}

fn render_requirement_input(frame: &mut Frame, area: Rect, app: &App) {
    let focused = app.focus == Focus::RequirementInput;
    let tool_label = app
        .current_tool
        .as_deref()
        .map(|t| format!(" Requirement — {} ", t))
        .unwrap_or_else(|| " Requirement ".to_string());

    let display = if app.requirement.is_empty() && !focused {
        Text::from(Span::styled(
            "Select a tool → Tab here → describe what the skill should do...",
            Style::default().fg(DIM_TEXT),
        ))
    } else {
        Text::from(app.requirement.as_str())
    };

    let widget = Paragraph::new(display)
        .block(
            Block::default()
                .title(Span::styled(tool_label, title_style()))
                .borders(Borders::ALL)
                .border_style(border_style(focused)),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(widget, area);

    if focused {
        let inner_w = (area.width as usize).saturating_sub(2).max(1);
        let x = area.x + 1 + (app.cursor_pos % inner_w) as u16;
        let y = area.y + 1 + (app.cursor_pos / inner_w) as u16;
        frame.set_cursor_position((x, y.min(area.y + area.height - 2)));
    }
}

fn render_skill_output(frame: &mut Frame, area: Rect, app: &mut App) {
    let focused = app.focus == Focus::SkillOutput;

    let action_hint = match &app.state {
        AppState::Generating => " [streaming…]",
        AppState::Ready => " [i]Install  [c]Copy  [r]Regen",
        AppState::Error(_) => " [r]Retry",
        AppState::Idle => "",
    };
    let title = format!(" Generated Skill Output{} ", action_hint);

    let (content, style) = match &app.state {
        AppState::Error(msg) => (msg.clone(), Style::default().fg(ERROR_COLOR)),
        _ => (app.output.clone(), Style::default().fg(BODY_TEXT)),
    };

    let text = if content.is_empty() {
        Text::from(Span::styled(
            "Output will stream here after generation...",
            Style::default().fg(DIM_TEXT),
        ))
    } else {
        Text::styled(content, style)
    };

    let widget = Paragraph::new(text)
        .block(
            Block::default()
                .title(Span::styled(title, title_style()))
                .borders(Borders::ALL)
                .border_style(border_style(focused)),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.output_scroll, 0));
    frame.render_widget(widget, area);

    // Scrollbar when there is scrollable content
    if !app.output.is_empty() {
        let line_count = app.output.lines().count();
        let visible = area.height.saturating_sub(2) as usize;
        if line_count > visible {
            let sb = Scrollbar::new(ScrollbarOrientation::VerticalRight);
            let mut sb_state = ScrollbarState::new(line_count).position(app.output_scroll as usize);
            frame.render_stateful_widget(
                sb,
                area.inner(Margin {
                    horizontal: 0,
                    vertical: 1,
                }),
                &mut sb_state,
            );
        }
    }
}

// ── Providers tab ─────────────────────────────────────────────────────────────

fn render_providers_tab(frame: &mut Frame, area: Rect, app: &mut App) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(area);

    render_provider_list(frame, cols[0], app);
    render_provider_config(frame, cols[1], app);
}

fn render_provider_list(frame: &mut Frame, area: Rect, app: &App) {
    let focused = app.focus == Focus::ProviderList;
    let block = Block::default()
        .title(Span::styled(" Select Provider ", title_style()))
        .borders(Borders::ALL)
        .border_style(border_style(focused));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(2)])
        .split(inner);

    // Provider radio-button list
    let items: Vec<ListItem> = app
        .providers
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let is_active = i == app.active_provider_idx;
            let is_editing = i == app.editing_provider_idx;
            let radio = if is_active { "●" } else { "○" };
            let key_status = if p.is_configured() {
                Span::styled(" ✓", Style::default().fg(ACTIVE_GREEN))
            } else {
                Span::styled(" ✗", Style::default().fg(ERROR_COLOR))
            };
            let name_style = if is_active {
                Style::default()
                    .fg(BRIGHT_YELLOW)
                    .add_modifier(Modifier::BOLD)
            } else if is_editing {
                Style::default().fg(BODY_TEXT)
            } else {
                Style::default().fg(DIM_TEXT)
            };
            let radio_style = if is_active {
                Style::default().fg(AMBER)
            } else {
                Style::default().fg(GREY)
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!(" {} ", radio), radio_style),
                Span::styled(p.display, name_style),
                key_status,
            ]))
        })
        .collect();

    // Highlight the editing row
    let mut list_state = ratatui::widgets::ListState::default();
    list_state.select(Some(app.editing_provider_idx));

    let list = List::new(items).highlight_style(
        Style::default()
            .bg(SELECTED_BG)
            .add_modifier(Modifier::BOLD),
    );

    frame.render_stateful_widget(list, rows[0], &mut list_state);

    // Hint
    let hint = Paragraph::new(Span::styled(
        " ↑↓ Navigate   Enter: Activate",
        Style::default().fg(DIM_TEXT),
    ));
    frame.render_widget(hint, rows[1]);
}

fn render_provider_config(frame: &mut Frame, area: Rect, app: &App) {
    let entry = &app.providers[app.editing_provider_idx];
    let is_active = app.editing_provider_idx == app.active_provider_idx;

    let active_badge = if is_active {
        Span::styled(
            " [ACTIVE] ",
            Style::default()
                .fg(Color::Black)
                .bg(ACTIVE_GREEN)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::raw("")
    };
    let title = Line::from(vec![
        Span::styled(format!(" {} ", entry.display), title_style()),
        active_badge,
    ]);

    let right_focus = matches!(app.focus, Focus::ApiKeyField | Focus::ModelField);
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style(right_focus));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Layout inside config panel
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // spacer
            Constraint::Length(1), // "API Key" label
            Constraint::Length(3), // key input
            Constraint::Length(1), // spacer
            Constraint::Length(1), // "Model" label
            Constraint::Length(3), // model input
            Constraint::Length(1), // spacer
            Constraint::Length(1), // env var
            Constraint::Length(1), // status
            Constraint::Min(0),    // fill
            Constraint::Length(1), // hint
        ])
        .split(inner);

    // ── API Key label ──
    frame.render_widget(
        Paragraph::new(Span::styled(
            " API Key  (Ctrl+H to toggle visibility)",
            Style::default().fg(DIM_TEXT),
        )),
        rows[1],
    );

    // ── API Key input ──
    let key_focused = app.focus == Focus::ApiKeyField;
    let key_display = if entry.show_key || key_focused {
        // Show actual characters (masked in focused field)
        if key_focused {
            "*".repeat(entry.api_key.len())
        } else {
            entry.display_key()
        }
    } else {
        entry.display_key()
    };

    // When focused and show_key is enabled, show plain text
    let key_display = if key_focused && entry.show_key {
        entry.api_key.clone()
    } else {
        key_display
    };

    let key_widget = Paragraph::new(Span::styled(
        format!(" {}", key_display),
        Style::default().fg(if key_focused {
            BRIGHT_YELLOW
        } else {
            BODY_TEXT
        }),
    ))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style(key_focused)),
    );
    frame.render_widget(key_widget, rows[2]);

    if key_focused {
        // Cursor position: 1 (border+space) + cursor_pos
        let display_cursor = if entry.show_key {
            app.key_cursor
        } else {
            app.key_cursor // same position, just masked characters
        };
        let cx = rows[2].x + 2 + display_cursor as u16;
        frame.set_cursor_position((cx.min(rows[2].x + rows[2].width - 2), rows[2].y + 1));
    }

    // ── Model label ──
    let model_label = if entry.models_loading {
        " Model  (fetching…)"
    } else if !entry.available_models.is_empty() {
        " Model  (◀/▶ to navigate)"
    } else {
        " Model"
    };
    frame.render_widget(
        Paragraph::new(Span::styled(model_label, Style::default().fg(DIM_TEXT))),
        rows[4],
    );

    // ── Model input / navigator ──
    let model_focused = app.focus == Focus::ModelField;
    let model_widget = if entry.models_loading {
        // Loading spinner placeholder
        Paragraph::new(Span::styled(
            " ⏳ Loading models…",
            Style::default().fg(DIM_TEXT),
        ))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style(model_focused)),
        )
    } else if !entry.available_models.is_empty() {
        // Arrow navigator: ◀ model-name ▶  (n/total)
        let total = entry.available_models.len();
        let idx = entry.model_idx.min(total.saturating_sub(1));
        let nav_text = format!(" ◀  {}  ▶   ({}/{})", entry.model, idx + 1, total);
        Paragraph::new(Span::styled(
            nav_text,
            Style::default().fg(if model_focused {
                BRIGHT_YELLOW
            } else {
                BODY_TEXT
            }),
        ))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style(model_focused)),
        )
    } else {
        // Plain text input (no models fetched yet)
        Paragraph::new(Span::styled(
            format!(" {}", entry.model),
            Style::default().fg(if model_focused {
                BRIGHT_YELLOW
            } else {
                BODY_TEXT
            }),
        ))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style(model_focused)),
        )
    };
    frame.render_widget(model_widget, rows[5]);

    // Only show text cursor in plain-text editing mode
    if model_focused && entry.available_models.is_empty() && !entry.models_loading {
        let cx = rows[5].x + 2 + app.model_cursor as u16;
        frame.set_cursor_position((cx.min(rows[5].x + rows[5].width - 2), rows[5].y + 1));
    }

    // ── Env var info ──
    frame.render_widget(
        Paragraph::new(Span::styled(
            format!(" Env var: {}", entry.env_var),
            Style::default().fg(DIM_TEXT),
        )),
        rows[7],
    );

    // ── Status ──
    let status = if entry.is_configured() {
        Span::styled(
            " Status:  ✓ Key configured",
            Style::default().fg(ACTIVE_GREEN),
        )
    } else {
        Span::styled(" Status:  ✗ No key set", Style::default().fg(ERROR_COLOR))
    };
    frame.render_widget(Paragraph::new(status), rows[8]);

    // ── Hints ──
    frame.render_widget(
        Paragraph::new(Span::styled(
            " Tab: Switch field   Enter: Save & Activate   Ctrl+H: Toggle key visibility",
            Style::default().fg(DIM_TEXT),
        )),
        rows[10],
    );
}

// ── Hints bar ─────────────────────────────────────────────────────────────────

fn render_hints_bar(frame: &mut Frame, area: Rect, app: &App) {
    let model_hint = {
        let entry = &app.providers[app.editing_provider_idx];
        if !entry.available_models.is_empty() {
            "◀/▶: Select model   Enter: Save&Activate   Tab: back to list   Esc: Back"
        } else {
            "Type model name   Enter: Save&Activate   Tab: back to list   Esc: Back"
        }
    };

    let hints = match (&app.active_tab, &app.focus) {
        (AppTab::Skills, Focus::ToolList) => {
            "1/2: Tabs  Tab: Panel  ↑↓: Nav  Space: Toggle  /: Filter  Enter: Generate  q: Quit  ?: Help"
        }
        (AppTab::Skills, Focus::SearchBar) => {
            "Type to filter   Enter/Tab: back to list   Esc: clear   1/2: Tabs"
        }
        (AppTab::Skills, Focus::SkillName) => {
            "Type skill file name   Tab: Requirement   Esc: Clear"
        }
        (AppTab::Skills, Focus::RequirementInput) => {
            "Enter: Generate   Tab: Panel   Esc: Clear   1/2: Tabs"
        }
        (AppTab::Skills, Focus::SkillOutput) => {
            "i: Install   c: Copy   r: Regen   ↑↓: Scroll   Tab: Panel   1/2: Tabs"
        }
        (AppTab::Providers, Focus::ProviderList) => {
            "1/2: Tabs  ↑↓: Navigate  Enter: Activate+Configure  Tab: API Key field  q: Quit"
        }
        (AppTab::Providers, Focus::ApiKeyField) => {
            "Type API key   Enter: Save&Activate   Tab: Model field   Ctrl+H: Toggle visibility   Esc: Back"
        }
        (AppTab::Providers, Focus::ModelField) => model_hint,
        _ => "1: Skills   2: Providers   Tab: Panel   q: Quit   ?: Help",
    };

    frame.render_widget(
        Paragraph::new(Span::styled(hints, Style::default().fg(DIM_TEXT)))
            .alignment(Alignment::Center),
        area,
    );
}

// ── Status toast (floats at bottom of content area) ───────────────────────────

fn render_status_toast(frame: &mut Frame, area: Rect, msg: &str, is_error: bool) {
    if area.height < 3 {
        return;
    }
    let toast_area = Rect {
        x: area.x + 2,
        y: area.y + area.height - 1,
        width: area.width.saturating_sub(4),
        height: 1,
    };
    let style = if is_error {
        Style::default().fg(ERROR_COLOR)
    } else {
        Style::default().fg(INSTALLED_GREEN)
    };
    let prefix = if is_error { "✗ " } else { "✓ " };
    frame.render_widget(
        Paragraph::new(Span::styled(format!("{}{}", prefix, msg), style))
            .alignment(Alignment::Center),
        toast_area,
    );
}

// ── Help overlay ──────────────────────────────────────────────────────────────

fn render_help_overlay(frame: &mut Frame, area: Rect) {
    let popup = centered_rect(62, 80, area);
    frame.render_widget(Clear, popup);

    let lines = vec![
        Line::from(Span::styled(
            "  Keyboard Shortcuts ",
            Style::default()
                .fg(Color::Black)
                .bg(AMBER)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Global",
            Style::default().fg(AMBER).add_modifier(Modifier::BOLD),
        )),
        hint_line("1 / 2", "Switch between Skills / Providers tab"),
        hint_line("Tab / Shift+Tab", "Cycle panel focus"),
        hint_line("q", "Quit"),
        hint_line("?", "Toggle this help"),
        Line::from(""),
        Line::from(Span::styled(
            "  Skills tab",
            Style::default().fg(AMBER).add_modifier(Modifier::BOLD),
        )),
        hint_line("↑↓ / j k", "Navigate tool list"),
        hint_line("Space", "Toggle tool selection"),
        hint_line("/", "Jump to filter bar"),
        hint_line("Tab", "Cycle: Tools → Skill Name → Req → Output"),
        hint_line("Skill Name", "Optional filename (defaults to tool name)"),
        hint_line("Enter", "Generate skill for selected tool"),
        hint_line("i", "Install generated skill to disk"),
        hint_line("c", "Copy skill to clipboard"),
        hint_line("r", "Regenerate with same requirement"),
        hint_line("↑↓ (Output)", "Scroll generated output"),
        Line::from(""),
        Line::from(Span::styled(
            "  Providers tab",
            Style::default().fg(AMBER).add_modifier(Modifier::BOLD),
        )),
        hint_line("↑↓", "Navigate provider list"),
        hint_line("Enter", "Activate provider + open config"),
        hint_line("Tab", "Switch between API key / model fields"),
        hint_line("Enter (field)", "Save & activate provider"),
        hint_line("◀/▶ (Model)", "Cycle through fetched models"),
        hint_line("Ctrl+H", "Toggle API key visibility"),
        Line::from(""),
        Line::from(Span::styled(
            "  Press ? or Esc to close",
            Style::default().fg(DIM_TEXT),
        )),
    ];

    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .title(Span::styled(" Help ", title_style()))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(AMBER)),
            )
            .wrap(Wrap { trim: false }),
        popup,
    );
}

fn hint_line<'a>(key: &'a str, desc: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("  {:<16}", key), Style::default().fg(BRIGHT_YELLOW)),
        Span::styled(desc, Style::default().fg(BODY_TEXT)),
    ])
}

// ── Utilities ─────────────────────────────────────────────────────────────────

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vert[1])[1]
}
