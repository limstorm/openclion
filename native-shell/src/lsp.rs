//! clangd through `ide-lsp`: documents are opened, changed and closed on the
//! manager; diagnostics arrive on its event sink and drive the native
//! editor's squiggles and the status bar.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use ide_core::{
    CancellationToken, Diagnostic, DocId, EventSink, IdeEvent, LanguageId, Pos, Range, Severity, SharedSink,
    event::ServerStatus,
};
use ide_language::LanguageRegistry;
use ide_lsp::LspManager;
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

/// Diagnostics cached for every path, so switching tabs is immediate.
pub type DiagnosticsCache = HashMap<PathBuf, Vec<Diagnostic>>;
