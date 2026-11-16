//! A rope-backed text editor widget, written from scratch.
//!
//! Deliberately minimal: one font, no syntax highlighting, no wrapping. What it does prove
//! is the interaction and rendering the migration plan depends on — a caret, a selection,
//! typed input, and the IME preedit string drawn on the spot at the caret.

use std::cell::{Ref, RefCell, RefMut};
use std::ops::Range;
use std::rc::Rc;

use iced::advanced::input_method::{self, InputMethod, Preedit, Purpose};
use iced::advanced::renderer::Renderer as _;
use iced::advanced::text::{self, Renderer as _};
use iced::advanced::widget::operation::Focusable as _;
use iced::advanced::widget::{Id, Tree, operation, tree::State as TreeState};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, renderer};
use iced::keyboard::key;
use iced::{Color, Element, Event, Font, Length, Pixels, Point, Rectangle, Size};

use ropey::Rope;

/// The editor state, shared with the widget (a real implementation would own it in the app
/// and drive it with messages; the prototype mutates it directly through a `RefCell`).
#[derive(Clone)]
pub struct Shared(Rc<RefCell<EditorState>>);

impl Shared {
    pub fn new(state: EditorState) -> Self {
        Self(Rc::new(RefCell::new(state)))
    }

    pub fn borrow(&self) -> Ref<'_, EditorState> {
        self.0.borrow()
    }

    pub fn borrow_mut(&self) -> RefMut<'_, EditorState> {
        self.0.borrow_mut()
    }
}

pub struct EditorState {
    rope: Rope,
    /// Caret position in `char` indices (not bytes, not graphemes: enough for the prototype).
    caret: usize,
    /// Where the selection started; equal to `caret` when nothing is selected.
    anchor: usize,
    /// First visible line.
    scroll: usize,
    pub line_height: f32,
    pub font_size: f32,
    focused: bool,
    /// `(content, selection within the content)` for the IME composition.
    preedit: Option<(String, Option<Range<usize>>)>,
    dragging: bool,
}

impl EditorState {
    pub fn new(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
            caret: 0,
            anchor: 0,
            scroll: 0,
            line_height: 20.0,
            font_size: 14.0,
            focused: false,
            preedit: None,
            dragging: false,
        }
    }

    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    pub fn line(&self) -> usize {
        self.rope.char_to_line(self.caret.min(self.rope.len_chars()))
    }

    pub fn column(&self) -> usize {
        let line = self.line();
        if line >= self.rope.len_lines() {
            return 0;
        }
        let start = self.rope.line_to_char(line);
        self.caret.min(self.rope.len_chars()).saturating_sub(start)
    }

    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    /// `(start, end)` in char indices when there is a selection.
    pub fn selection(&self) -> Option<(usize, usize)> {
        (self.caret != self.anchor).then(|| (self.caret.min(self.anchor), self.caret.max(self.anchor)))
    }

    fn line_text(&self, index: usize) -> String {
        if index >= self.rope.len_lines() {
            return String::new();
        }
        let slice = self.rope.line(index);
        slice.to_string().trim_end_matches('\n').to_owned()
    }

    /// Width of one character. A real editor asks the text renderer
    /// (`Paragraph::grapheme_position`); the prototype approximates a monospace cell and
    /// treats CJK as double width, which is close enough to place a caret.
    fn char_width(&self, c: char) -> f32 {
        let narrow = (c as u32) < 0x2e80;
        self.font_size * if narrow { 0.6 } else { 1.0 }
    }

    fn column_x(&self, line: usize, column: usize) -> f32 {
        let text = self.line_text(line);
        text.chars().take(column).map(|c| self.char_width(c)).sum()
    }

    fn column_at_x(&self, line: usize, x: f32) -> usize {
        let text = self.line_text(line);
        let mut width = 0.0;
        for (i, c) in text.chars().enumerate() {
            let w = self.char_width(c);
            if x < width + w / 2.0 {
                return i;
            }
            width += w;
        }
        text.chars().count()
    }

    /// Caret position for a point inside the text area.
    fn position_at(&self, point: Point) -> usize {
        let line = self.scroll + (point.y / self.line_height).max(0.0) as usize;
        let line = line.min(self.line_count().saturating_sub(1));
        let column = self.column_at_x(line, point.x.max(0.0));
        (self.rope.line_to_char(line) + column).min(self.rope.len_chars())
    }

    fn insert(&mut self, text: &str) {
        if let Some((start, end)) = self.selection() {
            self.rope.remove(start..end);
            self.caret = start;
        }
        let at = self.caret.min(self.rope.len_chars());
        self.rope.insert(at, text);
        self.caret = at + text.chars().count();
        self.anchor = self.caret;
        self.preedit = None;
    }

    fn backspace(&mut self) {
        if let Some((start, end)) = self.selection() {
            self.rope.remove(start..end);
            self.caret = start;
            self.anchor = start;
            return;
        }
        if self.caret > 0 {
            self.rope.remove(self.caret - 1..self.caret);
            self.caret -= 1;
            self.anchor = self.caret;
        }
    }

    fn delete_forward(&mut self) {
        if let Some((start, end)) = self.selection() {
            self.rope.remove(start..end);
            self.caret = start;
            self.anchor = start;
            return;
        }
        if self.caret < self.rope.len_chars() {
            self.rope.remove(self.caret..self.caret + 1);
        }
    }

    fn move_horizontal(&mut self, delta: isize, select: bool) {
        let next = (self.caret as isize + delta).clamp(0, self.rope.len_chars() as isize) as usize;
        self.caret = next;
        if !select {
            self.anchor = next;
        }
    }

    fn move_vertical(&mut self, delta: isize, select: bool) {
        let line = self.line() as isize + delta;
        let line = line.clamp(0, self.line_count().saturating_sub(1) as isize) as usize;
        let column = self.column();
        let start = self.rope.line_to_char(line);
        let len = self.line_text(line).chars().count();
        self.caret = start + column.min(len);
        if !select {
            self.anchor = self.caret;
        }
        self.ensure_visible();
    }

    fn ensure_visible(&mut self) {
        let line = self.line();
        if line < self.scroll {
            self.scroll = line;
        }
    }
}

pub struct Editor {
    state: Shared,
    /// iced routes `Event::InputMethod` through the focus machinery, which needs an `Id`.
    /// It must be the *same* id every frame — a fresh `Id::unique()` per view loses focus
    /// (and with it every IME event) on the next redraw.
    id: Id,
}

impl Editor {
    pub fn new(state: Shared, id: Id) -> Self {
        Self { state, id }
    }
}

/// The focus state iced's `operation::focusable` hands to us.
#[derive(Debug, Default)]
pub struct EditorFocus {
    focused: bool,
}

impl operation::Focusable for EditorFocus {
    fn is_focused(&self) -> bool {
        self.focused
    }

    fn focus(&mut self) {
        self.focused = true;
    }

    fn unfocus(&mut self) {
        self.focused = false;
    }
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for Editor {
    fn size(&self) -> Size<Length> {
        Size { width: Length::Fill, height: Length::Fill }
    }

    fn layout(&mut self, _tree: &mut Tree, _renderer: &iced::Renderer, limits: &layout::Limits) -> layout::Node {
        layout::atomic(limits, Length::Fill, Length::Fill)
    }

    fn state(&self) -> TreeState {
        TreeState::new(EditorFocus::default())
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        _renderer: &iced::Renderer,
        operation: &mut dyn operation::Operation,
    ) {
        operation.focusable(Some(&self.id), layout.bounds(), tree.state.downcast_mut::<EditorFocus>());
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &iced::Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let mut changed = false;
        let mut state = self.state.borrow_mut();

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(point) = cursor.position_in(bounds) {
                    state.focused = true;
                    tree.state.downcast_mut::<EditorFocus>().focus();
                    state.caret = state.position_at(point);
                    state.anchor = state.caret;
                    state.dragging = true;
                    changed = true;
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.dragging = false;
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if state.dragging
                    && let Some(point) = cursor.position_in(bounds)
                {
                    state.caret = state.position_at(point);
                    changed = true;
                }
            }
            Event::Keyboard(iced::keyboard::Event::KeyPressed { key, text, modifiers, .. }) if state.focused => {
                let select = modifiers.shift();
                match key.as_ref() {
                    key::Key::Named(key::Named::Backspace) => state.backspace(),
                    key::Key::Named(key::Named::Delete) => state.delete_forward(),
                    key::Key::Named(key::Named::Enter) => state.insert("\n"),
                    key::Key::Named(key::Named::Tab) => state.insert("    "),
                    key::Key::Named(key::Named::ArrowLeft) => state.move_horizontal(-1, select),
                    key::Key::Named(key::Named::ArrowRight) => state.move_horizontal(1, select),
                    key::Key::Named(key::Named::ArrowUp) => state.move_vertical(-1, select),
                    key::Key::Named(key::Named::ArrowDown) => state.move_vertical(1, select),
                    key::Key::Named(key::Named::Home) => {
                        let start = state.rope.line_to_char(state.line());
                        state.caret = start;
                        if !select {
                            state.anchor = start;
                        }
                    }
                    key::Key::Named(key::Named::End) => {
                        let len = state.line_text(state.line()).chars().count();
                        let start = state.rope.line_to_char(state.line());
                        state.caret = start + len;
                        if !select {
                            state.anchor = state.caret;
                        }
                    }
                    _ => {
                        if let Some(text) = text
                            && !modifiers.control()
                            && !modifiers.command()
                            && !modifiers.alt()
                        {
                            state.insert(text);
                        }
                    }
                }
                state.preedit = None;
                changed = true;
            }
            Event::InputMethod(ime) => match ime {
                input_method::Event::Opened => {
                    state.focused = true;
                }
                input_method::Event::Preedit(content, selection) => {
                    eprintln!("ime: preedit {content:?} {selection:?}");
                    state.preedit = (!content.is_empty()).then(|| (content.clone(), selection.clone()));
                    changed = true;
                }
                input_method::Event::Commit(content) => {
                    eprintln!("ime: commit {content:?}");
                    state.insert(content);
                    changed = true;
                }
                input_method::Event::Closed => {
                    state.preedit = None;
                }
            },
            other => {
                if std::env::var_os("EDITOR_TRACE").is_some() {
                    eprintln!("event: {other:?}");
                }
            }
        }

        // Request the input method on every event, not only the ones that changed
        // something: `text_editor` does the same, and XIM appears to attach lazily.
        {
            if std::env::var_os("EDITOR_TRACE").is_some() {
                eprintln!("request_input_method: enabled (focused={})", state.focused);
            }
            shell.request_redraw();
            // Keep the candidate window at the caret, and let the runtime show the
            // composition if we ever decide not to draw it inline.
            let caret = Rectangle::new(
                Point::new(
                    state.column_x(state.line(), state.column()),
                    (state.line() - state.scroll) as f32 * state.line_height,
                ),
                Size::new(2.0, state.line_height),
            );
            let preedit = state.preedit.clone().map(|(content, selection)| Preedit {
                content,
                selection,
                text_size: Some(Pixels::from(state.font_size)),
            });
            shell.request_input_method(&InputMethod::Enabled { cursor: caret, purpose: Purpose::Normal, preedit });
        }
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut iced::Renderer,
        _theme: &iced::Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        if !bounds.intersects(viewport) {
            return;
        }

        let state = self.state.borrow();
        let font = Font::MONOSPACE;
        let size = Pixels::from(state.font_size);
        let line_height = state.line_height;

        renderer.fill_quad(renderer::Quad { bounds, ..renderer::Quad::default() }, Color::from_rgb(0.11, 0.12, 0.14));

        let first = state.scroll;
        let visible = (bounds.height / line_height).ceil() as usize + 1;
        let last = (first + visible).min(state.line_count());

        if let Some((start, end)) = state.selection() {
            for line in first..last {
                let line_start = state.rope.line_to_char(line);
                let length = state.line_text(line).chars().count();
                let line_end = line_start + length;
                let (from, to) = (start.max(line_start), end.min(line_end + 1));
                if from >= to {
                    continue;
                }
                let x0 = state.column_x(line, from - line_start);
                let x1 = state.column_x(line, (to - line_start).min(length));
                let y = bounds.y + (line - first) as f32 * line_height;
                let width = if to - line_start > length {
                    bounds.x + bounds.width - (bounds.x + x0)
                } else {
                    (x1 - x0).max(2.0)
                };
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: Rectangle::new(Point::new(bounds.x + x0, y), Size::new(width, line_height)),
                        ..renderer::Quad::default()
                    },
                    Color::from_rgba(0.30, 0.45, 0.75, 0.35),
                );
            }
        }

        for line in first..last {
            let text = state.line_text(line);
            if text.is_empty() {
                continue;
            }
            let y = bounds.y + (line - first) as f32 * line_height;
            renderer.fill_text(
                text::Text {
                    content: text,
                    bounds: Size::new(bounds.width, line_height),
                    size,
                    line_height: text::LineHeight::Absolute(Pixels::from(line_height)),
                    font,
                    align_x: text::Alignment::Left,
                    align_y: iced::alignment::Vertical::Top,
                    shaping: text::Shaping::Advanced,
                    wrapping: text::Wrapping::None,
                },
                Point::new(bounds.x, y),
                Color::from_rgb(0.84, 0.86, 0.90),
                bounds,
            );
        }

        // The IME composition, drawn on the spot: underlined, after the caret.
        if let Some((content, _)) = &state.preedit
            && state.focused
        {
            let line = state.line();
            let x = bounds.x + state.column_x(line, state.column());
            let y = bounds.y + (line - state.scroll) as f32 * line_height;
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle::new(
                        Point::new(x, y + line_height - 2.0),
                        Size::new(state.column_x(0, 0) + content_width(&state, content), 1.0),
                    ),
                    ..renderer::Quad::default()
                },
                Color::from_rgb(0.95, 0.70, 0.30),
            );
            renderer.fill_text(
                text::Text {
                    content: content.clone(),
                    bounds: Size::new(bounds.width, line_height),
                    size,
                    line_height: text::LineHeight::Absolute(Pixels::from(line_height)),
                    font,
                    align_x: text::Alignment::Left,
                    align_y: iced::alignment::Vertical::Top,
                    shaping: text::Shaping::Advanced,
                    wrapping: text::Wrapping::None,
                },
                Point::new(x, y),
                Color::from_rgb(0.98, 0.78, 0.35),
                bounds,
            );
        }

        if state.focused {
            let line = state.line();
            let x = bounds.x + state.column_x(line, state.column());
            let y = bounds.y + (line - state.scroll) as f32 * line_height;
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle::new(Point::new(x, y), Size::new(1.5, line_height)),
                    ..renderer::Quad::default()
                },
                Color::from_rgb(0.98, 0.98, 0.98),
            );
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) { mouse::Interaction::Text } else { mouse::Interaction::None }
    }
}

fn content_width(state: &EditorState, content: &str) -> f32 {
    content.chars().map(|c| state.char_width(c)).sum()
}

impl<'a, Message> From<Editor> for Element<'a, Message>
where
    Message: 'a,
{
    fn from(editor: Editor) -> Self {
        Element::new(editor)
    }
}
