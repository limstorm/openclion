//! clangd through `ide-lsp`: documents are opened, changed and closed on the
//! manager; diagnostics arrive on its event sink and drive the native
//! editor's squiggles and the status bar.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use ide_core::{
    CancellationToken, Diagnostic, DocId, EventSink, FileEdit, IdeEvent, LanguageId, Pos, Range, Severity, SharedSink,
    WorkspaceEdit, event::ServerStatus,
};
use ide_language::LanguageRegistry;
use ide_lsp::LspManager;
use ide_lsp::types::HierarchyDirection;
use ide_text::AppliedChange;
use nsh_base::editor::{
    Editor, EditorApi, EditorCompletionItem, EditorCompletionKind, EditorCompletionRequest, EditorDiagnostic,
    EditorDiagnosticSeverity, EditorDiagnosticTag, EditorPosition, EditorRange, EditorSelection,
};
use nsh_core::prelude::*;
use std::time::Duration;

use crate::model::State;

const DOC: DocId = DocId(1);

#[derive(Clone)]
pub struct Session {
    manager: Arc<LspManager>,
    document: Option<Open>,
}

#[derive(Clone)]
struct Open {
    path: PathBuf,
    version: u32,
    text: String,
}

/// The sink `ide-lsp` emits language-server events on; it hands them to the
/// engine thread through a channel.
struct Sink {
    tx: tokio::sync::mpsc::UnboundedSender<IdeEvent>,
}

impl EventSink for Sink {
    fn emit(&self, event: IdeEvent) {
        let _ = self.tx.send(event);
    }
}

/// Creates the manager for the workspace and starts draining its events.
pub fn start(state: State) {
    if std::env::var_os("OPENCLION_NO_LSP").is_some() {
        return;
    }
    let root = state.root.get();
    let Ok(registry) = LanguageRegistry::load(None) else {
        state.lsp_status.set("languages unavailable".to_owned());
        return;
    };
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let sink: SharedSink = Arc::new(Sink { tx });
    let manager = LspManager::new(root, Arc::new(registry), sink);
    state.lsp.replace(Some(Session { manager, document: None }));
    spawn(async move {
        while let Some(event) = rx.recv().await {
            handle(state, event);
        }
    });
}

fn handle(state: State, event: IdeEvent) {
    match event {
        IdeEvent::Diagnostics { path, items, .. } => {
            let path = path.into_path_buf();
            let active = state.active_path.get();
            let converted: Vec<EditorDiagnostic> = items.iter().map(convert).collect();
            state.diagnostics_cache.with(|cache| {
                cache.insert(path.clone(), items);
            });
            state.diagnostics_rev.update(|rev| *rev += 1);
            if path == active {
                state.diagnostics.set(converted);
            }
        }
        IdeEvent::LanguageServer { name, status, message, .. } => {
            let label = match status {
                ServerStatus::Starting => "starting",
                ServerStatus::Ready => "ready",
                ServerStatus::Busy => "indexing",
                ServerStatus::Crashed => "crashed",
                ServerStatus::Stopped => "stopped",
                ServerStatus::Missing => "not installed",
                ServerStatus::Misconfigured => "misconfigured",
            };
            let text = match message {
                Some(message) => format!("{name}: {label} — {message}"),
                None => format!("{name}: {label}"),
            };
            state.lsp_status.set(text);
        }
        _ => {}
    }
}

fn convert(diagnostic: &Diagnostic) -> EditorDiagnostic {
    let mut out = EditorDiagnostic::default();
    out.range = EditorRange::new(
        EditorPosition::new(diagnostic.range.start.line, diagnostic.range.start.col),
        EditorPosition::new(diagnostic.range.end.line, diagnostic.range.end.col),
    );
    out.severity = match diagnostic.severity {
        Severity::Error => EditorDiagnosticSeverity::Error,
        Severity::Warning => EditorDiagnosticSeverity::Warning,
        Severity::Info => EditorDiagnosticSeverity::Info,
        Severity::Hint => EditorDiagnosticSeverity::Hint,
    };
    out.message = diagnostic.message.clone().into();
    out.source = diagnostic.source.clone().map(Into::into);
    out.code = diagnostic.code.clone().map(Into::into);
    if diagnostic.unnecessary {
        out.tags.push(EditorDiagnosticTag::Unnecessary);
    }
    if diagnostic.deprecated {
        out.tags.push(EditorDiagnosticTag::Deprecated);
    }
    out
}

/// `didOpen` for the document the editor switched to; the server starts on
/// the first call.
pub fn open(state: State, path: &Path, language: &str, text: &str) {
    let Some(mut session) = state.lsp.get() else {
        return;
    };
    if session.document.as_ref().is_some_and(|open| open.path == path) {
        return;
    }
    if session.document.is_some() {
        session.manager.did_close(DOC);
    }
    session.manager.did_open(DOC, path, &LanguageId::new(language.to_owned()), 1, text);
    session.document = Some(Open { path: path.to_path_buf(), version: 1, text: text.to_owned() });
    state.lsp.replace(Some(session));
    state.diagnostics_cache.with(|cache| {
        let cached = cache.get(path).cloned().unwrap_or_default();
        state.diagnostics.set(cached.iter().map(convert).collect());
    });
}

/// `didChange` with one replacement covering the document: simple and valid
/// for both full and incremental synchronisation.
pub fn change(state: State, text: &str) {
    let Some(mut session) = state.lsp.get() else {
        return;
    };
    let Some(open) = session.document.as_mut() else {
        return;
    };
    let end = end_position(&open.text);
    let version = open.version + 1;
    let change = AppliedChange { range: Range::new(Pos::new(0, 0), end), text: text.to_owned() };
    let full = text.to_owned();
    session.manager.did_change(DOC, version, &[change], &|| full.clone());
    open.version = version;
    open.text = text.to_owned();
    state.lsp.replace(Some(session));
}

/// `didClose` when the editor has no document.
pub fn close(state: State) {
    let Some(mut session) = state.lsp.get() else {
        return;
    };
    if session.document.is_some() {
        session.manager.did_close(DOC);
        session.document = None;
        state.lsp.replace(Some(session));
    }
    state.diagnostics.set(Vec::new());
}

fn end_position(text: &str) -> Pos {
    let mut line = 0;
    let mut col = 0;
    for ch in text.chars() {
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += ch.len_utf16() as u32;
        }
    }
    Pos::new(line, col)
}

/// Answers a completion request with clangd's items, presented in the native
/// completion list.
pub fn completion(state: State, request: &EditorCompletionRequest, editor: ComponentRef<Editor>) {
    let Some(session) = state.lsp.get() else {
        return;
    };
    let manager = session.manager.clone();
    let request = request.clone();
    spawn(async move {
        let position = Pos::new(request.position.line, request.position.column);
        let Ok(list) = manager.completion(DOC, position, request.character.clone(), CancellationToken::new()).await
        else {
            return;
        };
        let items: Vec<EditorCompletionItem> = list.items.iter().map(complete).collect();
        if let Some(api) = editor.api::<EditorApi>() {
            api.present_completions(request.id, items, Some(list.is_incomplete));
        }
    });
}

fn complete(item: &ide_lsp::types::CompletionItem) -> EditorCompletionItem {
    let mut out = EditorCompletionItem::new(item.label.clone());
    out.kind = item.kind.map(|kind| match kind {
        ide_lsp::types::CompletionKind::Text => EditorCompletionKind::Text,
        ide_lsp::types::CompletionKind::Method => EditorCompletionKind::Method,
        ide_lsp::types::CompletionKind::Function => EditorCompletionKind::Function,
        ide_lsp::types::CompletionKind::Constructor => EditorCompletionKind::Constructor,
        ide_lsp::types::CompletionKind::Field => EditorCompletionKind::Field,
        ide_lsp::types::CompletionKind::Variable => EditorCompletionKind::Variable,
        ide_lsp::types::CompletionKind::Class => EditorCompletionKind::Class,
        ide_lsp::types::CompletionKind::Interface => EditorCompletionKind::Interface,
        ide_lsp::types::CompletionKind::Module => EditorCompletionKind::Module,
        ide_lsp::types::CompletionKind::Property => EditorCompletionKind::Property,
        ide_lsp::types::CompletionKind::Unit => EditorCompletionKind::Unit,
        ide_lsp::types::CompletionKind::Value => EditorCompletionKind::Value,
        ide_lsp::types::CompletionKind::Enum => EditorCompletionKind::Enum,
        ide_lsp::types::CompletionKind::Keyword => EditorCompletionKind::Keyword,
        ide_lsp::types::CompletionKind::Snippet => EditorCompletionKind::Snippet,
        ide_lsp::types::CompletionKind::Color => EditorCompletionKind::Color,
        ide_lsp::types::CompletionKind::File => EditorCompletionKind::File,
        ide_lsp::types::CompletionKind::Reference => EditorCompletionKind::Reference,
        ide_lsp::types::CompletionKind::Folder => EditorCompletionKind::Folder,
        ide_lsp::types::CompletionKind::EnumMember => EditorCompletionKind::EnumMember,
        ide_lsp::types::CompletionKind::Constant => EditorCompletionKind::Constant,
        ide_lsp::types::CompletionKind::Struct => EditorCompletionKind::Struct,
        ide_lsp::types::CompletionKind::Event => EditorCompletionKind::Event,
        ide_lsp::types::CompletionKind::Operator => EditorCompletionKind::Operator,
        ide_lsp::types::CompletionKind::TypeParameter => EditorCompletionKind::TypeParameter,
    });
    out.detail = item.detail.clone().or_else(|| item.label_detail.clone()).map(Into::into);
    out.documentation = item.documentation.as_ref().map(|markup| markup.value.clone().into());
    out.insert_text = Some(item.insert_text.clone().into());
    out.filter_text = item.filter_text.clone().map(Into::into);
    out.sort_text = item.sort_text.clone().map(Into::into);
    out.range = item.replace_range.map(|range| {
        EditorRange::new(
            EditorPosition::new(range.start.line, range.start.col),
            EditorPosition::new(range.end.line, range.end.col),
        )
    });
    out
}

/// Go to definition (F12 or Mod-click): opens the target file and places the
/// caret at the definition.
pub fn definition(state: State, position: EditorPosition, editor: ComponentRef<Editor>) {
    let Some(session) = state.lsp.get() else {
        return;
    };
    let manager = session.manager.clone();
    spawn(async move {
        let pos = Pos::new(position.line, position.column);
        let Ok(locations) = manager.goto(DOC, pos, ide_lsp::types::GotoKind::Definition).await else {
            return;
        };
        let Some(location) = locations.into_iter().next() else {
            return;
        };
        let target = location.path.into_path_buf();
        let line = location.range.start.line;
        let col = location.range.start.col;
        state.open_path(target);
        set_timeout(Duration::from_millis(80), move || {
            if let Some(api) = editor.api::<EditorApi>() {
                api.set_selections(vec![EditorSelection::caret(EditorPosition::new(line, col))]);
                api.scroll_to_line(line);
            }
        });
    });
}

/// Renames the symbol at `line`/`col` (0-based) across the project and applies
/// clangd's workspace edit to disk and open buffers.
pub fn rename_symbol(state: State, line: u32, col: u32, new_name: String) {
    let Some(session) = state.lsp.get() else {
        return;
    };
    let manager = session.manager;
    state.status.set(format!("Renaming to {new_name}…"));
    spawn(async move {
        let outcome = tokio::time::timeout(
            Duration::from_secs(10),
            manager.rename(DOC, Pos::new(line, col), new_name.clone(), CancellationToken::new()),
        )
        .await;
        match outcome {
            Err(_) => {
                state.dialog.set(None);
                state.status.set("Rename timed out".to_owned());
            }
            Ok(Err(error)) => {
                state.dialog.set(None);
                state.status.set(format!("Rename failed: {error}"));
            }
            Ok(Ok(renamed)) => {
                let count = apply_workspace_edit(state, &renamed.edit);
                state.reload_tree();
                state.refresh_vcs();
                if let Some(tab) = state.active_tab() {
                    if let Some(api) = state.editor.api::<EditorApi>() {
                        api.set_value(tab.text.clone());
                    }
                    change(state, &tab.text);
                }
                state.dialog.set(None);
                state.status.set(format!("Renamed to {new_name} ({count} edit(s))"));
            }
        }
    });
}

fn apply_workspace_edit(state: State, edit: &WorkspaceEdit) -> usize {
    let mut count = 0;
    let mut updated: Vec<(std::path::PathBuf, String)> = Vec::new();
    for file in &edit.files {
        match file {
            FileEdit::Edit { path, edits } => {
                let path = path.clone().into_path_buf();
                let current = state
                    .tabs
                    .get()
                    .into_iter()
                    .find(|tab| tab.path == path)
                    .map(|tab| tab.text)
                    .or_else(|| std::fs::read_to_string(&path).ok());
                let Some(current) = current else {
                    continue;
                };
                let next = apply_edits(&current, edits);
                if std::fs::write(&path, &next).is_ok() {
                    updated.push((path, next));
                    count += edits.len();
                }
            }
            FileEdit::Rename { from, to } => {
                let from = from.clone().into_path_buf();
                let to = to.clone().into_path_buf();
                let _ = std::fs::rename(&from, &to);
                let mut tabs = state.tabs.get();
                for tab in &mut tabs {
                    if tab.path == from {
                        tab.path = to.clone();
                    }
                }
                state.tabs.set(tabs);
                count += 1;
            }
            FileEdit::Create { path, .. } => {
                let path = path.clone().into_path_buf();
                let _ = std::fs::write(&path, "");
                count += 1;
            }
            FileEdit::Delete { path, recursive } => {
                let path = path.clone().into_path_buf();
                let result = if *recursive { std::fs::remove_dir_all(&path) } else { std::fs::remove_file(&path) };
                if result.is_ok() {
                    count += 1;
                }
            }
        }
    }
    if !updated.is_empty() {
        let mut tabs = state.tabs.get();
        for tab in &mut tabs {
            if let Some((_, text)) = updated.iter().find(|(path, _)| *path == tab.path) {
                tab.text = text.clone();
                tab.modified = false;
            }
        }
        state.tabs.set(tabs);
        if let Some((_, text)) = updated.iter().find(|(path, _)| *path == state.active_path.get()) {
            state.editor_seed.set(text.clone());
        }
    }
    count
}

/// Code actions at the caret (Alt+Enter): quick fixes and refactors.
pub fn code_actions_at_caret(state: State) {
    let Some(session) = state.lsp.get() else {
        return;
    };
    let (line, col) = state.cursor.get();
    let pos = Pos::new(line.saturating_sub(1), col.saturating_sub(1));
    let manager = session.manager;
    spawn(async move {
        match manager.code_actions(DOC, Range::point(pos), Vec::new()).await {
            Ok(actions) if actions.is_empty() => {
                state.status.set("No code actions here".to_owned());
            }
            Ok(actions) => {
                state.code_action_titles.set(actions.iter().map(|action| action.title.clone()).collect());
                state.code_action_store.replace(actions);
                state.code_action_popup.set(true);
            }
            Err(error) => state.status.set(format!("Code actions failed: {error}")),
        }
    });
}

pub fn apply_code_action(state: State, index: usize) {
    state.code_action_popup.set(false);
    let Some(session) = state.lsp.get() else {
        return;
    };
    let Some(action) = state.code_action_store.with(|actions| actions.get(index).cloned()) else {
        return;
    };
    let manager = session.manager;
    spawn(async move {
        let action = match manager.resolve_code_action(DOC, action).await {
            Ok(action) => action,
            Err(error) => {
                state.status.set(format!("Code action failed: {error}"));
                return;
            }
        };
        if let Some(edit) = &action.edit {
            let count = apply_workspace_edit(state, edit);
            state.reload_tree();
            if let Some(tab) = state.active_tab() {
                if let Some(api) = state.editor.api::<EditorApi>() {
                    api.set_value(tab.text.clone());
                }
                change(state, &tab.text);
            }
            state.status.set(format!("Applied {} ({count} edit(s))", action.title));
        }
        if let Some(command) = action.command {
            if let Err(error) = manager.execute_command(DOC, command).await {
                state.status.set(format!("Code action command failed: {error}"));
            }
        }
    });
}

/// Type hierarchy (supertypes and subtypes) of the symbol at the caret.
pub fn type_hierarchy_at_caret(state: State) {
    let Some(session) = state.lsp.get() else {
        return;
    };
    let (line, col) = state.cursor.get();
    let pos = Pos::new(line.saturating_sub(1), col.saturating_sub(1));
    let manager = session.manager;
    state.status.set("Loading type hierarchy…".to_owned());
    spawn(async move {
        let mut rows = Vec::new();
        for (direction, relation) in
            [(HierarchyDirection::Incoming, "Supertype"), (HierarchyDirection::Outgoing, "Subtype")]
        {
            let items = manager.type_hierarchy(DOC, pos, direction).await.unwrap_or_default();
            for item in items {
                rows.push(crate::model::HierarchyRow {
                    name: item.name,
                    detail: item.detail.unwrap_or_default(),
                    path: item.path.into_path_buf(),
                    line: item.selection_range.start.line,
                    col: item.selection_range.start.col,
                    relation,
                });
            }
        }
        let count = rows.len();
        state.hierarchy.set(rows);
        state.show_bottom.set(true);
        state.bottom_tool.set(Some(crate::model::BottomTool::Hierarchy));
        state.status.set(format!("{count} related types"));
    });
}

/// Find Usages (Alt+F7): clangd's references for the symbol at the caret.
pub fn find_usages(state: State) {
    let Some(session) = state.lsp.get() else {
        return;
    };
    let (line, col) = state.cursor.get();
    let pos = Pos::new(line.saturating_sub(1), col.saturating_sub(1));
    let manager = session.manager;
    state.status.set("Finding usages…".to_owned());
    spawn(async move {
        match manager.references(DOC, pos, false, true, CancellationToken::new()).await {
            Ok(references) => {
                let mut rows = Vec::new();
                for location in references.locations {
                    let path = location.path.into_path_buf();
                    let line = location.range.start.line;
                    let preview = std::fs::read_to_string(&path)
                        .ok()
                        .and_then(|text| text.lines().nth(line as usize).map(|text| text.trim().to_owned()))
                        .unwrap_or_default();
                    rows.push(crate::model::UsageRow { path, line, col: location.range.start.col, preview });
                }
                let count = rows.len();
                state.usages.set(rows);
                state.show_bottom.set(true);
                state.bottom_tool.set(Some(crate::model::BottomTool::Usages));
                state.status.set(format!("{count} usages"));
            }
            Err(error) => {
                state.status.set(format!("Find Usages failed: {error}"));
            }
        }
    });
}

/// Opens hierarchy row `index` at its declaration.
pub fn open_hierarchy(state: State, index: usize) {
    let rows = state.hierarchy.get();
    let Some(row) = rows.get(index) else {
        return;
    };
    let path = row.path.clone();
    let line = row.line;
    let col = row.col;
    state.open_path(path);
    set_timeout(Duration::from_millis(120), move || {
        if let Some(api) = state.editor.api::<EditorApi>() {
            api.set_selections(vec![EditorSelection::caret(EditorPosition::new(line, col))]);
            api.scroll_to_line(line);
        }
    });
}

/// Opens usage `index`: the file, at its line and column.
pub fn open_usage(state: State, index: usize) {
    let rows = state.usages.get();
    let Some(row) = rows.get(index) else {
        return;
    };
    let path = row.path.clone();
    let line = row.line;
    let col = row.col;
    state.open_path(path);
    set_timeout(Duration::from_millis(120), move || {
        if let Some(api) = state.editor.api::<EditorApi>() {
            api.set_selections(vec![EditorSelection::caret(EditorPosition::new(line, col))]);
            api.scroll_to_line(line);
        }
    });
}

/// clangd's formatting of the active document, applied in memory.
pub fn format(state: State) {
    let Some(session) = state.lsp.get() else {
        return;
    };
    let Some(tab) = state.active_tab() else {
        return;
    };
    let text = tab.text;
    let manager = session.manager;
    spawn(async move {
        let options = ide_lsp::types::FormatOptions { tab_size: 4, insert_spaces: true, on_save: false };
        let edits = match manager.format(DOC, None, options).await {
            Ok(edits) => edits,
            Err(error) => {
                state.status.set(format!("Formatting unavailable: {error}"));
                return;
            }
        };
        let formatted = apply_edits(&text, &edits);
        if formatted == text {
            state.status.set("Already formatted".to_owned());
            return;
        }
        state.update_active_text(formatted.clone());
        if let Some(api) = state.editor.api::<EditorApi>() {
            api.set_value(formatted.clone());
        }
        change(state, &formatted);
        state.status.set("Formatted document".to_owned());
    });
}

fn apply_edits(text: &str, edits: &[ide_core::TextEdit]) -> String {
    let mut ordered: Vec<(usize, usize, &str)> = edits
        .iter()
        .map(|edit| (offset_of(text, edit.range.start), offset_of(text, edit.range.end), edit.new_text.as_str()))
        .collect();
    ordered.sort_by_key(|(start, _, _)| std::cmp::Reverse(*start));
    let mut out = text.to_owned();
    for (start, end, replacement) in ordered {
        if start <= end && end <= out.len() {
            out.replace_range(start..end, replacement);
        }
    }
    out
}

fn offset_of(text: &str, pos: Pos) -> usize {
    let mut line = 0;
    let mut line_start = 0;
    for (index, ch) in text.char_indices() {
        if line == pos.line {
            break;
        }
        if ch == '\n' {
            line += 1;
            line_start = index + 1;
        }
    }
    if line != pos.line {
        return text.len();
    }
    let mut col = 0;
    for (index, ch) in text[line_start..].char_indices() {
        if col >= pos.col {
            return line_start + index;
        }
        col += ch.len_utf16() as u32;
    }
    text.len()
}

/// Diagnostics cached for every path, so switching tabs is immediate.
pub type DiagnosticsCache = HashMap<PathBuf, Vec<Diagnostic>>;
