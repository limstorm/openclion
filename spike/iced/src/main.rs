//! iced spike: the shape of the IDE shell, on the software renderer.
//!
//! Three panes (list / editor / inspector) inside `pane_grid`, an editable document, a
//! scrollable list of 5000 rows, and a status bar whose numbers come from measuring the
//! `view` pass itself — the point is to find out what a frame costs on this machine
//! before any real UI work starts.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use iced::widget::{Space, button, column, container, pane_grid, row, scrollable, text, text_editor, text_input};
use iced::{Alignment, Element, Length, Task, Theme};

fn main() -> iced::Result {
    let renderer = std::env::var("ICED_BACKEND").unwrap_or_else(|_| "default".into());
    eprintln!("iced-spike: starting (ICED_BACKEND={renderer})");
    iced::application(Spike::boot, Spike::update, Spike::view)
        .title("iced spike")
        .theme(Spike::theme)
        .window_size((1440.0, 900.0))
        .antialiasing(true)
        .subscription(Spike::subscription)
        .run()
}

/// Rows in the scrollable list: enough that a naive `column` would be pathological.
const ROWS: usize = 5_000;
/// Height of one row, used to window the list: iced's `scrollable` does not virtualise by
/// itself, so the spike renders only the rows the viewport can show (what a real file tree
/// widget has to do anyway).
const ROW_HEIGHT: f32 = 16.0;
/// Frames kept for the percentile readout.
const WINDOW: usize = 180;

struct Spike {
    panes: pane_grid::State<Pane>,
    focused: Option<pane_grid::Pane>,
    editor: text_editor::Content,
    list: Vec<String>,
    filter: String,
    /// Frame clock: `view` only sees `&self`, so this is behind a `RefCell`.
    clock: std::cell::RefCell<Clock>,
    /// Scroll offset of the list (px) and the viewport height, for row windowing.
    scrolled: f32,
    viewport_height: f32,
    /// Render every row instead of windowing, to show what that costs.
    unvirtualized: bool,
    /// Draw the same 62 strings whatever the scroll offset is: separates text shaping
    /// from rasterisation (what a re-shaping cache would buy).
    frozen_content: bool,
}

#[derive(Debug, Clone)]
enum Message {
    PaneResized(pane_grid::ResizeEvent),
    PaneDragged(pane_grid::DragEvent),
    PaneFocused(pane_grid::Pane),
    EditorAction(text_editor::Action),
    FilterChanged(String),
    Scrolled(scrollable::Viewport),
    ToggleVirtualization,
    ToggleFrozenContent,
    Tick,
    Window(iced::window::Event),
}

#[derive(Clone, Copy)]
struct Pane {
    kind: Kind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    List,
    Editor,
    Inspector,
}

impl Spike {
    fn boot() -> (Self, Task<Message>) {
        // List 20% | editor | inspector 24%. `split` puts the new pane after the target and
        // `resize` positions the divider inside that node, so the anchor is the list.
        let (mut panes, list) = pane_grid::State::new(Pane { kind: Kind::List });
        let (editor, editor_split) =
            panes.split(pane_grid::Axis::Vertical, list, Pane { kind: Kind::Editor }).expect("split editor");
        panes.resize(editor_split, 0.2);
        let (_inspector, inspector_split) =
            panes.split(pane_grid::Axis::Vertical, editor, Pane { kind: Kind::Inspector }).expect("split inspector");
        panes.resize(inspector_split, 0.7);
        let main = editor;

        let list = (0..ROWS).map(|i| format!("src/feature_{:04}/module_{}.rs", i / 20, i)).collect();

        let state = Self {
            panes,
            focused: Some(main),
            editor: text_editor::Content::with_text(
                "// iced spike: type here, including 中文（IBus）\nfn main() {\n    println!(\"hello\");\n}\n",
            ),
            list,
            filter: String::new(),
            clock: std::cell::RefCell::new(Clock {
                frames: VecDeque::with_capacity(WINDOW),
                last: None,
                first: None,
                boot: Instant::now(),
            }),
            scrolled: 0.0,
            viewport_height: 600.0,
            unvirtualized: false,
            // SPIKE_FROZEN=1 starts with fixed row content, for a scripted A/B.
            frozen_content: std::env::var_os("SPIKE_FROZEN").is_some(),
        };
        (state, Task::none())
    }

    /// `(p50, p95, max, samples)` of the last [`WINDOW`] frames, in milliseconds.
    fn stats(&self) -> (f64, f64, f64, f64) {
        let clock = self.clock.borrow();
        if clock.frames.is_empty() {
            return (0.0, 0.0, 0.0, 0.0);
        }
        let mut sorted: Vec<f64> = clock.frames.iter().map(|d| d.as_secs_f64() * 1000.0).collect();
        sorted.sort_by(f64::total_cmp);
        let at = |q: f64| sorted[((sorted.len() - 1) as f64 * q) as usize];
        (at(0.5), at(0.95), *sorted.last().unwrap(), sorted.len() as f64)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        if std::env::var_os("SPIKE_TRACE").is_some() {
            eprintln!("msg: {message:?}");
        }
        match message {
            Message::PaneResized(pane_grid::ResizeEvent { split, ratio }) => {
                self.panes.resize(split, ratio);
            }
            Message::PaneDragged(pane_grid::DragEvent::Dropped { pane, target }) => {
                self.panes.drop(pane, target);
            }
            Message::PaneDragged(_) => {}
            Message::PaneFocused(pane) => self.focused = Some(pane),
            Message::EditorAction(action) => {
                let is_edit = action.is_edit();
                self.editor.perform(action);
                if is_edit {
                    // Nothing else: an IDE would push the change to the document model here.
                }
            }
            Message::FilterChanged(value) => self.filter = value,
            Message::Scrolled(viewport) => {
                self.scrolled = viewport.absolute_offset().y;
                self.viewport_height = viewport.bounds().height;
            }
            Message::ToggleVirtualization => self.unvirtualized = !self.unvirtualized,
            Message::ToggleFrozenContent => self.frozen_content = !self.frozen_content,
            Message::Window(event) => {
                // IME does not travel as a window event in 0.14: `text_editor` consumes
                // `Event::Ime` at the widget level, which is why showing the preedit string
                // needs our own widget (M2).
                if std::env::var_os("SPIKE_TRACE").is_some() {
                    eprintln!("window: {event:?}");
                }
            }
            Message::Tick => {
                let (p50, p95, max, n) = self.stats();
                eprintln!(
                    "frames n={n:>3} p50={p50:>6.2}ms p95={p95:>6.2}ms max={max:>6.2}ms  scroll_y={:.0} virtualized={}",
                    self.scrolled, !self.unvirtualized
                );
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        // Measure the interval between `view` calls: one per rendered frame.
        let now = Instant::now();
        {
            let mut clock = self.clock.borrow_mut();
            if let Some(previous) = clock.last {
                let delta = now - previous;
                // iced redraws on demand, so a quiet window produces one frame every few
                // seconds; those gaps are idle, not frame time, and would swamp the p95.
                if delta <= Duration::from_millis(250) {
                    if clock.frames.len() == WINDOW {
                        clock.frames.pop_front();
                    }
                    clock.frames.push_back(delta);
                }
            }
            clock.last = Some(now);
            if clock.first.is_none() {
                clock.first = Some(now - clock.boot);
            }
        }

        let panes = pane_grid::PaneGrid::new(&self.panes, |_pane, state, _is_maximized| {
            pane_grid::Content::new(match state.kind {
                Kind::List => self.list_pane(),
                Kind::Editor => self.editor_pane(),
                Kind::Inspector => self.inspector_pane(),
            })
            .title_bar(pane_grid::TitleBar::new(match state.kind {
                Kind::List => "Project",
                Kind::Editor => "Editor",
                Kind::Inspector => "Inspector",
            }))
        })
        .spacing(2)
        .on_resize(4, Message::PaneResized)
        .on_drag(Message::PaneDragged)
        .on_click(Message::PaneFocused);

        column![panes, container(self.status_bar()).height(Length::Fixed(24.0)).width(Length::Fill).padding([0, 8]),]
            .into()
    }

    fn list_pane(&self) -> Element<'_, Message> {
        let filter = text_input("Filter files…", &self.filter).on_input(Message::FilterChanged).padding(4).size(13);

        let rows: Vec<&String> =
            self.list.iter().filter(|name| self.filter.is_empty() || name.contains(&self.filter)).collect();
        let total = rows.len();
        // Only the rows the viewport shows, unless the toggle asks for the naive version.
        let (first, last) = if self.frozen_content {
            // Same 62 strings every frame, moved to wherever the viewport is: identical
            // text at the same place, so the only difference from the live mode is that
            // shaping can be reused.
            (0, self.list.len().min(62))
        } else if self.unvirtualized {
            (0, total)
        } else {
            let first = ((self.scrolled / ROW_HEIGHT).floor() as usize).saturating_sub(8);
            let visible = (self.viewport_height / ROW_HEIGHT).ceil() as usize + 16;
            (first.min(total), (first + visible).min(total))
        };
        let visible_rows = last - first;
        let (above, below) = if self.frozen_content {
            let drawn = (last - first) as f32 * ROW_HEIGHT;
            let above = self.scrolled.clamp(0.0, (total as f32 * ROW_HEIGHT - drawn).max(0.0));
            (above, (total as f32 * ROW_HEIGHT - drawn - above).max(0.0))
        } else {
            (first as f32 * ROW_HEIGHT, (total.saturating_sub(last)) as f32 * ROW_HEIGHT)
        };

        let mut list_column = column![].spacing(0);
        if above > 0.0 {
            list_column = list_column.push(Space::new().height(Length::Fixed(above)));
        }
        for name in &rows[first..last] {
            list_column = list_column.push(text(name.as_str()).size(12).height(ROW_HEIGHT));
        }
        if below > 0.0 {
            list_column = list_column.push(Space::new().height(Length::Fixed(below)));
        }
        let list = scrollable(list_column).on_scroll(Message::Scrolled).height(Length::Fill);

        column![filter, list, text(format!("rows drawn {visible_rows}/{total}")).size(11),].spacing(4).padding(6).into()
    }

    fn editor_pane(&self) -> Element<'_, Message> {
        text_editor(&self.editor).on_action(Message::EditorAction).padding(8).size(14).height(Length::Fill).into()
    }

    fn inspector_pane(&self) -> Element<'_, Message> {
        let (p50, p95, max, n) = self.stats();
        column![
            text("Frame statistics").size(14),
            text(format!("frames: {n}")).size(12),
            text(format!("p50: {p50:.2} ms")).size(12),
            text(format!("p95: {p95:.2} ms")).size(12),
            text(format!("max: {max:.2} ms")).size(12),
            Space::new().height(Length::Fixed(8.0)),
            text(format!("first frame: {:.0} ms", self.clock.borrow().first.map_or(0.0, |d| d.as_secs_f64() * 1000.0)))
                .size(12),
            text(format!("scroll: {:.0} px", self.scrolled)).size(12),
            Space::new().height(Length::Fixed(8.0)),
            button(if self.unvirtualized { "Virtualize list" } else { "Render all rows" })
                .on_press(Message::ToggleVirtualization),
            button(if self.frozen_content { "Live content" } else { "Frozen content" })
                .on_press(Message::ToggleFrozenContent),
            button("Print statistics").on_press(Message::Tick),
        ]
        .spacing(2)
        .padding(6)
        .into()
    }

    fn status_bar(&self) -> Element<'_, Message> {
        let (p50, _, _, _) = self.stats();
        row![
            text("iced ✓").size(12),
            text("|").size(12),
            text(format!("frame p50 {p50:.1} ms")).size(12),
            Space::new().width(Length::Fill),
            text(format!("rows {ROWS}")).size(12),
            text("|").size(12),
            text("UTF-8").size(12),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
    }

    fn subscription(&self) -> iced::Subscription<Message> {
        iced::window::events().map(|(_id, event)| Message::Window(event))
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }
}

/// Keep the type checker honest about the widget lifetimes the real app will hit.
#[allow(dead_code)]
fn _assert_element<'a>(element: Element<'a, Message>) -> Element<'a, Message> {
    element
}

fn _unused(_: Duration) {}

/// Frame timings, kept out of `Spike`'s `&self` `view` by interior mutability.
struct Clock {
    frames: VecDeque<Duration>,
    last: Option<Instant>,
    first: Option<Duration>,
    boot: Instant,
}
