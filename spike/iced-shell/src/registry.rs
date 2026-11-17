//! The registries the web workbench exposes — tool windows, commands, status items, editor
//! kinds and key bindings — reduced to the shape a native workbench can iterate over.
//!
//! Ids and titles are copied from `src/features/*/index.ts` (`registerToolWindow`,
//! `registerActions`, …) so the native side can be diffed against the web side.

/// Where a tool window lives when it is first opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dock {
    Left,
    Right,
    Bottom,
}

#[derive(Debug, Clone, Copy)]
pub struct ToolWindow {
    pub id: &'static str,
    /// i18n key, not the label: the label depends on the current language.
    pub title_key: &'static str,
    pub dock: Dock,
}

pub const TOOL_WINDOWS: &[ToolWindow] = &[
    ToolWindow { id: "project", title_key: "tool.project", dock: Dock::Left },
    ToolWindow { id: "structure", title_key: "tool.structure", dock: Dock::Left },
    ToolWindow { id: "search", title_key: "tool.search", dock: Dock::Left },
    ToolWindow { id: "git", title_key: "tool.git", dock: Dock::Left },
    ToolWindow { id: "commit", title_key: "tool.commit", dock: Dock::Left },
    ToolWindow { id: "problems", title_key: "tool.problems", dock: Dock::Bottom },
    ToolWindow { id: "find", title_key: "tool.find", dock: Dock::Bottom },
    ToolWindow { id: "usages", title_key: "tool.usages", dock: Dock::Bottom },
    ToolWindow { id: "terminal", title_key: "tool.terminal", dock: Dock::Bottom },
    ToolWindow { id: "run", title_key: "tool.run", dock: Dock::Bottom },
    ToolWindow { id: "todo", title_key: "tool.todo", dock: Dock::Bottom },
    ToolWindow { id: "cmake", title_key: "tool.cmake", dock: Dock::Right },
    ToolWindow { id: "build", title_key: "tool.build", dock: Dock::Right },
    ToolWindow { id: "database", title_key: "tool.database", dock: Dock::Right },
];

impl ToolWindow {
    pub fn by_id(id: &str) -> Option<&'static Self> {
        TOOL_WINDOWS.iter().find(|tool| tool.id == id)
    }
}

/// When a command is available. The web workbench has a full `when` expression language; this is
/// the subset the shell needs today, with the same ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum When {
    Always,
    Project,
    Editor,
}

#[derive(Debug, Clone, Copy)]
pub struct Command {
    pub id: &'static str,
    pub title: &'static str,
    pub when: When,
}

pub const COMMANDS: &[Command] = &[
    Command { id: "file.openProject", title: "Open Project…", when: When::Always },
    Command { id: "file.closeProject", title: "Close Project", when: When::Project },
    Command { id: "view.project", title: "View: Project", when: When::Always },
    Command { id: "view.search", title: "View: Search", when: When::Always },
    Command { id: "view.git", title: "View: Git", when: When::Always },
    Command { id: "view.problems", title: "View: Problems", when: When::Always },
    Command { id: "view.terminal", title: "View: Terminal", when: When::Always },
    Command { id: "view.hideTool", title: "View: Hide Tool Window", when: When::Always },
    Command { id: "app.toggleTheme", title: "Appearance: Toggle Theme", when: When::Always },
    Command { id: "app.toggleLanguage", title: "Appearance: Toggle Language", when: When::Always },
    Command { id: "editor.close", title: "Editor: Close Tab", when: When::Editor },
    Command { id: "app.quit", title: "Quit OpenCLion", when: When::Always },
];

impl Command {
    pub fn by_id(id: &str) -> Option<&'static Self> {
        COMMANDS.iter().find(|command| command.id == id)
    }

    pub fn available(&self, has_project: bool, has_editor: bool) -> bool {
        match self.when {
            When::Always => true,
            When::Project => has_project,
            When::Editor => has_editor,
        }
    }
}

/// A key binding: the chord, the command it runs, and the context it is valid in.
#[derive(Debug, Clone, Copy)]
pub struct Binding {
    pub chord: Chord,
    pub command: &'static str,
    pub when: When,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chord {
    Ctrl(char),
    CtrlShift(char),
}

pub const KEYMAP: &[Binding] = &[
    Binding { chord: Chord::CtrlShift('P'), command: "palette", when: When::Always },
    Binding { chord: Chord::CtrlShift('N'), command: "file.openProject", when: When::Always },
    Binding { chord: Chord::Ctrl('W'), command: "editor.close", when: When::Editor },
    Binding { chord: Chord::Ctrl('\\'), command: "app.toggleTheme", when: When::Always },
];

impl Chord {
    pub fn matches(self, character: &str, control: bool, shift: bool) -> bool {
        match (self, character) {
            (Chord::Ctrl(expected), c) => control && !shift && c.eq_ignore_ascii_case(&expected.to_string()),
            (Chord::CtrlShift(expected), c) => control && shift && c.eq_ignore_ascii_case(&expected.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_window_ids_are_unique() {
        let mut ids: Vec<&str> = TOOL_WINDOWS.iter().map(|tool| tool.id).collect();
        ids.sort_unstable();
        let count = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), count, "duplicate tool window id");
    }

    #[test]
    fn command_ids_are_unique() {
        let mut ids: Vec<&str> = COMMANDS.iter().map(|command| command.id).collect();
        ids.sort_unstable();
        let count = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), count, "duplicate command id");
    }

    #[test]
    fn keymap_covers_the_palette_and_never_binds_an_unknown_command() {
        for binding in KEYMAP {
            if binding.command == "palette" {
                continue; // handled by the shell itself, not by the command registry
            }
            assert!(Command::by_id(binding.command).is_some(), "unknown command in keymap: {}", binding.command);
        }
        assert!(KEYMAP.iter().any(|binding| binding.command == "palette"));
    }

    #[test]
    fn chords_match_only_with_their_modifiers() {
        assert!(Chord::CtrlShift('P').matches("P", true, true));
        assert!(!Chord::CtrlShift('P').matches("p", true, false));
        assert!(Chord::Ctrl('W').matches("w", true, false));
        assert!(!Chord::Ctrl('W').matches("w", true, true));
    }
}
