mod tui;

use anyhow::{Context, Result, anyhow, bail};
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const APP_NAME: &str = "gwtm";
const DEFAULT_IDE_MODE: &str = "system";
const DEFAULT_IDE_COMMAND: &str = "open";
const DEFAULT_IDE_LABEL: &str = "System Default";
const PROMPT_IDE_MODE: &str = "prompt";
const PROMPT_IDE_LABEL: &str = "稍后再设置";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const HOMEPAGE: &str = env!("CARGO_PKG_HOMEPAGE");

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppConfig {
    projects_root_dirs: Vec<PathBuf>,
    worktrees_root_dir: PathBuf,
    ide_mode: String,
    ide_command: String,
    ide_label: String,
}

impl AppConfig {
    fn default_with_paths(projects_root_dir: PathBuf, worktrees_root_dir: PathBuf) -> Self {
        Self {
            projects_root_dirs: vec![projects_root_dir],
            worktrees_root_dir,
            ide_mode: DEFAULT_IDE_MODE.to_string(),
            ide_command: DEFAULT_IDE_COMMAND.to_string(),
            ide_label: DEFAULT_IDE_LABEL.to_string(),
        }
    }

    fn primary_projects_root(&self) -> &Path {
        self.projects_root_dirs
            .first()
            .map(PathBuf::as_path)
            .unwrap_or_else(|| Path::new("."))
    }
}

#[derive(Debug, Clone)]
struct Project {
    name: String,
    display_name: String,
    path: PathBuf,
    source_root: PathBuf,
}

#[derive(Debug, Clone)]
struct WorktreeEntry {
    path: PathBuf,
    head: String,
    branch: Option<String>,
    bare: bool,
}

#[derive(Debug, Clone)]
struct IdeOption {
    mode: String,
    command: String,
    label: String,
    detail: String,
}

#[derive(Clone, Copy)]
enum ProjectIntent {
    Create,
    Open,
    List,
    Remove,
}

enum Page {
    MainMenu(tui::MenuState),
    ProjectSelect {
        intent: ProjectIntent,
        menu: tui::MenuState,
    },
    BranchInput {
        project_idx: usize,
        input: tui::InputState,
    },
    ConfigProjectsRoots {
        roots: Vec<PathBuf>,
        initial_setup: bool,
        menu: tui::MenuState,
    },
    ConfigProjectRootActions {
        roots: Vec<PathBuf>,
        root_idx: usize,
        initial_setup: bool,
        menu: tui::MenuState,
    },
    ConfigProjectRootInput {
        roots: Vec<PathBuf>,
        edit_idx: Option<usize>,
        initial_setup: bool,
        input: tui::InputState,
    },
    ConfigWorktreesRoot {
        project_roots: Vec<PathBuf>,
        input: tui::InputState,
        initial_setup: bool,
    },
    ConfigIdeSelect {
        project_roots: Vec<PathBuf>,
        worktrees_root: PathBuf,
        initial_setup: bool,
        options: Vec<IdeOption>,
        menu: tui::MenuState,
    },
    BaseBranchSelect {
        project_idx: usize,
        new_branch: String,
        base_branches: Vec<String>,
        menu: tui::MenuState,
    },
    ConfirmOpenIde {
        worktree_path: PathBuf,
        lines: Vec<String>,
        menu: tui::MenuState,
    },
    ChooseIdeForOpen {
        result_title: String,
        result_subtitle: String,
        worktree_path: PathBuf,
        lines: Vec<String>,
        options: Vec<IdeOption>,
        menu: tui::MenuState,
    },
    SaveDefaultIdeConfirm {
        result_title: String,
        result_subtitle: String,
        lines: Vec<String>,
        selected_ide: IdeOption,
        menu: tui::MenuState,
    },
    OpenWorktreeSelect {
        project_idx: usize,
        worktrees: Vec<WorktreeEntry>,
        menu: tui::MenuState,
    },
    RemoveWorktreeSelect {
        project_idx: usize,
        removable: Vec<WorktreeEntry>,
        menu: tui::MenuState,
    },
    RemoveLocalBranchConfirm {
        project_idx: usize,
        selected: WorktreeEntry,
        menu: tui::MenuState,
    },
    RemoveRemoteBranchConfirm {
        project_idx: usize,
        selected: WorktreeEntry,
        delete_local_branch: bool,
        menu: tui::MenuState,
    },
    Result(tui::ResultState),
}

enum LoopAction {
    None,
    Push(Page),
    Pop,
    ReplaceConfigRoots {
        roots: Vec<PathBuf>,
        initial_setup: bool,
    },
    ResetToMain,
    Exit,
}

struct FullScreenApp {
    config: AppConfig,
    config_path: PathBuf,
    projects: Vec<Project>,
    start_with_setup: bool,
}

impl FullScreenApp {
    fn new(config: AppConfig, config_path: PathBuf) -> Self {
        Self {
            config,
            config_path,
            projects: Vec::new(),
            start_with_setup: false,
        }
    }

    fn new_for_setup(config: AppConfig, config_path: PathBuf) -> Self {
        Self {
            config,
            config_path,
            projects: Vec::new(),
            start_with_setup: true,
        }
    }

    fn run(&mut self) -> Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = self.main_loop(&mut terminal);

        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        result
    }

    fn main_loop(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        let mut pages = if self.start_with_setup {
            vec![self.config_projects_root_page(true)]
        } else {
            vec![Page::MainMenu(self.main_menu_page())]
        };

        loop {
            let Some(page) = pages.last_mut() else {
                return Ok(());
            };

            let action = match page {
                Page::MainMenu(menu) => {
                    terminal.draw(|frame| menu.render(frame))?;
                    match menu.handle_key_event() {
                        Some(tui::MenuAction::Select(0)) => {
                            match self.project_select_page(ProjectIntent::Create) {
                                Ok(page) => LoopAction::Push(page),
                                Err(err) => {
                                    LoopAction::Push(self.error_result_page("项目扫描失败", err))
                                }
                            }
                        }
                        Some(tui::MenuAction::Select(1)) => {
                            match self.project_select_page(ProjectIntent::Open) {
                                Ok(page) => LoopAction::Push(page),
                                Err(err) => {
                                    LoopAction::Push(self.error_result_page("项目扫描失败", err))
                                }
                            }
                        }
                        Some(tui::MenuAction::Select(2)) => {
                            match self.project_select_page(ProjectIntent::List) {
                                Ok(page) => LoopAction::Push(page),
                                Err(err) => {
                                    LoopAction::Push(self.error_result_page("项目扫描失败", err))
                                }
                            }
                        }
                        Some(tui::MenuAction::Select(3)) => {
                            match self.project_select_page(ProjectIntent::Remove) {
                                Ok(page) => LoopAction::Push(page),
                                Err(err) => {
                                    LoopAction::Push(self.error_result_page("项目扫描失败", err))
                                }
                            }
                        }
                        Some(tui::MenuAction::Select(4)) => {
                            LoopAction::Push(self.config_projects_root_page(false))
                        }
                        Some(tui::MenuAction::Select(5))
                        | Some(tui::MenuAction::Back)
                        | Some(tui::MenuAction::Quit) => LoopAction::Exit,
                        _ => LoopAction::None,
                    }
                }
                Page::ProjectSelect { intent, menu } => {
                    terminal.draw(|frame| menu.render(frame))?;
                    match menu.handle_key_event() {
                        Some(tui::MenuAction::Select(index)) => match *intent {
                            ProjectIntent::Create => LoopAction::Push(Page::BranchInput {
                                project_idx: index,
                                input: tui::InputState::new(
                                    "gwtm / 新分支",
                                    "输入要创建的 worktree 分支名",
                                    format!("项目: {}", self.projects[index].name).as_str(),
                                    "feat/my-feature",
                                ),
                            }),
                            ProjectIntent::Open => match self.open_worktree_page(index) {
                                Ok(page) => LoopAction::Push(page),
                                Err(err) => LoopAction::Push(
                                    self.error_result_page("读取 worktree 失败", err),
                                ),
                            },
                            ProjectIntent::List => match self.worktree_list_page(index) {
                                Ok(page) => LoopAction::Push(page),
                                Err(err) => LoopAction::Push(
                                    self.error_result_page("读取 worktree 失败", err),
                                ),
                            },
                            ProjectIntent::Remove => match self.remove_worktree_page(index) {
                                Ok(page) => LoopAction::Push(page),
                                Err(err) => LoopAction::Push(
                                    self.error_result_page("读取 worktree 失败", err),
                                ),
                            },
                        },
                        Some(tui::MenuAction::Back) => LoopAction::Pop,
                        Some(tui::MenuAction::Quit) => LoopAction::Exit,
                        _ => LoopAction::None,
                    }
                }
                Page::BranchInput { project_idx, input } => {
                    terminal.draw(|frame| input.render(frame))?;
                    match input.handle_key_event() {
                        Some(tui::InputAction::Submit(branch_name)) => {
                            match self.base_branch_page(*project_idx, branch_name) {
                                Ok(page) => LoopAction::Push(page),
                                Err(err) => LoopAction::Push(
                                    self.error_result_page("读取远程分支失败", err),
                                ),
                            }
                        }
                        Some(tui::InputAction::PickFolder) => LoopAction::None,
                        Some(tui::InputAction::Back) => LoopAction::Pop,
                        Some(tui::InputAction::Quit) => LoopAction::Exit,
                        None => LoopAction::None,
                    }
                }
                Page::ConfigProjectsRoots {
                    roots,
                    initial_setup,
                    menu,
                } => {
                    terminal.draw(|frame| menu.render(frame))?;
                    match menu.handle_key_event() {
                        Some(tui::MenuAction::Select(index)) => {
                            let root_count = roots.len();
                            if index < root_count {
                                LoopAction::Push(self.config_project_root_actions_page(
                                    roots.clone(),
                                    index,
                                    *initial_setup,
                                ))
                            } else if index == root_count {
                                LoopAction::Push(self.config_project_root_input_page(
                                    roots.clone(),
                                    None,
                                    *initial_setup,
                                ))
                            } else {
                                LoopAction::Push(
                                    self.config_worktrees_root_page(roots.clone(), *initial_setup),
                                )
                            }
                        }
                        Some(tui::MenuAction::Back) => {
                            if *initial_setup {
                                LoopAction::Exit
                            } else {
                                LoopAction::Pop
                            }
                        }
                        Some(tui::MenuAction::Quit) => LoopAction::Exit,
                        _ => LoopAction::None,
                    }
                }
                Page::ConfigProjectRootActions {
                    roots,
                    root_idx,
                    initial_setup,
                    menu,
                } => {
                    terminal.draw(|frame| menu.render(frame))?;
                    match menu.handle_key_event() {
                        Some(tui::MenuAction::Select(0)) => LoopAction::Push(
                            self.config_project_root_input_page(
                                roots.clone(),
                                Some(*root_idx),
                                *initial_setup,
                            ),
                        ),
                        Some(tui::MenuAction::Select(1)) if roots.len() > 1 => {
                            let mut next_roots = roots.clone();
                            next_roots.remove(*root_idx);
                            LoopAction::ReplaceConfigRoots {
                                roots: next_roots,
                                initial_setup: *initial_setup,
                            }
                        }
                        Some(tui::MenuAction::Back) => LoopAction::Pop,
                        Some(tui::MenuAction::Quit) => LoopAction::Exit,
                        _ => LoopAction::None,
                    }
                }
                Page::ConfigProjectRootInput {
                    roots,
                    edit_idx,
                    initial_setup,
                    input,
                } => {
                    terminal.draw(|frame| input.render(frame))?;
                    match input.handle_key_event() {
                        Some(tui::InputAction::Submit(value)) => {
                            match validate_directory_input(&value, true) {
                                Ok(projects_root) => {
                                    let mut next_roots = roots.clone();
                                    if let Some(index) = edit_idx {
                                        next_roots[*index] = projects_root;
                                    } else {
                                        next_roots.push(projects_root);
                                    }
                                    LoopAction::ReplaceConfigRoots {
                                        roots: dedupe_project_roots(next_roots),
                                        initial_setup: *initial_setup,
                                    }
                                }
                                Err(err) => {
                                    input.error = Some(err.to_string());
                                    LoopAction::None
                                }
                            }
                        }
                        Some(tui::InputAction::PickFolder) => {
                            if let Some(path) = choose_folder_with_dialog("请选择项目根目录") {
                                input.value = path.to_string_lossy().to_string();
                                input.cursor_pos = input.value.len();
                                input.error = None;
                            }
                            LoopAction::None
                        }
                        Some(tui::InputAction::Back) => LoopAction::Pop,
                        Some(tui::InputAction::Quit) => LoopAction::Exit,
                        None => LoopAction::None,
                    }
                }
                Page::ConfigWorktreesRoot {
                    project_roots,
                    input,
                    initial_setup,
                } => {
                    terminal.draw(|frame| input.render(frame))?;
                    match input.handle_key_event() {
                        Some(tui::InputAction::Submit(value)) => {
                            match validate_directory_input(&value, false) {
                                Ok(worktrees_root) => match self.config_ide_page(
                                    project_roots.clone(),
                                    worktrees_root,
                                    *initial_setup,
                                ) {
                                    Ok(page) => LoopAction::Push(page),
                                    Err(err) => LoopAction::Push(
                                        self.error_result_page("检测 IDE 失败", err),
                                    ),
                                },
                                Err(err) => {
                                    input.error = Some(err.to_string());
                                    LoopAction::None
                                }
                            }
                        }
                        Some(tui::InputAction::PickFolder) => {
                            if let Some(path) = choose_folder_with_dialog("请选择 Worktree 根目录")
                            {
                                input.value = path.to_string_lossy().to_string();
                                input.cursor_pos = input.value.len();
                                input.error = None;
                            }
                            LoopAction::None
                        }
                        Some(tui::InputAction::Back) => LoopAction::Pop,
                        Some(tui::InputAction::Quit) => {
                            if *initial_setup {
                                LoopAction::Exit
                            } else {
                                LoopAction::ResetToMain
                            }
                        }
                        None => LoopAction::None,
                    }
                }
                Page::ConfigIdeSelect {
                    project_roots,
                    worktrees_root,
                    initial_setup,
                    options,
                    menu,
                } => {
                    terminal.draw(|frame| menu.render(frame))?;
                    match menu.handle_key_event() {
                        Some(tui::MenuAction::Select(index)) => {
                            match self.apply_config(
                                project_roots.clone(),
                                worktrees_root.clone(),
                                options[index].clone(),
                            ) {
                                Ok(lines) => LoopAction::Push(Page::Result(tui::ResultState::new(
                                    "gwtm / 配置结果",
                                    if *initial_setup {
                                        "初始化完成"
                                    } else {
                                        "配置已更新"
                                    },
                                    lines,
                                ))),
                                Err(err) => {
                                    LoopAction::Push(self.error_result_page("保存配置失败", err))
                                }
                            }
                        }
                        Some(tui::MenuAction::Back) => LoopAction::Pop,
                        Some(tui::MenuAction::Quit) => {
                            if *initial_setup {
                                LoopAction::Exit
                            } else {
                                LoopAction::ResetToMain
                            }
                        }
                        None => LoopAction::None,
                    }
                }
                Page::BaseBranchSelect {
                    project_idx,
                    new_branch,
                    base_branches,
                    menu,
                } => {
                    terminal.draw(|frame| menu.render(frame))?;
                    match menu.handle_key_event() {
                        Some(tui::MenuAction::Select(index)) => {
                            match self.create_worktree_with_lines(
                                *project_idx,
                                new_branch.clone(),
                                base_branches[index].clone(),
                            ) {
                                Ok((lines, worktree_path)) => LoopAction::Push(
                                    self.confirm_open_ide_page(worktree_path, lines),
                                ),
                                Err(err) => LoopAction::Push(
                                    self.error_result_page("创建 worktree 失败", err),
                                ),
                            }
                        }
                        Some(tui::MenuAction::Back) => LoopAction::Pop,
                        Some(tui::MenuAction::Quit) => LoopAction::Exit,
                        _ => LoopAction::None,
                    }
                }
                Page::ConfirmOpenIde {
                    worktree_path,
                    lines,
                    menu,
                } => {
                    terminal.draw(|frame| menu.render(frame))?;
                    match menu.handle_key_event() {
                        Some(tui::MenuAction::Select(0)) => {
                            if self.ide_is_configured() {
                                let mut result_lines = lines.clone();
                                match open_with_ide(&self.config, worktree_path) {
                                    Ok(()) => result_lines.push(format!(
                                        "[SUCCESS] 已触发 {} 打开项目: {}",
                                        self.config.ide_label,
                                        worktree_path.display()
                                    )),
                                    Err(err) => {
                                        result_lines.push(format!("[WARNING] 打开 IDE 失败: {err}"))
                                    }
                                }
                                LoopAction::Push(Page::Result(tui::ResultState::new(
                                    "gwtm / 创建结果",
                                    "Worktree 已创建",
                                    result_lines,
                                )))
                            } else {
                                match self.ide_selection_page_for_open(
                                    "gwtm / 选择 IDE",
                                    "当前未设置默认 IDE，选择一个打开刚创建的 worktree",
                                    "gwtm / 创建结果",
                                    "Worktree 已创建",
                                    worktree_path.clone(),
                                    lines.clone(),
                                ) {
                                    Ok(page) => LoopAction::Push(page),
                                    Err(err) => LoopAction::Push(
                                        self.error_result_page("检测 IDE 失败", err),
                                    ),
                                }
                            }
                        }
                        Some(tui::MenuAction::Select(1)) | Some(tui::MenuAction::Back) => {
                            LoopAction::Push(Page::Result(tui::ResultState::new(
                                "gwtm / 创建结果",
                                "Worktree 已创建",
                                lines.clone(),
                            )))
                        }
                        Some(tui::MenuAction::Quit) => LoopAction::Exit,
                        _ => LoopAction::None,
                    }
                }
                Page::ChooseIdeForOpen {
                    result_title,
                    result_subtitle,
                    worktree_path,
                    lines,
                    options,
                    menu,
                } => {
                    terminal.draw(|frame| menu.render(frame))?;
                    match menu.handle_key_event() {
                        Some(tui::MenuAction::Select(index)) => {
                            let selected = options[index].clone();
                            if selected.mode == PROMPT_IDE_MODE {
                                let mut result_lines = lines.clone();
                                result_lines.push(
                                    "[INFO] 本次未打开 IDE，可稍后在“重新配置”中设置默认 IDE。"
                                        .to_string(),
                                );
                                LoopAction::Push(Page::Result(tui::ResultState::new(
                                    result_title,
                                    result_subtitle,
                                    result_lines,
                                )))
                            } else {
                                let mut result_lines = lines.clone();
                                match open_with_ide_option(&selected, worktree_path) {
                                    Ok(()) => {
                                        result_lines.push(format!(
                                            "[SUCCESS] 已触发 {} 打开项目: {}",
                                            selected.label,
                                            worktree_path.display()
                                        ));
                                        LoopAction::Push(Page::SaveDefaultIdeConfirm {
                                            result_title: result_title.clone(),
                                            result_subtitle: result_subtitle.clone(),
                                            lines: result_lines,
                                            selected_ide: selected,
                                            menu: tui::MenuState::new(
                                                "gwtm / 保存默认 IDE",
                                                format!(
                                                    "是否将 {} 保存为默认 IDE？",
                                                    options[index].label
                                                )
                                                .as_str(),
                                                vec!["是".to_string(), "否".to_string()],
                                            )
                                            .with_details(vec![
                                                vec![
                                                    "后续创建或打开 worktree 时会默认使用这个 IDE。"
                                                        .to_string(),
                                                ],
                                                vec!["仅本次使用，不修改配置文件。".to_string()],
                                            ]),
                                        })
                                    }
                                    Err(err) => {
                                        result_lines
                                            .push(format!("[WARNING] 打开 IDE 失败: {err}"));
                                        LoopAction::Push(Page::Result(tui::ResultState::new(
                                            result_title,
                                            result_subtitle,
                                            result_lines,
                                        )))
                                    }
                                }
                            }
                        }
                        Some(tui::MenuAction::Back) => LoopAction::Pop,
                        Some(tui::MenuAction::Quit) => LoopAction::Exit,
                        _ => LoopAction::None,
                    }
                }
                Page::SaveDefaultIdeConfirm {
                    result_title,
                    result_subtitle,
                    lines,
                    selected_ide,
                    menu,
                } => {
                    terminal.draw(|frame| menu.render(frame))?;
                    match menu.handle_key_event() {
                        Some(tui::MenuAction::Select(0)) => {
                            let mut result_lines = lines.clone();
                            match self.save_ide_preference(selected_ide.clone()) {
                                Ok(()) => result_lines.push(format!(
                                    "[SUCCESS] 已将 {} 保存为默认 IDE",
                                    selected_ide.label
                                )),
                                Err(err) => {
                                    result_lines.push(format!("[WARNING] 保存默认 IDE 失败: {err}"))
                                }
                            }
                            LoopAction::Push(Page::Result(tui::ResultState::new(
                                result_title,
                                result_subtitle,
                                result_lines,
                            )))
                        }
                        Some(tui::MenuAction::Select(1)) | Some(tui::MenuAction::Back) => {
                            let mut result_lines = lines.clone();
                            result_lines.push("[INFO] 未修改默认 IDE 配置".to_string());
                            LoopAction::Push(Page::Result(tui::ResultState::new(
                                result_title,
                                result_subtitle,
                                result_lines,
                            )))
                        }
                        Some(tui::MenuAction::Quit) => LoopAction::Exit,
                        _ => LoopAction::None,
                    }
                }
                Page::OpenWorktreeSelect {
                    project_idx,
                    worktrees,
                    menu,
                } => {
                    terminal.draw(|frame| menu.render(frame))?;
                    match menu.handle_key_event() {
                        Some(tui::MenuAction::Select(index)) => match self
                            .open_worktree_action(*project_idx, worktrees[index].clone())
                        {
                            Ok(action) => action,
                            Err(err) => {
                                LoopAction::Push(self.error_result_page("打开 worktree 失败", err))
                            }
                        },
                        Some(tui::MenuAction::Back) => LoopAction::Pop,
                        Some(tui::MenuAction::Quit) => LoopAction::Exit,
                        _ => LoopAction::None,
                    }
                }
                Page::RemoveWorktreeSelect {
                    project_idx,
                    removable,
                    menu,
                } => {
                    terminal.draw(|frame| menu.render(frame))?;
                    match menu.handle_key_event() {
                        Some(tui::MenuAction::Select(index)) => {
                            let selected = removable[index].clone();
                            if selected.branch.is_some() {
                                LoopAction::Push(
                                    self.remove_local_branch_confirm_page(*project_idx, selected),
                                )
                            } else {
                                match self.remove_worktree_with_lines(
                                    *project_idx,
                                    selected,
                                    false,
                                    false,
                                ) {
                                    Ok(lines) => {
                                        LoopAction::Push(Page::Result(tui::ResultState::new(
                                            "gwtm / 删除结果",
                                            "Worktree 已删除",
                                            lines,
                                        )))
                                    }
                                    Err(err) => LoopAction::Push(
                                        self.error_result_page("删除 worktree 失败", err),
                                    ),
                                }
                            }
                        }
                        Some(tui::MenuAction::Back) => LoopAction::Pop,
                        Some(tui::MenuAction::Quit) => LoopAction::Exit,
                        _ => LoopAction::None,
                    }
                }
                Page::RemoveLocalBranchConfirm {
                    project_idx,
                    selected,
                    menu,
                } => {
                    terminal.draw(|frame| menu.render(frame))?;
                    match menu.handle_key_event() {
                        Some(tui::MenuAction::Select(index)) => {
                            let delete_local_branch = index == 0;
                            match self.next_remove_step(
                                *project_idx,
                                selected.clone(),
                                delete_local_branch,
                            ) {
                                Ok(next_page) => LoopAction::Push(next_page),
                                Err(err) => LoopAction::Push(
                                    self.error_result_page("删除 worktree 失败", err),
                                ),
                            }
                        }
                        Some(tui::MenuAction::Back) => LoopAction::Pop,
                        Some(tui::MenuAction::Quit) => LoopAction::Exit,
                        _ => LoopAction::None,
                    }
                }
                Page::RemoveRemoteBranchConfirm {
                    project_idx,
                    selected,
                    delete_local_branch,
                    menu,
                } => {
                    terminal.draw(|frame| menu.render(frame))?;
                    match menu.handle_key_event() {
                        Some(tui::MenuAction::Select(index)) => {
                            let delete_remote_branch = index == 0;
                            match self.remove_worktree_with_lines(
                                *project_idx,
                                selected.clone(),
                                *delete_local_branch,
                                delete_remote_branch,
                            ) {
                                Ok(lines) => LoopAction::Push(Page::Result(tui::ResultState::new(
                                    "gwtm / 删除结果",
                                    "删除流程已完成",
                                    lines,
                                ))),
                                Err(err) => LoopAction::Push(
                                    self.error_result_page("删除 worktree 失败", err),
                                ),
                            }
                        }
                        Some(tui::MenuAction::Back) => LoopAction::Pop,
                        Some(tui::MenuAction::Quit) => LoopAction::Exit,
                        _ => LoopAction::None,
                    }
                }
                Page::Result(result) => {
                    terminal.draw(|frame| result.render(frame))?;
                    match result.handle_key_event() {
                        Some(tui::ResultAction::Back) => LoopAction::ResetToMain,
                        Some(tui::ResultAction::Quit) => LoopAction::Exit,
                        None => LoopAction::None,
                    }
                }
            };

            match action {
                LoopAction::None => {}
                LoopAction::Push(page) => pages.push(page),
                LoopAction::Pop => {
                    pages.pop();
                    if pages.is_empty() {
                        return Ok(());
                    }
                }
                LoopAction::ReplaceConfigRoots {
                    roots,
                    initial_setup,
                } => {
                    while matches!(
                        pages.last(),
                        Some(
                            Page::ConfigProjectsRoots { .. }
                                | Page::ConfigProjectRootActions { .. }
                                | Page::ConfigProjectRootInput { .. }
                        )
                    ) {
                        pages.pop();
                    }
                    pages.push(self.config_projects_roots_page(roots, initial_setup));
                }
                LoopAction::ResetToMain => {
                    if matches!(pages.first(), Some(Page::MainMenu(_))) {
                        pages.truncate(1);
                    } else {
                        pages.clear();
                        pages.push(Page::MainMenu(self.main_menu_page()));
                    }
                    self.start_with_setup = false;
                }
                LoopAction::Exit => return Ok(()),
            }
        }
    }

    fn main_menu_page(&self) -> tui::MenuState {
        tui::MenuState::new(
            "gwtm",
            "Git worktree manager",
            vec![
                "创建 Worktree".to_string(),
                "打开 Worktree".to_string(),
                "列出 Worktree".to_string(),
                "删除 Worktree".to_string(),
                "重新配置".to_string(),
                "退出程序".to_string(),
            ],
        )
        .with_details(vec![
            vec!["为某个仓库创建新的 worktree 与远程分支。".to_string()],
            vec!["打开已有 worktree 或主仓库到配置的 IDE。".to_string()],
            vec!["查看一个仓库当前已有的 worktree 列表。".to_string()],
            vec!["删除已有 worktree，并可选删除本地/远程分支。".to_string()],
            vec!["重新设置项目根目录、worktree 根目录和 IDE。".to_string()],
            vec!["结束 gwtm。".to_string()],
        ])
    }

    fn config_projects_root_page(&self, initial_setup: bool) -> Page {
        self.config_projects_roots_page(self.config.projects_root_dirs.clone(), initial_setup)
    }

    fn config_projects_roots_page(&self, roots: Vec<PathBuf>, initial_setup: bool) -> Page {
        let subtitle = if initial_setup {
            "首次启动：管理一个或多个项目根目录"
        } else {
            "管理项目根目录列表"
        };
        let mut items: Vec<String> = roots
            .iter()
            .enumerate()
            .map(|(index, root)| format!("目录 {}: {}", index + 1, root_label(root)))
            .collect();
        let mut details: Vec<Vec<String>> = roots
            .iter()
            .enumerate()
            .map(|(index, root)| {
                vec![
                    format!("完整路径: {}", root.display()),
                    if index == 0 {
                        "默认扫描顺序中的第一个目录。".to_string()
                    } else {
                        "会和其他项目根目录一起参与扫描。".to_string()
                    },
                ]
            })
            .collect();

        items.push("新增项目根目录".to_string());
        details.push(vec!["添加新的项目来源目录。".to_string()]);

        if !roots.is_empty() {
            items.push("继续".to_string());
            details.push(vec![
                format!("当前共配置 {} 个项目根目录。", roots.len()),
                "下一步设置 Worktree 根目录。".to_string(),
            ]);
        }

        Page::ConfigProjectsRoots {
            roots,
            initial_setup,
            menu: tui::MenuState::new("gwtm / 项目目录", subtitle, items)
                .with_details(details)
                .with_search("过滤项目根目录"),
        }
    }

    fn config_project_root_actions_page(
        &self,
        roots: Vec<PathBuf>,
        root_idx: usize,
        initial_setup: bool,
    ) -> Page {
        let root = &roots[root_idx];
        let mut items = vec!["编辑路径".to_string()];
        let mut details = vec![vec![
            format!("当前路径: {}", root.display()),
            "重新选择或修改这个项目根目录。".to_string(),
        ]];
        if roots.len() > 1 {
            items.push("删除目录".to_string());
            details.push(vec![
                format!("当前路径: {}", root.display()),
                "从扫描列表里移除这个项目根目录。".to_string(),
            ]);
        }

        Page::ConfigProjectRootActions {
            roots,
            root_idx,
            initial_setup,
            menu: tui::MenuState::new(
                "gwtm / 项目目录操作",
                "选择要对这个项目根目录执行的操作",
                items,
            )
            .with_details(details),
        }
    }

    fn config_project_root_input_page(
        &self,
        roots: Vec<PathBuf>,
        edit_idx: Option<usize>,
        initial_setup: bool,
    ) -> Page {
        let (subtitle, value) = match edit_idx {
            Some(index) => (
                "更新一个已有的项目根目录",
                roots[index].to_string_lossy().to_string(),
            ),
            None => (
                "添加一个新的项目根目录，其一级子目录应为 Git 仓库",
                String::new(),
            ),
        };
        Page::ConfigProjectRootInput {
            roots,
            edit_idx,
            initial_setup,
            input: tui::InputState::new(
                "gwtm / 编辑项目目录",
                subtitle,
                "输入项目根目录路径",
                &value,
            )
            .with_file_picker(),
        }
    }

    fn config_worktrees_root_page(&self, project_roots: Vec<PathBuf>, initial_setup: bool) -> Page {
        let default_worktrees_root = self.default_worktrees_root_input(&project_roots);
        let subtitle = if initial_setup {
            "设置 Worktree 根目录，不存在会自动创建"
        } else {
            "更新 Worktree 根目录，不存在会自动创建"
        };
        Page::ConfigWorktreesRoot {
            project_roots,
            initial_setup,
            input: tui::InputState::new(
                "gwtm / 配置 Worktree 目录",
                subtitle,
                "输入 Worktree 根目录路径",
                &default_worktrees_root.to_string_lossy(),
            )
            .with_file_picker(),
        }
    }

    fn config_ide_page(
        &self,
        project_roots: Vec<PathBuf>,
        worktrees_root: PathBuf,
        initial_setup: bool,
    ) -> Result<Page> {
        let mut options = detect_ide_options()?;
        options.push(config_prompt_ide_option());
        let items: Vec<String> = options.iter().map(|option| option.label.clone()).collect();
        let details: Vec<Vec<String>> = options
            .iter()
            .map(|option| vec![option.detail.clone()])
            .collect();
        let default_index = preferred_ide_index(&options, &self.config);
        let mut menu = tui::MenuState::new(
            "gwtm / 选择 IDE",
            "选择创建或打开 worktree 时默认使用的 IDE",
            items,
        )
        .with_details(details);
        menu.list_state.select(Some(default_index));

        Ok(Page::ConfigIdeSelect {
            project_roots,
            worktrees_root,
            initial_setup,
            options,
            menu,
        })
    }

    fn default_worktrees_root_input(&self, project_roots: &[PathBuf]) -> PathBuf {
        let current_default = derive_default_worktrees_root(self.config.primary_projects_root());
        let next_primary = project_roots
            .first()
            .map(PathBuf::as_path)
            .unwrap_or_else(|| self.config.primary_projects_root());
        if self.config.worktrees_root_dir == current_default {
            derive_default_worktrees_root(next_primary)
        } else {
            self.config.worktrees_root_dir.clone()
        }
    }

    fn config_with_paths(&self, project_roots: Vec<PathBuf>, worktrees_root: PathBuf) -> AppConfig {
        AppConfig {
            projects_root_dirs: project_roots,
            worktrees_root_dir: worktrees_root,
            ide_mode: self.config.ide_mode.clone(),
            ide_command: self.config.ide_command.clone(),
            ide_label: self.config.ide_label.clone(),
        }
    }

    fn apply_config(
        &mut self,
        project_roots: Vec<PathBuf>,
        worktrees_root: PathBuf,
        ide: IdeOption,
    ) -> Result<Vec<String>> {
        fs::create_dir_all(&worktrees_root)
            .with_context(|| format!("创建 worktree 根目录失败: {}", worktrees_root.display()))?;

        let mut new_config = self.config_with_paths(project_roots.clone(), worktrees_root.clone());
        new_config.ide_mode = ide.mode;
        new_config.ide_command = ide.command;
        new_config.ide_label = ide.label;
        normalize_config(&mut new_config)?;
        save_config(&self.config_path, &new_config)?;
        self.config = new_config.clone();
        self.start_with_setup = false;

        Ok(vec![
            "[SUCCESS] 配置已保存".to_string(),
            format!("项目根目录数量: {}", project_roots.len()),
            format!(
                "项目根目录: {}",
                project_roots
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(" | ")
            ),
            format!("Worktree 根目录: {}", worktrees_root.display()),
            format!(
                "IDE: {}",
                if new_config.ide_mode == PROMPT_IDE_MODE {
                    "未设置（首次打开时再选择）".to_string()
                } else {
                    new_config.ide_label.clone()
                }
            ),
            format!("配置文件: {}", self.config_path.display()),
        ])
    }

    fn save_ide_preference(&mut self, ide: IdeOption) -> Result<()> {
        self.config.ide_mode = ide.mode;
        self.config.ide_command = ide.command;
        self.config.ide_label = ide.label;
        normalize_config(&mut self.config)?;
        save_config(&self.config_path, &self.config)
    }

    fn ide_is_configured(&self) -> bool {
        self.config.ide_mode != PROMPT_IDE_MODE
    }

    fn ide_selection_page_for_open(
        &self,
        title: &str,
        subtitle: &str,
        result_title: &str,
        result_subtitle: &str,
        worktree_path: PathBuf,
        lines: Vec<String>,
    ) -> Result<Page> {
        let mut options = detect_ide_options()?;
        options.push(IdeOption {
            mode: PROMPT_IDE_MODE.to_string(),
            command: String::new(),
            label: "本次先不打开".to_string(),
            detail: "保持当前结果，不打开 IDE。".to_string(),
        });

        let items: Vec<String> = options.iter().map(|option| option.label.clone()).collect();
        let details: Vec<Vec<String>> = options
            .iter()
            .map(|option| vec![option.detail.clone()])
            .collect();

        Ok(Page::ChooseIdeForOpen {
            result_title: result_title.to_string(),
            result_subtitle: result_subtitle.to_string(),
            worktree_path,
            lines,
            options,
            menu: tui::MenuState::new(title, subtitle, items).with_details(details),
        })
    }

    fn project_select_page(&mut self, intent: ProjectIntent) -> Result<Page> {
        self.projects = scan_projects(&self.config.projects_root_dirs)?;
        let items: Vec<String> = self
            .projects
            .iter()
            .map(|project| project.display_name.clone())
            .collect();
        let details: Vec<Vec<String>> = self
            .projects
            .iter()
            .map(|project| {
                vec![
                    format!("仓库路径: {}", project.path.display()),
                    format!("来源目录: {}", project.source_root.display()),
                ]
            })
            .collect();
        let subtitle = match intent {
            ProjectIntent::Create => "选择一个仓库来创建 worktree",
            ProjectIntent::Open => "选择一个仓库来打开已有 worktree",
            ProjectIntent::List => "选择一个仓库查看 worktree 列表",
            ProjectIntent::Remove => "选择一个仓库删除 worktree",
        };
        Ok(Page::ProjectSelect {
            intent,
            menu: tui::MenuState::new("gwtm / 项目选择", subtitle, items)
                .with_details(details)
                .with_search("输入项目名或路径关键词"),
        })
    }

    fn base_branch_page(&self, project_idx: usize, new_branch: String) -> Result<Page> {
        let project = &self.projects[project_idx];
        let base_branches = remote_branches(&project.path)?;
        let details: Vec<Vec<String>> = base_branches
            .iter()
            .map(|branch| vec![format!("将从 origin/{branch} 创建新分支 {new_branch}")])
            .collect();
        let default_index = default_base_branch_index(&base_branches);
        let mut menu = tui::MenuState::new(
            "gwtm / 基准分支",
            "选择新 worktree 的基准远程分支",
            base_branches.clone(),
        )
        .with_details(details)
        .with_search("输入分支关键词");
        menu.list_state.select(Some(default_index));
        Ok(Page::BaseBranchSelect {
            project_idx,
            new_branch,
            base_branches,
            menu,
        })
    }

    fn open_worktree_page(&self, project_idx: usize) -> Result<Page> {
        let project = &self.projects[project_idx];
        let worktrees: Vec<WorktreeEntry> = read_worktrees(&project.path)?
            .into_iter()
            .filter(|entry| !entry.bare)
            .collect();

        if worktrees.is_empty() {
            return Ok(Page::Result(tui::ResultState::new(
                "gwtm / 打开结果",
                project.name.as_str(),
                vec![format!(
                    "[INFO] 项目 {} 没有可打开的 worktree",
                    project.name
                )],
            )));
        }

        let items: Vec<String> = worktrees
            .iter()
            .map(|entry| {
                let branch = entry
                    .branch
                    .clone()
                    .unwrap_or_else(|| "(detached)".to_string());
                if entry.path == project.path {
                    format!("{branch} (主仓库)")
                } else {
                    branch
                }
            })
            .collect();
        let details: Vec<Vec<String>> = worktrees
            .iter()
            .map(|entry| {
                let branch = entry
                    .branch
                    .clone()
                    .unwrap_or_else(|| "(detached)".to_string());
                let location = if entry.path == project.path {
                    "主仓库"
                } else {
                    "Worktree"
                };
                vec![
                    format!("类型: {location}"),
                    format!("分支: {branch}"),
                    format!("路径: {}", entry.path.display()),
                    format!("提交: {}", entry.head),
                ]
            })
            .collect();

        Ok(Page::OpenWorktreeSelect {
            project_idx,
            worktrees,
            menu: tui::MenuState::new(
                "gwtm / 打开 Worktree",
                "选择一个已有 worktree 或主仓库",
                items,
            )
            .with_details(details)
            .with_search("输入分支名或路径关键词"),
        })
    }

    fn worktree_list_page(&self, project_idx: usize) -> Result<Page> {
        let project = &self.projects[project_idx];
        let worktrees = read_worktrees(&project.path)?;
        let mut lines = vec![format!("[INFO] 项目: {}", project.name)];
        for (index, worktree) in worktrees.iter().enumerate() {
            let branch = worktree
                .branch
                .clone()
                .unwrap_or_else(|| "(detached)".to_string());
            let marker = if worktree.path == project.path {
                " (主仓库)"
            } else {
                ""
            };
            lines.push(format!("{}. {}{}", index + 1, branch, marker));
            lines.push(format!("   路径: {}", worktree.path.display()));
            lines.push(format!("   提交: {}", worktree.head));
        }
        lines.push(format!("[INFO] 共 {} 个 worktree", worktrees.len()));
        Ok(Page::Result(tui::ResultState::new(
            "gwtm / Worktree 列表",
            project.name.as_str(),
            lines,
        )))
    }

    fn remove_worktree_page(&self, project_idx: usize) -> Result<Page> {
        let project = &self.projects[project_idx];
        let removable: Vec<WorktreeEntry> = read_worktrees(&project.path)?
            .into_iter()
            .filter(|entry| entry.path != project.path && !entry.bare)
            .collect();

        if removable.is_empty() {
            return Ok(Page::Result(tui::ResultState::new(
                "gwtm / 删除结果",
                project.name.as_str(),
                vec![format!(
                    "[INFO] 项目 {} 没有可删除的 worktree",
                    project.name
                )],
            )));
        }

        let items: Vec<String> = removable
            .iter()
            .map(|entry| {
                format!(
                    "{}",
                    entry
                        .branch
                        .clone()
                        .unwrap_or_else(|| "(detached)".to_string())
                )
            })
            .collect();
        let details: Vec<Vec<String>> = removable
            .iter()
            .map(|entry| {
                vec![
                    format!(
                        "分支: {}",
                        entry
                            .branch
                            .clone()
                            .unwrap_or_else(|| "(detached)".to_string())
                    ),
                    format!("路径: {}", entry.path.display()),
                    format!("提交: {}", entry.head),
                ]
            })
            .collect();

        Ok(Page::RemoveWorktreeSelect {
            project_idx,
            removable,
            menu: tui::MenuState::new("gwtm / 删除 Worktree", "选择一个要删除的 worktree", items)
                .with_details(details)
                .with_search("输入分支名或路径关键词"),
        })
    }

    fn remove_local_branch_confirm_page(
        &self,
        project_idx: usize,
        selected: WorktreeEntry,
    ) -> Page {
        let branch_name = selected.branch.clone().unwrap_or_default();
        Page::RemoveLocalBranchConfirm {
            project_idx,
            selected,
            menu: tui::MenuState::new(
                "gwtm / 删除分支",
                "是否同时删除本地分支？",
                vec!["是".to_string(), "否".to_string()],
            )
            .with_details(vec![
                vec![
                    format!("分支: {branch_name}"),
                    "将删除 worktree 后继续删除本地分支。".to_string(),
                ],
                vec![
                    format!("分支: {branch_name}"),
                    "只删除 worktree，保留本地分支。".to_string(),
                ],
            ]),
        }
    }

    fn next_remove_step(
        &self,
        project_idx: usize,
        selected: WorktreeEntry,
        delete_local_branch: bool,
    ) -> Result<Page> {
        let Some(branch_name) = selected.branch.clone() else {
            let lines =
                self.remove_worktree_with_lines(project_idx, selected, delete_local_branch, false)?;
            return Ok(Page::Result(tui::ResultState::new(
                "gwtm / 删除结果",
                "删除流程已完成",
                lines,
            )));
        };

        let project = &self.projects[project_idx];
        if remote_branch_exists(&project.path, &branch_name)? {
            Ok(Page::RemoveRemoteBranchConfirm {
                project_idx,
                selected,
                delete_local_branch,
                menu: tui::MenuState::new(
                    "gwtm / 删除远程分支",
                    "是否同时删除远程分支？",
                    vec!["是".to_string(), "否".to_string()],
                )
                .with_details(vec![
                    vec![
                        format!("远程分支: origin/{branch_name}"),
                        "删除 worktree 后同时删除远程分支。".to_string(),
                    ],
                    vec![
                        format!("远程分支: origin/{branch_name}"),
                        "删除 worktree 后保留远程分支。".to_string(),
                    ],
                ]),
            })
        } else {
            let lines =
                self.remove_worktree_with_lines(project_idx, selected, delete_local_branch, false)?;
            Ok(Page::Result(tui::ResultState::new(
                "gwtm / 删除结果",
                "删除流程已完成",
                lines,
            )))
        }
    }

    fn confirm_open_ide_page(&self, worktree_path: PathBuf, lines: Vec<String>) -> Page {
        Page::ConfirmOpenIde {
            worktree_path,
            lines,
            menu: tui::MenuState::new(
                "gwtm / 打开 IDE",
                if self.ide_is_configured() {
                    format!("是否使用 {} 打开刚创建的 worktree？", self.config.ide_label)
                } else {
                    "当前未设置默认 IDE，是否现在选择一个来打开？".to_string()
                }
                .as_str(),
                vec!["是".to_string(), "否".to_string()],
            )
            .with_details(vec![
                vec![if self.ide_is_configured() {
                    "创建完成后立即调用当前配置的 IDE 打开该目录。".to_string()
                } else {
                    "创建完成后会扫描本机可用 IDE，并让你临时选择一个打开。".to_string()
                }],
                vec!["只展示结果，不额外打开 IDE。".to_string()],
            ]),
        }
    }

    fn error_result_page(&self, title: &str, err: anyhow::Error) -> Page {
        Page::Result(tui::ResultState::new(
            "gwtm / 错误",
            title,
            vec![format!("[ERROR] {err:#}")],
        ))
    }

    fn create_worktree_with_lines(
        &self,
        project_idx: usize,
        new_branch: String,
        base_branch: String,
    ) -> Result<(Vec<String>, PathBuf)> {
        let project = &self.projects[project_idx];
        let dir_name = branch_to_dirname(&new_branch);
        let worktree_path = self
            .config
            .worktrees_root_dir
            .join(&project.name)
            .join(&dir_name);

        if worktree_path.exists() {
            bail!("Worktree 目录已存在: {}", worktree_path.display());
        }

        let mut lines = vec![
            format!("[INFO] 项目: {}", project.name),
            format!("[INFO] 新分支: {new_branch}"),
            format!("[INFO] 基准分支: origin/{base_branch}"),
            format!("[INFO] Worktree 路径: {}", worktree_path.display()),
            "[INFO] 正在 fetch 远程仓库...".to_string(),
        ];

        run_git(&project.path, &["fetch", "origin"])?;

        fs::create_dir_all(
            worktree_path
                .parent()
                .ok_or_else(|| anyhow!("Worktree 路径无效: {}", worktree_path.display()))?,
        )
        .with_context(|| format!("创建 Worktree 父目录失败: {}", worktree_path.display()))?;

        lines.push("[INFO] 正在创建 worktree...".to_string());
        run_git(
            &project.path,
            &[
                "worktree",
                "add",
                "-b",
                &new_branch,
                &worktree_path.to_string_lossy(),
                &format!("origin/{base_branch}"),
            ],
        )?;

        lines.push("[INFO] 正在推送新分支到远程...".to_string());
        if let Err(err) = run_git(&worktree_path, &["push", "-u", "origin", &new_branch]) {
            lines.push(format!(
                "[WARNING] 推送远程分支失败，Worktree 已创建但未成功建立远程分支: {err}"
            ));
        } else {
            lines.push(format!(
                "[SUCCESS] 远程分支已创建并建立跟踪: origin/{new_branch}"
            ));
        }

        lines.push("[SUCCESS] Worktree 创建成功".to_string());
        lines.push(format!("路径: {}", worktree_path.display()));
        lines.push(format!("分支: {new_branch}"));
        lines.push(format!("远程: origin/{new_branch}"));

        Ok((lines, worktree_path))
    }

    fn open_worktree_action(
        &self,
        project_idx: usize,
        selected: WorktreeEntry,
    ) -> Result<LoopAction> {
        if self.ide_is_configured() {
            let lines = self.open_worktree_with_lines(project_idx, selected)?;
            return Ok(LoopAction::Push(Page::Result(tui::ResultState::new(
                "gwtm / 打开结果",
                "Worktree 已打开",
                lines,
            ))));
        }

        let project = &self.projects[project_idx];
        let branch = selected
            .branch
            .clone()
            .unwrap_or_else(|| "(detached)".to_string());
        let kind = if selected.path == project.path {
            "主仓库"
        } else {
            "Worktree"
        };

        let lines = vec![
            format!("[INFO] 项目: {}", project.name),
            format!("[INFO] 类型: {kind}"),
            format!("[INFO] 分支: {branch}"),
            format!("[INFO] 路径: {}", selected.path.display()),
        ];

        Ok(LoopAction::Push(self.ide_selection_page_for_open(
            "gwtm / 选择 IDE",
            "当前未设置默认 IDE，选择一个打开已有 worktree",
            "gwtm / 打开结果",
            "Worktree 已打开",
            selected.path.clone(),
            lines,
        )?))
    }

    fn open_worktree_with_lines(
        &self,
        project_idx: usize,
        selected: WorktreeEntry,
    ) -> Result<Vec<String>> {
        let project = &self.projects[project_idx];
        let branch = selected
            .branch
            .clone()
            .unwrap_or_else(|| "(detached)".to_string());
        let kind = if selected.path == project.path {
            "主仓库"
        } else {
            "Worktree"
        };

        open_with_ide(&self.config, &selected.path)?;

        Ok(vec![
            format!("[INFO] 项目: {}", project.name),
            format!("[INFO] 类型: {kind}"),
            format!("[INFO] 分支: {branch}"),
            format!("[INFO] 路径: {}", selected.path.display()),
            format!(
                "[SUCCESS] 已触发 {} 打开项目: {}",
                self.config.ide_label,
                selected.path.display()
            ),
        ])
    }

    fn remove_worktree_with_lines(
        &self,
        project_idx: usize,
        selected: WorktreeEntry,
        delete_local_branch: bool,
        delete_remote_branch: bool,
    ) -> Result<Vec<String>> {
        let project = &self.projects[project_idx];
        let branch_name = selected.branch.clone();

        let mut lines = vec![
            "[INFO] 即将删除 Worktree".to_string(),
            format!("路径: {}", selected.path.display()),
        ];
        if let Some(ref branch) = branch_name {
            lines.push(format!("分支: {branch}"));
        }

        if let Err(err) = run_git(
            &project.path,
            &["worktree", "remove", &selected.path.to_string_lossy()],
        ) {
            lines.push(format!("[WARNING] 普通删除失败，尝试强制删除: {err}"));
            run_git(
                &project.path,
                &[
                    "worktree",
                    "remove",
                    "--force",
                    &selected.path.to_string_lossy(),
                ],
            )?;
        }
        lines.push(format!(
            "[SUCCESS] Worktree 已删除: {}",
            selected.path.display()
        ));

        if delete_local_branch {
            let branch = branch_name
                .as_ref()
                .ok_or_else(|| anyhow!("无法删除本地分支，当前 worktree 没有关联分支"))?;
            if let Err(err) = run_git(&project.path, &["branch", "-d", branch]) {
                lines.push(format!("[WARNING] git branch -d 失败，尝试强制删除: {err}"));
                run_git(&project.path, &["branch", "-D", branch])?;
            }
            lines.push(format!("[SUCCESS] 本地分支已删除: {branch}"));
        }

        if delete_remote_branch {
            let branch = branch_name
                .as_ref()
                .ok_or_else(|| anyhow!("无法删除远程分支，当前 worktree 没有关联分支"))?;
            run_git(&project.path, &["push", "origin", "--delete", branch])?;
            lines.push(format!("[SUCCESS] 远程分支已删除: origin/{branch}"));
        }

        Ok(lines)
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("[ERROR] {err:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    match parse_args()? {
        CliAction::ShowHelp => {
            print_help()?;
            return Ok(());
        }
        CliAction::ShowVersion => {
            println!("{APP_NAME} {VERSION}");
            return Ok(());
        }
        CliAction::RunInteractive => {}
    }

    ensure_git_available()?;
    let config_path = config_file_path()?;
    let (config, needs_setup) = load_or_prepare_config(&config_path)?;

    if needs_setup {
        FullScreenApp::new_for_setup(config, config_path).run()
    } else {
        FullScreenApp::new(config, config_path).run()
    }
}

enum CliAction {
    ShowHelp,
    ShowVersion,
    RunInteractive,
}

fn parse_args() -> Result<CliAction> {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        None => Ok(CliAction::RunInteractive),
        Some("-h") | Some("--help") => Ok(CliAction::ShowHelp),
        Some("-V") | Some("--version") => Ok(CliAction::ShowVersion),
        Some(arg) => bail!("不支持的参数: {arg}\n使用 `gwtm --help` 查看可用参数。"),
    }
}

fn print_help() -> Result<()> {
    let config_path = config_file_path()?;
    println!("{APP_NAME} {VERSION}");
    println!("Git Worktree Manager for Rust-oriented workflows");
    println!();
    println!("USAGE:");
    println!("  gwtm");
    println!("  gwtm --help");
    println!("  gwtm --version");
    println!();
    println!("OPTIONS:");
    println!("  -h, --help       Print help information");
    println!("  -V, --version    Print version information");
    println!();
    println!("CONFIG:");
    println!("  {}", config_path.display());
    println!();
    println!("HOMEPAGE:");
    println!("  {HOMEPAGE}");
    Ok(())
}

fn ensure_git_available() -> Result<()> {
    Command::new("git")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("无法执行 git，请先安装 Git")?
        .success()
        .then_some(())
        .ok_or_else(|| anyhow!("命令 `git` 不可用"))
}

fn config_file_path() -> Result<PathBuf> {
    if let Some(xdg_config_home) = env::var_os("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(xdg_config_home)
            .join(APP_NAME)
            .join("config.toml"));
    }

    let home = dirs::home_dir().ok_or_else(|| anyhow!("无法定位 home 目录"))?;
    Ok(home.join(".config").join(APP_NAME).join("config.toml"))
}

fn load_or_prepare_config(config_path: &Path) -> Result<(AppConfig, bool)> {
    if config_path.exists() {
        let mut config = load_config(config_path)?;
        normalize_config(&mut config)?;
        return Ok((config, false));
    }

    Ok((initial_config_guess()?, true))
}

fn load_config(config_path: &Path) -> Result<AppConfig> {
    let content = fs::read_to_string(config_path)
        .with_context(|| format!("读取配置文件失败: {}", config_path.display()))?;
    let value: toml::Value = toml::from_str(&content).context("解析配置文件失败")?;
    parse_app_config(value)
}

fn save_config(config_path: &Path, config: &AppConfig) -> Result<()> {
    let parent = config_path
        .parent()
        .ok_or_else(|| anyhow!("配置文件路径无效"))?;
    fs::create_dir_all(parent)
        .with_context(|| format!("创建配置目录失败: {}", parent.display()))?;
    let content = toml::to_string_pretty(config).context("序列化配置失败")?;
    fs::write(config_path, content)
        .with_context(|| format!("写入配置文件失败: {}", config_path.display()))
}

fn normalize_config(config: &mut AppConfig) -> Result<()> {
    if config.projects_root_dirs.is_empty() {
        bail!("至少需要配置一个项目根目录");
    }
    config.projects_root_dirs = dedupe_project_roots(
        config
            .projects_root_dirs
            .iter()
            .map(|path| normalize_path(path))
            .collect::<Result<Vec<_>>>()?,
    );
    config.worktrees_root_dir = normalize_path(&config.worktrees_root_dir)?;
    if config.ide_mode.is_empty() {
        config.ide_mode = PROMPT_IDE_MODE.to_string();
    }

    match config.ide_mode.as_str() {
        PROMPT_IDE_MODE => {
            config.ide_command.clear();
            if config.ide_label.is_empty() {
                config.ide_label = PROMPT_IDE_LABEL.to_string();
            }
        }
        "system" => {
            if config.ide_command.is_empty() {
                config.ide_command = DEFAULT_IDE_COMMAND.to_string();
            }
            if config.ide_label.is_empty() {
                config.ide_label = DEFAULT_IDE_LABEL.to_string();
            }
        }
        _ => {
            if config.ide_command.is_empty() {
                config.ide_command = DEFAULT_IDE_COMMAND.to_string();
                config.ide_mode = DEFAULT_IDE_MODE.to_string();
            }
            if config.ide_label.is_empty() {
                config.ide_label = config.ide_command.clone();
            }
        }
    }
    Ok(())
}

fn parse_app_config(value: toml::Value) -> Result<AppConfig> {
    #[derive(Deserialize)]
    struct ConfigCompat {
        #[serde(default)]
        projects_root_dirs: Vec<PathBuf>,
        #[serde(default)]
        projects_root_dir: Option<PathBuf>,
        worktrees_root_dir: PathBuf,
        #[serde(default)]
        ide_mode: String,
        #[serde(default)]
        ide_command: String,
        #[serde(default)]
        ide_label: String,
    }

    let compat: ConfigCompat = value.try_into().context("解析配置文件失败")?;
    let mut projects_root_dirs = compat.projects_root_dirs;
    if projects_root_dirs.is_empty() {
        if let Some(legacy_root) = compat.projects_root_dir {
            projects_root_dirs.push(legacy_root);
        }
    }

    Ok(AppConfig {
        projects_root_dirs,
        worktrees_root_dir: compat.worktrees_root_dir,
        ide_mode: compat.ide_mode,
        ide_command: compat.ide_command,
        ide_label: compat.ide_label,
    })
}

fn normalize_path(input: &Path) -> Result<PathBuf> {
    let raw = input.to_string_lossy();
    let expanded = if raw == "~" {
        dirs::home_dir().ok_or_else(|| anyhow!("无法解析 home 目录"))?
    } else if let Some(rest) = raw.strip_prefix("~/") {
        dirs::home_dir()
            .ok_or_else(|| anyhow!("无法解析 home 目录"))?
            .join(rest)
    } else {
        input.to_path_buf()
    };

    if expanded.exists() {
        expanded
            .canonicalize()
            .with_context(|| format!("规范化路径失败: {}", expanded.display()))
    } else {
        Ok(expanded)
    }
}

fn dedupe_project_roots(roots: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = std::collections::HashSet::new();
    let mut deduped = Vec::new();
    for root in roots {
        if seen.insert(root.clone()) {
            deduped.push(root);
        }
    }
    deduped
}

fn root_label(path: &Path) -> String {
    path.file_name()
        .map(|value| value.to_string_lossy().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| path.display().to_string())
}

fn derive_default_worktrees_root(projects_root: &Path) -> PathBuf {
    projects_root
        .parent()
        .unwrap_or_else(|| Path::new("/"))
        .join("worktrees")
}

fn choose_folder_with_dialog(prompt: &str) -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        let output = Command::new("osascript")
            .arg("-e")
            .arg("try")
            .arg("-e")
            .arg(format!(
                "POSIX path of (choose folder with prompt \"{}\")",
                prompt.replace('"', "\\\"")
            ))
            .arg("-e")
            .arg("on error number -128")
            .arg("-e")
            .arg("return \"\"")
            .arg("-e")
            .arg("end try")
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if value.is_empty() {
            None
        } else {
            normalize_path(Path::new(&value)).ok()
        }
    } else {
        None
    }
}

fn initial_config_guess() -> Result<AppConfig> {
    let projects_root = normalize_path(&default_projects_root_guess())?;
    let worktrees_root = derive_default_worktrees_root(&projects_root);
    Ok(AppConfig::default_with_paths(projects_root, worktrees_root))
}

fn default_projects_root_guess() -> PathBuf {
    env::current_dir()
        .ok()
        .filter(|path| path.is_dir())
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn detect_ide_options() -> Result<Vec<IdeOption>> {
    let mut options = Vec::new();

    for (label, command) in [
        ("Visual Studio Code", "code"),
        ("Cursor", "cursor"),
        ("Windsurf", "windsurf"),
        ("Zed", "zed"),
        ("IntelliJ IDEA", "idea"),
        ("GoLand", "goland"),
        ("WebStorm", "webstorm"),
        ("PyCharm", "pycharm"),
        ("CLion", "clion"),
        ("Rider", "rider"),
        ("RustRover", "rustrover"),
        ("Android Studio", "studio"),
    ] {
        if let Some(path) = command_in_path(command) {
            options.push(IdeOption {
                mode: "command".to_string(),
                command: command.to_string(),
                label: label.to_string(),
                detail: format!("CLI 启动命令: {} ({})", command, path.display()),
            });
        }
    }

    for (label, app_name) in [
        ("Visual Studio Code", "Visual Studio Code.app"),
        ("Cursor", "Cursor.app"),
        ("Windsurf", "Windsurf.app"),
        ("Zed", "Zed.app"),
        ("IntelliJ IDEA", "IntelliJ IDEA.app"),
        ("IntelliJ IDEA Ultimate", "IntelliJ IDEA Ultimate.app"),
        (
            "IntelliJ IDEA Community Edition",
            "IntelliJ IDEA Community Edition.app",
        ),
        ("GoLand", "GoLand.app"),
        ("WebStorm", "WebStorm.app"),
        ("PyCharm", "PyCharm.app"),
        ("CLion", "CLion.app"),
        ("Rider", "Rider.app"),
        ("RustRover", "RustRover.app"),
        ("Android Studio", "Android Studio.app"),
    ] {
        if let Some(path) = find_macos_app(app_name) {
            if options
                .iter()
                .any(|option| option.label == label && option.mode == "app")
            {
                continue;
            }
            options.push(IdeOption {
                mode: "app".to_string(),
                command: label.to_string(),
                label: label.to_string(),
                detail: format!("macOS 应用: {}", path.display()),
            });
        }
    }

    options.push(IdeOption {
        mode: "system".to_string(),
        command: "open".to_string(),
        label: DEFAULT_IDE_LABEL.to_string(),
        detail: "使用 macOS 默认关联应用打开目录。".to_string(),
    });

    Ok(options)
}

fn command_in_path(command: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    for dir in env::split_paths(&path) {
        let candidate = dir.join(command);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn find_macos_app(app_name: &str) -> Option<PathBuf> {
    if !cfg!(target_os = "macos") {
        return None;
    }

    let mut bases = vec![PathBuf::from("/Applications")];
    if let Some(home) = dirs::home_dir() {
        bases.push(home.join("Applications"));
    }

    for base in bases {
        let candidate = base.join(app_name);
        if candidate.is_dir() {
            return Some(candidate);
        }
    }

    None
}

fn preferred_ide_index(options: &[IdeOption], config: &AppConfig) -> usize {
    if let Some(index) = options.iter().position(|option| {
        option.mode == config.ide_mode
            && option.command == config.ide_command
            && option.label == config.ide_label
    }) {
        return index;
    }

    options
        .iter()
        .position(|option| option.mode == PROMPT_IDE_MODE)
        .or_else(|| options.iter().position(|option| option.mode != "system"))
        .unwrap_or(0)
}

fn config_prompt_ide_option() -> IdeOption {
    IdeOption {
        mode: PROMPT_IDE_MODE.to_string(),
        command: String::new(),
        label: PROMPT_IDE_LABEL.to_string(),
        detail: "暂不设置默认 IDE，等首次创建或打开 worktree 时再选择。".to_string(),
    }
}

fn open_with_ide_option(ide: &IdeOption, project_path: &Path) -> Result<()> {
    let mut command = match ide.mode.as_str() {
        "app" => {
            let mut cmd = Command::new("open");
            cmd.arg("-a").arg(&ide.command).arg(project_path);
            cmd
        }
        "system" => {
            let mut cmd = Command::new("open");
            cmd.arg(project_path);
            cmd
        }
        _ => {
            let mut cmd = Command::new(&ide.command);
            cmd.arg(project_path);
            cmd
        }
    };

    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("执行 IDE 命令失败: {}。", ide.label))?;

    let _ = child.try_wait();
    Ok(())
}

fn open_with_ide(config: &AppConfig, project_path: &Path) -> Result<()> {
    open_with_ide_option(
        &IdeOption {
            mode: config.ide_mode.clone(),
            command: config.ide_command.clone(),
            label: config.ide_label.clone(),
            detail: String::new(),
        },
        project_path,
    )
    .with_context(|| "可进入“重新配置”重新选择可用 IDE。")
}

fn validate_directory_input(value: &str, must_exist: bool) -> Result<PathBuf> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        bail!("输入不能为空");
    }

    let normalized = normalize_path(Path::new(trimmed))?;
    if must_exist && !normalized.is_dir() {
        bail!("目录不存在: {}", normalized.display());
    }
    if normalized.exists() && !normalized.is_dir() {
        bail!("路径不是目录: {}", normalized.display());
    }

    Ok(normalized)
}

fn scan_projects(projects_root_dirs: &[PathBuf]) -> Result<Vec<Project>> {
    if projects_root_dirs.is_empty() {
        bail!("至少需要配置一个项目根目录。请进入“重新配置”添加。");
    }

    let mut seen = std::collections::HashSet::new();
    let mut projects = Vec::new();
    for projects_root_dir in projects_root_dirs {
        if !projects_root_dir.is_dir() {
            bail!("项目根目录不存在: {}", projects_root_dir.display());
        }

        for entry in fs::read_dir(projects_root_dir)
            .with_context(|| format!("读取项目根目录失败: {}", projects_root_dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if !path.join(".git").exists() {
                continue;
            }
            let canonical = path
                .canonicalize()
                .with_context(|| format!("规范化项目路径失败: {}", path.display()))?;
            if !seen.insert(canonical.clone()) {
                continue;
            }
            let name = canonical
                .file_name()
                .map(|v| v.to_string_lossy().to_string())
                .ok_or_else(|| anyhow!("项目目录名称无效: {}", canonical.display()))?;
            projects.push(Project {
                display_name: name.clone(),
                name,
                path: canonical,
                source_root: projects_root_dir.clone(),
            });
        }
    }

    projects.sort_by(|a, b| {
        a.name
            .cmp(&b.name)
            .then_with(|| a.path.cmp(&b.path))
    });

    let mut counts = std::collections::HashMap::new();
    for project in &projects {
        *counts.entry(project.name.clone()).or_insert(0usize) += 1;
    }
    for project in &mut projects {
        if counts.get(&project.name).copied().unwrap_or(0) > 1 {
            project.display_name = format!("{} ({})", project.name, root_label(&project.source_root));
        }
    }

    if projects.is_empty() {
        bail!(
            "在这些目录中未找到任何 Git 仓库：{}。请确认项目目录结构或重新配置。",
            projects_root_dirs
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(" | ")
        );
    }

    Ok(projects)
}

fn remote_branches(project_path: &Path) -> Result<Vec<String>> {
    let output = run_git_capture(
        project_path,
        &[
            "for-each-ref",
            "--format=%(refname:short)",
            "--sort=-committerdate",
            "refs/remotes/origin",
        ],
    )?;

    let mut branches: Vec<String> = output
        .lines()
        .map(|line| line.trim().trim_start_matches("origin/").to_string())
        .filter(|line| !line.is_empty() && line != "HEAD")
        .collect();

    if branches.is_empty() {
        branches.push("master".to_string());
    }

    Ok(branches)
}

fn default_base_branch_index(branches: &[String]) -> usize {
    branches
        .iter()
        .position(|branch| branch == "main")
        .or_else(|| branches.iter().position(|branch| branch == "master"))
        .unwrap_or(0)
}

fn branch_to_dirname(branch: &str) -> String {
    branch.replace('/', "-")
}

fn read_worktrees(project_path: &Path) -> Result<Vec<WorktreeEntry>> {
    let output = run_git_capture(project_path, &["worktree", "list", "--porcelain"])?;
    parse_worktree_porcelain(&output)
}

fn parse_worktree_porcelain(input: &str) -> Result<Vec<WorktreeEntry>> {
    let mut entries = Vec::new();
    let mut current: Option<WorktreeEntry> = None;

    for line in input.lines() {
        if line.trim().is_empty() {
            if let Some(entry) = current.take() {
                entries.push(entry);
            }
            continue;
        }

        if let Some(path) = line.strip_prefix("worktree ") {
            if let Some(entry) = current.take() {
                entries.push(entry);
            }
            current = Some(WorktreeEntry {
                path: PathBuf::from(path.trim()),
                head: String::new(),
                branch: None,
                bare: false,
            });
            continue;
        }

        let entry = current
            .as_mut()
            .ok_or_else(|| anyhow!("worktree 输出格式异常: {line}"))?;

        if let Some(head) = line.strip_prefix("HEAD ") {
            entry.head = head.trim().to_string();
        } else if let Some(branch) = line.strip_prefix("branch ") {
            entry.branch = Some(branch.trim().trim_start_matches("refs/heads/").to_string());
        } else if line.trim() == "bare" {
            entry.bare = true;
        }
    }

    if let Some(entry) = current.take() {
        entries.push(entry);
    }

    Ok(entries)
}

fn remote_branch_exists(project_path: &Path, branch: &str) -> Result<bool> {
    let status = Command::new("git")
        .arg("-C")
        .arg(project_path)
        .arg("show-ref")
        .arg("--verify")
        .arg("--quiet")
        .arg(format!("refs/remotes/origin/{branch}"))
        .status()
        .with_context(|| format!("检查远程分支失败: origin/{branch}"))?;
    Ok(status.success())
}

fn run_git(project_path: &Path, args: &[&str]) -> Result<()> {
    let output = Command::new("git")
        .arg("-C")
        .arg(project_path)
        .args(args)
        .output()
        .with_context(|| {
            format!(
                "执行 git 命令失败: git -C {} {}",
                project_path.display(),
                args.join(" ")
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if stderr.trim().is_empty() {
            stdout.trim()
        } else {
            stderr.trim()
        };
        bail!(
            "git 命令执行失败: git -C {} {}\n{}",
            project_path.display(),
            args.join(" "),
            detail
        );
    }

    Ok(())
}

fn run_git_capture(project_path: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(project_path)
        .args(args)
        .output()
        .with_context(|| {
            format!(
                "执行 git 命令失败: git -C {} {}",
                project_path.display(),
                args.join(" ")
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "git 命令执行失败: git -C {} {}\n{}",
            project_path.display(),
            args.join(" "),
            stderr.trim()
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn branch_name_is_mapped_to_directory_name() {
        assert_eq!(branch_to_dirname("feat/my-feature"), "feat-my-feature");
        assert_eq!(branch_to_dirname("main"), "main");
    }

    #[test]
    fn parse_worktree_porcelain_output() {
        let input = "\
worktree /tmp/repo
HEAD abc123
branch refs/heads/main

worktree /tmp/worktrees/repo/feat-a
HEAD def456
branch refs/heads/feat/a
";

        let entries = parse_worktree_porcelain(input).expect("parse should succeed");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path, PathBuf::from("/tmp/repo"));
        assert_eq!(entries[0].branch.as_deref(), Some("main"));
        assert_eq!(entries[1].path, PathBuf::from("/tmp/worktrees/repo/feat-a"));
        assert_eq!(entries[1].branch.as_deref(), Some("feat/a"));
        assert_eq!(entries[1].head, "def456");
    }

    #[test]
    fn load_legacy_single_root_config() {
        let value: toml::Value = toml::from_str(
            r#"
projects_root_dir = "/tmp/code"
worktrees_root_dir = "/tmp/worktrees"
ide_mode = "app"
ide_command = "IntelliJ IDEA"
ide_label = "IntelliJ IDEA"
"#,
        )
        .expect("toml should parse");

        let config = parse_app_config(value).expect("legacy config should parse");
        assert_eq!(config.projects_root_dirs, vec![PathBuf::from("/tmp/code")]);
        assert_eq!(config.worktrees_root_dir, PathBuf::from("/tmp/worktrees"));
    }

    #[test]
    fn scan_projects_across_multiple_roots() {
        let base = env::temp_dir().join(format!(
            "gwtm-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should be valid")
                .as_nanos()
        ));
        let root_a = base.join("code-a");
        let root_b = base.join("code-b");
        let alpha = root_a.join("alpha");
        let beta = root_b.join("beta");

        fs::create_dir_all(alpha.join(".git")).expect("alpha git dir should be created");
        fs::create_dir_all(beta.join(".git")).expect("beta git dir should be created");

        let projects =
            scan_projects(&[root_a.clone(), root_b.clone()]).expect("scan should succeed");

        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0].name, "alpha");
        assert_eq!(projects[0].source_root, root_a);
        assert_eq!(projects[1].name, "beta");
        assert_eq!(projects[1].source_root, root_b);

        fs::remove_dir_all(base).expect("temp test tree should be removed");
    }
}
