//! Application state and the update function. The workbench layout lives in `workbench`.
//!
//! The shape mirrors the web app: a project, editor tabs, one active tool window per dock, a
//! command registry with `when` contexts, a keymap, a status bar and the command palette.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use iced::Task;
use iced_editor::{EditorState, Shared};

use crate::fs;

use crate::i18n::{self, Language};
use crate::registry::{Command, Dock, TOOL_WINDOWS, ToolWindow, When};
use crate::tokens::Tokens;

pub struct Tab {
    pub title: String,
    /// Where the tab came from, for the language label and for reopening.
    pub path: Option<PathBuf>,
    pub editor: Shared,
}

pub struct Project {
    pub name: String,
    pub root: PathBuf,
}

pub struct App {
    pub tokens: Tokens,
    pub light: bool,
    pub language: Language,
    /// The open project, if any.
    pub project: Option<Project>,
    /// Directories the tree has expanded, with their children already read.
    pub expanded: HashSet<PathBuf>,
    pub children: HashMap<PathBuf, Vec<fs::Entry>>,
    /// A one-line message for the status bar (unreadable file, missing project, …).
    pub notice: Option<String>,
    pub tabs: Vec<Tab>,
    pub active_tab: usize,
    /// One active tool window per dock, like the web workbench stripes.
    pub left: Option<&'static str>,
    /// Panel width in pixels. The web stores this per window; a fresh session there is 403px
    /// wide, which is what the comparison screenshots use.
    pub left_width: f32,
    pub right: Option<&'static str>,
    pub bottom: Option<&'static str>,
    pub palette: Option<String>,
    /// The main menu is open (a stand-in until the menu bar is ported).
    pub menu: bool,
    /// Which tool window the pointer is over; its header actions show only then.
    pub dock_hovered: Option<&'static str>,
    pub palette_input: iced::advanced::widget::Id,
    /// Stable id for the editor widget; iced routes IME through focus, which needs one.
    editor_id: iced::advanced::widget::Id,
    pub status: String,
    /// The file selected in the project tree, to show selection in the panel.
    pub selected: Option<PathBuf>,
    /// Kept so the title bar's minimize/maximize/close can address this window.
    window_id: Option<iced::window::Id>,
}

#[derive(Debug, Clone)]
pub enum Message {
    ShowTool(&'static str),
    HideTool(Dock),
    ActivateTab(usize),
    CloseTab(usize),
    ToggleDir(PathBuf),
    OpenFile(PathBuf),
    TogglePalette,
    /// The ☰ button's menu (the web's MainMenu).
    ToggleMenu,
    /// The pointer entered or left a tool window island.
    HoverDock(Option<&'static str>),
    PaletteInput(String),
    PaletteSubmit,
    RunCommand(&'static str),
    Key(iced::keyboard::Event),
    ToggleTheme,
    ToggleLanguage,
    /// The splitter moved to this window x; the left panel takes the space up to it.
    ResizeDock(f32),
    /// The window id, fetched once at boot so the title bar's controls can act on it.
    WindowId(Option<iced::window::Id>),
    Minimize,
    Maximize,
    CloseWindow,
}

impl App {
    pub fn boot() -> (Self, Task<Message>) {
        let welcome =
            Tab { title: "welcome.md".to_owned(), path: None, editor: Shared::new(EditorState::new(WELCOME)) };

        // Open the same project the web version opens, so the two can be compared.
        let root = PathBuf::from("/home/limstorm/gowork/src/gitee.com/kwbasedb");
        let project = root.is_dir().then(|| Project { name: "kwbasedb".to_owned(), root: root.clone() });

        let mut children = HashMap::new();
        let mut expanded = HashSet::new();
        let mut notice = None;
        if let Some(project) = &project {
            match fs::read_dir(&project.root) {
                Ok(entries) => {
                    children.insert(project.root.clone(), entries);
                    expanded.insert(project.root.clone());
                }
                Err(error) => notice = Some(error),
            }
        }

        let mut app = Self {
            tokens: Tokens::light(),
            light: true,
            language: Language::Zh,
            project,
            expanded,
            children,
            notice,
            tabs: vec![welcome],
            active_tab: 0,
            left: Some("project"),
            left_width: load_panel_width().unwrap_or(403.0),
            right: None,
            bottom: None,
            palette: std::env::var_os("SHELL_PALETTE").map(|_| String::new()),
            menu: false,
            dock_hovered: None,
            palette_input: iced::advanced::widget::Id::unique(),
            editor_id: iced::advanced::widget::Id::unique(),
            status: i18n::t(Language::Zh, "status.ready").to_owned(),
            selected: None,
            window_id: None,
        };

        if let Some(path) = std::env::var_os("SHELL_OPEN") {
            app.open_file(PathBuf::from(path));
        }

        let id = iced::window::latest().map(Message::WindowId);
        (app, id)
    }

    pub fn title(&self) -> String {
        match &self.project {
            Some(project) => format!("{} – {}", project.name, self.tabs[self.active_tab].title),
            None => i18n::t(self.language, "app.title").to_owned(),
        }
    }

    /// The branch shown in the status bar, from the project's repository.
    pub fn branch(&self) -> Option<String> {
        self.project.as_ref().and_then(|project| fs::git_branch(&project.root))
    }

    /// Indentation and selection of the active tab, as the web status bar reports them.
    pub fn indentation(&self) -> Option<String> {
        self.tabs.get(self.active_tab).and_then(|tab| {
            let state = tab.editor.borrow();
            fs::indentation(&state.text())
        })
    }

    pub fn selection_chars(&self) -> Option<usize> {
        self.tabs.get(self.active_tab).and_then(|tab| {
            let state = tab.editor.borrow();
            state.selection().map(|(start, end)| end - start)
        })
    }

    /// Language of the active tab, as shown in the status bar.
    pub fn active_language(&self) -> String {
        self.tabs
            .get(self.active_tab)
            .and_then(|tab| tab.path.as_deref())
            .map_or_else(|| "Plain text".to_owned(), fs::language_of)
    }

    pub fn editor_id(&self) -> iced::advanced::widget::Id {
        self.editor_id.clone()
    }

    pub fn tokens(&self) -> &Tokens {
        &self.tokens
    }

    pub fn language(&self) -> Language {
        self.language
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ShowTool(id) => {
                let Some(tool) = ToolWindow::by_id(id) else { return Task::none() };
                let slot = match tool.dock {
                    Dock::Left => &mut self.left,
                    Dock::Right => &mut self.right,
                    Dock::Bottom => &mut self.bottom,
                };
                *slot = if *slot == Some(tool.id) { None } else { Some(tool.id) };
            }
            Message::HideTool(dock) => {
                let slot = match dock {
                    Dock::Left => &mut self.left,
                    Dock::Right => &mut self.right,
                    Dock::Bottom => &mut self.bottom,
                };
                *slot = None;
            }
            Message::ActivateTab(index) => self.active_tab = index.min(self.tabs.len() - 1),
            Message::CloseTab(index) => {
                if index < self.tabs.len() {
                    self.tabs.remove(index);
                    if self.tabs.is_empty() {
                        self.open_welcome();
                    }
                    self.active_tab = self.active_tab.min(self.tabs.len().saturating_sub(1));
                }
            }
            Message::ToggleDir(path) => {
                if !self.expanded.remove(&path) {
                    if !self.children.contains_key(&path) {
                        match fs::read_dir(&path) {
                            Ok(entries) => {
                                self.children.insert(path.clone(), entries);
                            }
                            Err(error) => {
                                self.notice = Some(error);
                                return Task::none();
                            }
                        }
                    }
                    self.expanded.insert(path);
                }
            }
            Message::OpenFile(path) => self.open_file(path),
            Message::HoverDock(id) => self.dock_hovered = id,
            Message::ToggleMenu => self.menu = !self.menu,
            Message::TogglePalette => {
                self.palette = if self.palette.is_some() { None } else { Some(String::new()) };
                if self.palette.is_some() {
                    return iced::widget::operation::focus(self.palette_input.clone());
                }
            }
            Message::PaletteInput(value) => {
                if self.palette.is_some() {
                    self.palette = Some(value);
                }
            }
            Message::PaletteSubmit => {
                if std::env::var_os("SHELL_TRACE").is_some() {
                    eprintln!("palette submit (filter {:?})", self.palette);
                }
                let filter = self.palette.take().unwrap_or_default().to_lowercase();
                if let Some(command) = self.matching_commands(&filter).first().copied()
                    && let Some(command) = Command::by_id(command)
                {
                    return Task::done(Message::RunCommand(command.id));
                }
            }
            Message::RunCommand(id) => return self.run_command(id),
            Message::Key(iced::keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                use iced::keyboard::key;
                if std::env::var_os("SHELL_TRACE").is_some() {
                    eprintln!("key: {key:?} ctrl={} shift={}", modifiers.control(), modifiers.shift());
                }
                if let key::Key::Named(key::Named::Escape) = key.as_ref() {
                    self.palette = None;
                    return Task::none();
                }
                // Enter submits the palette even if the input did not take focus, so the
                // keyboard path does not depend on the text input's own `on_submit`.
                if let key::Key::Named(key::Named::Enter) = key.as_ref()
                    && self.palette.is_some()
                {
                    return Task::done(Message::PaletteSubmit);
                }
                if let key::Key::Character(character) = key.as_ref()
                    && let Some(binding) = crate::registry::KEYMAP.iter().find(|binding| {
                        binding.chord.matches(character, modifiers.control(), modifiers.shift())
                            && App::when_available(binding.when, self.project.is_some(), !self.tabs.is_empty())
                    })
                {
                    if binding.command == "palette" {
                        return Task::done(Message::TogglePalette);
                    }
                    return Task::done(Message::RunCommand(binding.command));
                }
            }
            Message::Key(_) => {}
            Message::ResizeDock(x) => {
                let margin = crate::metrics::STRIPE + 6.0;
                self.left_width = (x - margin).clamp(160.0, 900.0);
                save_panel_width(self.left_width);
            }
            Message::WindowId(id) => self.window_id = id,
            Message::Minimize => {
                if let Some(id) = self.window_id {
                    return iced::window::minimize(id, true);
                }
            }
            Message::Maximize => {
                if let Some(id) = self.window_id {
                    return iced::window::toggle_maximize(id);
                }
            }
            Message::CloseWindow => {
                if let Some(id) = self.window_id {
                    return iced::window::close(id);
                }
            }
            Message::ToggleTheme => {
                self.light = !self.light;
                self.tokens = if self.light { Tokens::light() } else { Tokens::dark() };
            }
            Message::ToggleLanguage => {
                self.language = match self.language {
                    Language::En => Language::Zh,
                    Language::Zh => Language::En,
                };
            }
        }
        Task::none()
    }

    /// Opens (or focuses) a file, computing its highlights with the language grammars.
    pub fn open_file(&mut self, path: PathBuf) {
        self.selected = Some(path.clone());
        if let Some(index) = self.tabs.iter().position(|tab| tab.path.as_deref() == Some(path.as_path())) {
            self.active_tab = index;
            return;
        }

        match fs::read_text(&path) {
            Ok(text) => {
                let mut state = EditorState::new(&text);
                state.font_size = crate::metrics::FONT_CODE;
                state.line_height = crate::metrics::EDITOR_LINE;
                // Re-highlight after every edit: the widget calls this hook once the rope has
                // changed, so typing a `//` turns the line into a comment immediately.
                let language_path = path.clone();
                state.set_highlighter(Box::new(move |text| fs::highlights(&language_path, text)));
                self.tabs.push(Tab { title: fs::display_name(&path), path: Some(path), editor: Shared::new(state) });
                self.active_tab = self.tabs.len() - 1;
                self.notice = None;
            }
            Err(error) => self.notice = Some(error),
        }
    }

    fn open_welcome(&mut self) {
        self.tabs.push(Tab {
            title: "welcome.md".to_owned(),
            path: None,
            editor: Shared::new(EditorState::new(WELCOME)),
        });
        self.active_tab = 0;
    }

    /// Commands matching the palette filter, in registry order.
    pub fn matching_commands(&self, filter: &str) -> Vec<&'static str> {
        let has_project = self.project.is_some();
        let has_editor = !self.tabs.is_empty();
        crate::registry::COMMANDS
            .iter()
            .filter(|command| command.available(has_project, has_editor))
            .filter(|command| filter.is_empty() || command.title.to_lowercase().contains(filter))
            .map(|command| command.id)
            .collect()
    }

    fn run_command(&mut self, id: &'static str) -> Task<Message> {
        match id {
            "view.project" => self.left = Some("project"),
            "view.search" => self.left = Some("search"),
            "view.git" => self.left = Some("git"),
            "view.problems" => self.bottom = Some("problems"),
            "view.terminal" => self.bottom = Some("terminal"),
            "view.hideTool" => {
                self.left = None;
                self.right = None;
                self.bottom = None;
            }
            "editor.close" => return Task::done(Message::CloseTab(self.active_tab)),
            "app.toggleTheme" => return Task::done(Message::ToggleTheme),
            "app.toggleLanguage" => return Task::done(Message::ToggleLanguage),
            "app.quit" => return iced::exit(),
            "file.openProject" => {
                let root = PathBuf::from("/home/limstorm/gowork/src/gitee.com/kwbasedb");
                match fs::read_dir(&root) {
                    Ok(entries) => {
                        self.children.clear();
                        self.expanded.clear();
                        self.children.insert(root.clone(), entries);
                        self.expanded.insert(root.clone());
                        self.project = Some(Project { name: "kwbasedb".to_owned(), root });
                        self.notice = None;
                    }
                    Err(error) => self.notice = Some(error),
                }
            }
            "file.closeProject" => {
                self.project = None;
                self.expanded.clear();
                self.children.clear();
                self.tabs.clear();
                self.open_welcome();
            }
            other => self.status = format!("unhandled: {other}"),
        }
        Task::none()
    }

    /// Tool windows that share a dock, for the dock's own tab strip.
    pub fn dock_titles(dock: Dock, language: Language) -> Vec<(&'static str, &'static str)> {
        TOOL_WINDOWS
            .iter()
            .filter(|tool| tool.dock == dock)
            .map(|tool| (tool.id, i18n::t(language, tool.title_key)))
            .collect()
    }

    pub fn when_available(when: When, has_project: bool, has_editor: bool) -> bool {
        match when {
            When::Always => true,
            When::Project => has_project,
            When::Editor => has_editor,
        }
    }
}

const WELCOME: &str = "# OpenCLion（iced 原生外壳）\n\n\
这是 M1 的骨架：左侧活动栏 + 工具窗、编辑器标签、底部状态栏、命令面板。\n\
文本区复用 spike/iced-editor 的 widget（rope 缓冲、光标、选区、中文 IME）。\n\n\
## 和 web 版的对应关系\n\n\
| web | 这里 |\n\
| --- | --- |\n\
| registerToolWindow | registry::TOOL_WINDOWS |\n\
| registerActions | registry::COMMANDS（带 when 上下文）|\n\
| keymap when 上下文 | registry::KEYMAP |\n\
| CSS 变量 | tokens::Tokens |\n\
| locales JSON | i18n::t |\n\n\
## 试一下\n\n\
- Ctrl+Shift+P 打开命令面板\n\
- 活动栏图标切工具窗（Project / Search / Git / Problems / Terminal）\n\
- 状态栏右侧切主题、切语言\n";

/// The web stores panel widths in the window's workspace; the prototype keeps the one value
/// it has in a small state file so a restart looks the same.
fn state_file() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".cache/iced-shell/state"))
}

pub fn load_panel_width() -> Option<f32> {
    let contents = std::fs::read_to_string(state_file()?).ok()?;
    contents.lines().find_map(|line| line.strip_prefix("left_width=")?.parse().ok())
}

fn save_panel_width(width: f32) {
    let Some(path) = state_file() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, format!("left_width={width:.0}\n"));
}
