//! clangd through `ide-lsp`: documents are opened, changed and closed on the
//! manager; diagnostics arrive on its event sink and drive the native
//! editor's squiggles and the status bar.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use ide_core::{
    Diagnostic, DocId, EventSink, IdeEvent, LanguageId, Pos, Range, Severity, SharedSink, event::ServerStatus,
};
use ide_language::LanguageRegistry;
use ide_lsp::LspManager;
use ide_text::AppliedChange;
use nsh_base::editor::{EditorDiagnostic, EditorDiagnosticSeverity, EditorDiagnosticTag, EditorPosition, EditorRange};
use nsh_core::prelude::*;

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

/// Diagnostics cached for every path, so switching tabs is immediate.
pub type DiagnosticsCache = HashMap<PathBuf, Vec<Diagnostic>>;
