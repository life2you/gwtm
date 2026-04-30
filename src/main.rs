use anyhow::{Context, Result, anyhow, bail};
use dialoguer::{Confirm, Input, Select, theme::ColorfulTheme};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const APP_NAME: &str = "gwtm";
const DEFAULT_IDE_MODE: &str = "rust";
const DEFAULT_IDE_COMMAND: &str = "rustrover";
const DEFAULT_IDE_LABEL: &str = "RustRover";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const HOMEPAGE: &str = env!("CARGO_PKG_HOMEPAGE");

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppConfig {
    projects_root_dir: PathBuf,
    worktrees_root_dir: PathBuf,
    ide_mode: String,
    ide_command: String,
    ide_label: String,
}

impl AppConfig {
    fn default_with_paths(projects_root_dir: PathBuf, worktrees_root_dir: PathBuf) -> Self {
        Self {
            projects_root_dir,
            worktrees_root_dir,
            ide_mode: DEFAULT_IDE_MODE.to_string(),
            ide_command: DEFAULT_IDE_COMMAND.to_string(),
            ide_label: DEFAULT_IDE_LABEL.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
struct Project {
    name: String,
    path: PathBuf,
}

#[derive(Debug, Clone)]
struct WorktreeEntry {
    path: PathBuf,
    head: String,
    branch: Option<String>,
    bare: bool,
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
    let theme = ColorfulTheme::default();
    let config_path = config_file_path()?;
    let mut config = load_or_setup_config(&theme, &config_path)?;
    main_menu(&theme, &config_path, &mut config)
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

fn load_or_setup_config(theme: &ColorfulTheme, config_path: &Path) -> Result<AppConfig> {
    if config_path.exists() {
        let mut config = load_config(config_path)?;
        normalize_config(&mut config)?;
        return Ok(config);
    }

    let config = run_setup_wizard(theme, None, None)?;
    save_config(config_path, &config)?;
    Ok(config)
}

fn load_config(config_path: &Path) -> Result<AppConfig> {
    let content = fs::read_to_string(config_path)
        .with_context(|| format!("读取配置文件失败: {}", config_path.display()))?;
    toml::from_str(&content).context("解析配置文件失败")
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
    config.projects_root_dir = normalize_path(&config.projects_root_dir)?;
    config.worktrees_root_dir = normalize_path(&config.worktrees_root_dir)?;
    if config.ide_mode.is_empty() {
        config.ide_mode = DEFAULT_IDE_MODE.to_string();
    }
    if config.ide_command.is_empty() {
        config.ide_command = DEFAULT_IDE_COMMAND.to_string();
    }
    if config.ide_label.is_empty() {
        config.ide_label = DEFAULT_IDE_LABEL.to_string();
    }
    Ok(())
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

fn prompt_directory(
    theme: &ColorfulTheme,
    title: &str,
    default: Option<&Path>,
    must_exist: bool,
) -> Result<PathBuf> {
    loop {
        let mut input = Input::<String>::with_theme(theme).with_prompt(title.to_string());
        if let Some(default_value) = default {
            input = input.default(default_value.to_string_lossy().to_string());
        }
        let value = input.interact_text().context("读取目录输入失败")?;
        let normalized = normalize_path(Path::new(&value))?;

        if must_exist && !normalized.is_dir() {
            println!("[ERROR] 目录不存在: {}", normalized.display());
            pause();
            continue;
        }
        if normalized.exists() && !normalized.is_dir() {
            println!("[ERROR] 路径不是目录: {}", normalized.display());
            pause();
            continue;
        }
        return Ok(normalized);
    }
}

fn run_setup_wizard(
    theme: &ColorfulTheme,
    existing_projects_root: Option<&Path>,
    existing_worktrees_root: Option<&Path>,
) -> Result<AppConfig> {
    println!("== gwtm 初始化配置 ==");
    println!("首次启动会先配置项目根目录和 worktree 根目录。");

    let project_picker_default = existing_projects_root
        .map(Path::to_path_buf)
        .or_else(|| choose_folder_with_dialog("请选择包含多个 Git 仓库的项目根目录"));

    let projects_root =
        prompt_directory(theme, "项目根目录", project_picker_default.as_deref(), true)?;

    let worktrees_default = existing_worktrees_root
        .map(Path::to_path_buf)
        .unwrap_or_else(|| derive_default_worktrees_root(&projects_root));

    let worktrees_root = prompt_directory(
        theme,
        "Worktree 根目录（不存在会自动创建）",
        Some(&worktrees_default),
        false,
    )?;

    fs::create_dir_all(&worktrees_root)
        .with_context(|| format!("创建 worktree 根目录失败: {}", worktrees_root.display()))?;

    Ok(AppConfig::default_with_paths(projects_root, worktrees_root))
}

fn main_menu(theme: &ColorfulTheme, config_path: &Path, config: &mut AppConfig) -> Result<()> {
    loop {
        println!();
        println!("== gwtm ==");
        println!("模式: {} ({})", config.ide_label, config.ide_command);
        println!("项目根目录: {}", config.projects_root_dir.display());

        let items = vec![
            "创建 Worktree",
            "列出 Worktree",
            "删除 Worktree",
            "重新配置",
            "退出程序",
        ];

        let selection = Select::with_theme(theme)
            .with_prompt("请选择操作")
            .items(&items)
            .default(0)
            .interact()
            .context("读取主菜单选择失败")?;

        match selection {
            0 => {
                let projects = scan_projects(&config.projects_root_dir)?;
                do_create_worktree(theme, config, &projects)?;
            }
            1 => {
                let projects = scan_projects(&config.projects_root_dir)?;
                do_list_worktrees(theme, &projects)?;
            }
            2 => {
                let projects = scan_projects(&config.projects_root_dir)?;
                do_remove_worktree(theme, &projects)?;
            }
            3 => {
                let new_config = run_setup_wizard(
                    theme,
                    Some(&config.projects_root_dir),
                    Some(&config.worktrees_root_dir),
                )?;
                save_config(config_path, &new_config)?;
                *config = new_config;
                println!("[INFO] 配置已更新: {}", config_path.display());
            }
            4 => return Ok(()),
            _ => unreachable!(),
        }
    }
}

fn scan_projects(projects_root_dir: &Path) -> Result<Vec<Project>> {
    if !projects_root_dir.is_dir() {
        bail!("项目根目录不存在: {}", projects_root_dir.display());
    }

    let mut projects = Vec::new();
    for entry in fs::read_dir(projects_root_dir)
        .with_context(|| format!("读取项目根目录失败: {}", projects_root_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if path.join(".git").exists() {
            let name = path
                .file_name()
                .map(|v| v.to_string_lossy().to_string())
                .ok_or_else(|| anyhow!("项目目录名称无效: {}", path.display()))?;
            projects.push(Project { name, path });
        }
    }

    projects.sort_by(|a, b| a.name.cmp(&b.name));

    if projects.is_empty() {
        bail!(
            "在目录 {} 中未找到任何 Git 仓库。请确认你的项目目录结构或重新配置。",
            projects_root_dir.display()
        );
    }

    Ok(projects)
}

fn select_project(theme: &ColorfulTheme, projects: &[Project], prompt: &str) -> Result<Project> {
    let labels: Vec<String> = projects
        .iter()
        .map(|project| format!("{}  ({})", project.name, project.path.display()))
        .collect();

    let index = Select::with_theme(theme)
        .with_prompt(prompt)
        .items(&labels)
        .default(0)
        .interact()
        .context("读取项目选择失败")?;

    Ok(projects[index].clone())
}

fn do_create_worktree(
    theme: &ColorfulTheme,
    config: &AppConfig,
    projects: &[Project],
) -> Result<()> {
    let project = select_project(theme, projects, "选择一个 Git 仓库")?;

    let new_branch = Input::<String>::with_theme(theme)
        .with_prompt(format!("请输入新分支名称（项目: {}）", project.name))
        .default("feat/my-feature".to_string())
        .interact_text()
        .context("读取分支名称失败")?;

    let base_options = remote_branches(&project.path)?;
    let base_index = Select::with_theme(theme)
        .with_prompt("选择基准分支")
        .items(&base_options)
        .default(default_base_branch_index(&base_options))
        .interact()
        .context("读取基准分支选择失败")?;
    let base_branch = &base_options[base_index];

    let dir_name = branch_to_dirname(&new_branch);
    let wt_path = config
        .worktrees_root_dir
        .join(&project.name)
        .join(&dir_name);

    if wt_path.exists() {
        bail!("Worktree 目录已存在: {}", wt_path.display());
    }

    println!("[INFO] 项目: {}", project.name);
    println!("[INFO] 新分支: {}", new_branch);
    println!("[INFO] 基准分支: origin/{base_branch}");
    println!("[INFO] Worktree 路径: {}", wt_path.display());

    println!("[INFO] 正在 fetch 远程仓库...");
    run_git(&project.path, &["fetch", "origin"])?;

    fs::create_dir_all(
        wt_path
            .parent()
            .ok_or_else(|| anyhow!("Worktree 路径无效: {}", wt_path.display()))?,
    )
    .with_context(|| format!("创建 Worktree 父目录失败: {}", wt_path.display()))?;

    println!("[INFO] 正在创建 worktree...");
    run_git(
        &project.path,
        &[
            "worktree",
            "add",
            "-b",
            &new_branch,
            &wt_path.to_string_lossy(),
            &format!("origin/{base_branch}"),
        ],
    )?;

    println!("[INFO] 正在推送新分支到远程...");
    if let Err(err) = run_git(&wt_path, &["push", "-u", "origin", &new_branch]) {
        println!("[WARNING] 推送远程分支失败，Worktree 已创建但未成功建立远程分支: {err}");
    } else {
        println!("[SUCCESS] 远程分支已创建并建立跟踪: origin/{new_branch}");
    }

    println!("[SUCCESS] Worktree 创建成功");
    println!("路径: {}", wt_path.display());
    println!("分支: {new_branch}");
    println!("远程: origin/{new_branch}");
    println!("cd {}", wt_path.display());

    if Confirm::with_theme(theme)
        .with_prompt(format!(
            "是否使用 {} 打开刚创建的 Worktree 项目？",
            config.ide_label
        ))
        .default(true)
        .interact()
        .context("读取打开项目确认失败")?
    {
        open_with_ide(config, &wt_path)?;
    }

    pause();
    Ok(())
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

fn open_with_ide(config: &AppConfig, project_path: &Path) -> Result<()> {
    let mut child = Command::new(&config.ide_command)
        .arg(project_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("执行 IDE 命令失败: {}", config.ide_command))?;

    let _ = child.try_wait();
    println!(
        "[SUCCESS] 已触发 {} 打开项目: {}",
        config.ide_label,
        project_path.display()
    );
    Ok(())
}

fn do_list_worktrees(theme: &ColorfulTheme, projects: &[Project]) -> Result<()> {
    let project = select_project(theme, projects, "选择要查看的项目")?;
    let worktrees = read_worktrees(&project.path)?;

    println!("== Worktree 列表: {} ==", project.name);
    for (index, worktree) in worktrees.iter().enumerate() {
        let branch = worktree
            .branch
            .clone()
            .unwrap_or_else(|| "(detached)".to_string());
        let main_flag = if worktree.path == project.path {
            " (主仓库)"
        } else {
            ""
        };
        println!("{}. {}{}", index + 1, branch, main_flag);
        println!("   路径: {}", worktree.path.display());
        println!("   提交: {}", worktree.head);
    }
    println!("共 {} 个 worktree", worktrees.len());

    pause();
    Ok(())
}

fn do_remove_worktree(theme: &ColorfulTheme, projects: &[Project]) -> Result<()> {
    let project = select_project(theme, projects, "选择要操作的项目")?;
    let worktrees = read_worktrees(&project.path)?;
    let removable: Vec<WorktreeEntry> = worktrees
        .into_iter()
        .filter(|entry| entry.path != project.path && !entry.bare)
        .collect();

    if removable.is_empty() {
        println!("[INFO] 项目 {} 没有可删除的 worktree", project.name);
        pause();
        return Ok(());
    }

    let labels: Vec<String> = removable
        .iter()
        .map(|entry| {
            format!(
                "{}  ({})",
                entry
                    .branch
                    .clone()
                    .unwrap_or_else(|| "(detached)".to_string()),
                entry.path.display()
            )
        })
        .collect();

    let selection = Select::with_theme(theme)
        .with_prompt("选择要删除的 worktree")
        .items(&labels)
        .default(0)
        .interact()
        .context("读取 worktree 删除选择失败")?;

    let selected = removable[selection].clone();
    let branch_name = selected.branch.clone().ok_or_else(|| {
        anyhow!(
            "无法删除 detached HEAD worktree，请手动处理: {}",
            selected.path.display()
        )
    })?;

    println!("[INFO] 即将删除 Worktree");
    println!("分支: {}", branch_name);
    println!("路径: {}", selected.path.display());

    if let Err(err) = run_git(
        &project.path,
        &["worktree", "remove", &selected.path.to_string_lossy()],
    ) {
        println!("[WARNING] 普通删除失败，尝试强制删除: {err}");
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
    println!("[SUCCESS] Worktree 已删除: {}", selected.path.display());

    if Confirm::with_theme(theme)
        .with_prompt(format!("是否同时删除本地分支 {}？", branch_name))
        .default(false)
        .interact()
        .context("读取删除分支确认失败")?
    {
        if let Err(err) = run_git(&project.path, &["branch", "-d", &branch_name]) {
            println!("[WARNING] git branch -d 失败，尝试强制删除: {err}");
            run_git(&project.path, &["branch", "-D", &branch_name])?;
        }
        println!("[SUCCESS] 本地分支已删除: {}", branch_name);

        if remote_branch_exists(&project.path, &branch_name)? {
            if Confirm::with_theme(theme)
                .with_prompt(format!("是否同时删除远程分支 origin/{}？", branch_name))
                .default(false)
                .interact()
                .context("读取删除远程分支确认失败")?
            {
                run_git(&project.path, &["push", "origin", "--delete", &branch_name])?;
                println!("[SUCCESS] 远程分支已删除: origin/{}", branch_name);
            }
        }
    }

    pause();
    Ok(())
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
    let status = Command::new("git")
        .arg("-C")
        .arg(project_path)
        .args(args)
        .status()
        .with_context(|| {
            format!(
                "执行 git 命令失败: git -C {} {}",
                project_path.display(),
                args.join(" ")
            )
        })?;

    if !status.success() {
        bail!(
            "git 命令执行失败: git -C {} {}",
            project_path.display(),
            args.join(" ")
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

fn pause() {
    print!("按回车继续...");
    let _ = io::stdout().flush();
    let mut line = String::new();
    let _ = io::stdin().read_line(&mut line);
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
