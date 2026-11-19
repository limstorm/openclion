//! Persisted workbench settings: layout sizes, editor font and recent
//! projects, kept in nsh storage (the platform's local store).

use nsh_core::storage::Storage;
use serde::{Deserialize, Serialize};

use crate::model::State;

const KEY: &str = "openclion.settings.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct Persisted {
    left_width: f32,
    right_width: f32,
    bottom_height: f32,
    editor_font: f32,
    recent: Vec<String>,
    show_left: bool,
    show_right: bool,
    show_bottom: bool,
    run_target: Option<String>,
}

impl Default for Persisted {
    fn default() -> Self {
        Self {
            left_width: 234.0,
            right_width: 294.0,
            bottom_height: 220.0,
            editor_font: 13.0,
            recent: Vec::new(),
            show_left: true,
            show_right: true,
            show_bottom: false,
            run_target: None,
        }
    }
}

/// Applies the stored settings to `state` (defaults when nothing is stored).
pub fn load(state: State) {
    let Some(json) = Storage::get_item(KEY) else {
        return;
    };
    let Ok(settings) = serde_json::from_str::<Persisted>(&json) else {
        return;
    };
    state.left_width.set(settings.left_width.clamp(180.0, 480.0));
    state.right_width.set(settings.right_width.clamp(220.0, 520.0));
    state.bottom_height.set(settings.bottom_height.clamp(120.0, 600.0));
    state.editor_font.set(settings.editor_font.clamp(10.0, 24.0));
    state.recent.set(settings.recent);
    state.show_left.set(settings.show_left);
    state.show_right.set(settings.show_right);
    state.show_bottom.set(settings.show_bottom);
    state.run_target.set(settings.run_target);
}

/// Writes the workbench settings back to nsh storage.
pub fn save(state: State) {
    let settings = Persisted {
        left_width: state.left_width.get(),
        right_width: state.right_width.get(),
        bottom_height: state.bottom_height.get(),
        editor_font: state.editor_font.get(),
        recent: state.recent.get(),
        show_left: state.show_left.get(),
        show_right: state.show_right.get(),
        show_bottom: state.show_bottom.get(),
        run_target: state.run_target.get(),
    };
    let Ok(json) = serde_json::to_string(&settings) else {
        return;
    };
    let _ = Storage::set_item(KEY, json);
}

pub fn set_editor_font(state: State, points: f32) {
    state.editor_font.set(points.clamp(10.0, 24.0));
    save(state);
}

pub fn restore_defaults(state: State) {
    let defaults = Persisted::default();
    state.left_width.set(defaults.left_width);
    state.right_width.set(defaults.right_width);
    state.bottom_height.set(defaults.bottom_height);
    state.editor_font.set(defaults.editor_font);
    state.show_left.set(defaults.show_left);
    state.show_right.set(defaults.show_right);
    state.show_bottom.set(defaults.show_bottom);
    save(state);
}
