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
use iced::advanced::text::{self, Paragraph as _, Renderer as _};
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

/// Where the line numbers end: the digits `87` sit at x=471..485 with the editor at 453, so
/// their right edge is 32px in (`.cm-lineNumbers .cm-gutterElement` pads `0 8px 0 10px`).
const GUTTER_NUMBERS_RIGHT: f32 = 32.0;
/// The 1px `--editor-gutter-border` line: measured at x=543 for an editor at 453, 90px in.
const GUTTER_BORDER: f32 = 90.0;
/// Where the code column starts: the first glyph of `func reserve…` is at 550, 97px in.
const TEXT_ORIGIN: f32 = 97.0;

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
    /// Draw the line-number gutter (the web editor always has one).
    pub gutter: bool,
    /// Rows the viewport shows, refreshed on every draw.
    visible_rows: usize,
    /// Highlighted ranges, sorted by start; empty when the file is not highlighted.
    highlights: Vec<Highlight>,
    /// Recomputes the highlights after an edit. The shell installs this when it opens a file,
    /// so the widget does not need to know which language it is editing.
    highlighter: Option<Highlighter>,
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
            gutter: true,
            visible_rows: 0,
            highlights: Vec::new(),
            highlighter: None,
        }
    }

    /// The buffer as a string (the shell reads it for indentation and outline panels).
    pub fn text(&self) -> String {
        self.rope.to_string()
    }

    /// Installs the re-highlighting hook; called after every edit.
    pub fn set_highlighter(&mut self, highlighter: Highlighter) {
        self.highlights = highlighter(&self.rope.to_string());
        self.highlighter = Some(highlighter);
    }

    fn rehighlight(&mut self) {
        if let Some(highlighter) = &self.highlighter {
            let spans = highlighter(&self.rope.to_string());
            self.highlights = spans;
        }
    }

    /// Replaces the highlights; the shell recomputes them when the document changes.
    pub fn set_highlights(&mut self, mut highlights: Vec<Highlight>) {
        highlights.sort_by_key(|highlight| highlight.start);
        self.highlights = highlights;
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

    /// Caret position for a point inside the text area.
    fn position_at(&self, point: Point) -> usize {
        let line = self.scroll + (point.y / self.line_height).max(0.0) as usize;
        let line = line.min(self.line_count().saturating_sub(1));
        let paragraph = line_paragraph(&self.line_text(line), self.font_size, self.line_height);
        let column = column_at_x(
            &paragraph,
            self.line_text(line).chars().count(),
            (point.x - if self.gutter { TEXT_ORIGIN } else { 0.0 }).max(0.0),
        );
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
        self.rehighlight();
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
        self.rehighlight();
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
        self.rehighlight();
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
        let rows = self.visible_rows.max(1);
        if line < self.scroll {
            self.scroll = line;
        } else if line >= self.scroll + rows {
            self.scroll = line + 1 - rows;
        }
    }
}

/// The syntax classes the editor paints. The shell maps its highlighter's kinds onto these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxClass {
    Keyword,
    String,
    Number,
    Comment,
    Function,
    Type,
    Constant,
    Punctuation,
    Variable,
    Other,
}

/// One highlighted range, in `char` offsets into the buffer.
#[derive(Debug, Clone, Copy)]
pub struct Highlight {
    pub start: usize,
    pub end: usize,
    pub class: SyntaxClass,
}

/// Computes the highlight spans for a buffer (installed by the shell, so the widget stays
/// independent of any language).
pub type Highlighter = Box<dyn Fn(&str) -> Vec<Highlight>>;

/// Syntax colours, mirroring the `--syn-*` variables of the web theme.
#[derive(Debug, Clone, Copy)]
pub struct SyntaxColors {
    pub keyword: Color,
    pub string: Color,
    pub number: Color,
    pub comment: Color,
    pub function: Color,
    pub ty: Color,
    pub constant: Color,
    pub punctuation: Color,
    pub variable: Color,
    pub other: Color,
}

impl SyntaxColors {
    pub fn color(&self, class: SyntaxClass) -> Color {
        match class {
            SyntaxClass::Keyword => self.keyword,
            SyntaxClass::String => self.string,
            SyntaxClass::Number => self.number,
            SyntaxClass::Comment => self.comment,
            SyntaxClass::Function => self.function,
            SyntaxClass::Type => self.ty,
            SyntaxClass::Constant => self.constant,
            SyntaxClass::Punctuation => self.punctuation,
            SyntaxClass::Variable => self.variable,
            SyntaxClass::Other => self.other,
        }
    }
}

/// Colours the widget draws with; the shell passes its tokens in so the editor follows the
/// workbench theme instead of hardcoding one.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub background: Color,
    pub text: Color,
    pub selection: Color,
    pub caret: Color,
    pub preedit: Color,
    pub preedit_underline: Color,
    /// `--fg-line-number`
    pub gutter_text: Color,
    /// `--fg-line-number-current`
    pub gutter_text_current: Color,
    /// `--editor-gutter-border`
    pub gutter_border: Color,
    /// `--syn-*`
    pub syntax: SyntaxColors,
    /// `--scrollbar-thumb`
    pub scrollbar: Color,
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            background: Color::from_rgb(0.11, 0.12, 0.14),
            text: Color::from_rgb(0.84, 0.86, 0.90),
            selection: Color::from_rgba(0.30, 0.45, 0.75, 0.35),
            caret: Color::from_rgb(0.98, 0.98, 0.98),
            preedit: Color::from_rgb(0.98, 0.78, 0.35),
            preedit_underline: Color::from_rgb(0.95, 0.70, 0.30),
            gutter_text: Color::from_rgb(0.294, 0.314, 0.349),
            gutter_text_current: Color::from_rgb(0.631, 0.639, 0.671),
            scrollbar: Color::from_rgba(1.0, 1.0, 1.0, 0.2),
            gutter_border: Color::from_rgb(0.149, 0.157, 0.173),
            syntax: SyntaxColors {
                keyword: Color::from_rgb(0.812, 0.557, 0.427),
                string: Color::from_rgb(0.416, 0.671, 0.451),
                number: Color::from_rgb(0.165, 0.675, 0.722),
                comment: Color::from_rgb(0.478, 0.494, 0.522),
                function: Color::from_rgb(0.337, 0.659, 0.961),
                ty: Color::from_rgb(0.737, 0.745, 0.769),
                constant: Color::from_rgb(0.780, 0.490, 0.733),
                punctuation: Color::from_rgb(0.737, 0.745, 0.769),
                variable: Color::from_rgb(0.737, 0.745, 0.769),
                other: Color::from_rgb(0.737, 0.745, 0.769),
            },
        }
    }
}

pub struct Editor {
    state: Shared,
    palette: Palette,
    font: Font,
    /// iced routes `Event::InputMethod` through the focus machinery, which needs an `Id`.
    /// It must be the *same* id every frame — a fresh `Id::unique()` per view loses focus
    /// (and with it every IME event) on the next redraw.
    id: Id,
}

impl Editor {
    pub fn new(state: Shared, id: Id) -> Self {
        Self { state, id, palette: Palette::default(), font: Font::MONOSPACE }
    }

    /// `--font-mono`: the shell passes the family the web editor uses.
    pub fn font(mut self, font: Font) -> Self {
        self.font = font;
        self
    }

    pub fn palette(mut self, palette: Palette) -> Self {
        self.palette = palette;
        self
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
        let mut state = self.state.borrow_mut();
        state.visible_rows = (bounds.height / state.line_height).floor().max(1.0) as usize;

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(point) = cursor.position_in(bounds) {
                    state.focused = true;
                    tree.state.downcast_mut::<EditorFocus>().focus();
                    state.caret = state.position_at(point);
                    state.anchor = state.caret;
                    state.dragging = true;
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.dragging = false;
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) if cursor.is_over(bounds) => {
                let rows = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => *y,
                    mouse::ScrollDelta::Pixels { y, .. } => *y / state.line_height,
                };
                let visible = (bounds.height / state.line_height).ceil() as isize;
                let max = (state.line_count() as isize - visible).max(0);
                let next = (state.scroll as isize - rows.round() as isize).clamp(0, max);
                if next != state.scroll as isize {
                    state.scroll = next as usize;
                    shell.request_redraw();
                }
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if state.dragging
                    && let Some(point) = cursor.position_in(bounds)
                {
                    state.caret = state.position_at(point);
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
            }
            Event::InputMethod(ime) => match ime {
                input_method::Event::Opened => {
                    state.focused = true;
                }
                input_method::Event::Preedit(content, selection) => {
                    if std::env::var_os("EDITOR_TRACE").is_some() {
                        eprintln!("ime: preedit {content:?} {selection:?}");
                    }
                    state.preedit = (!content.is_empty()).then(|| (content.clone(), selection.clone()));
                }
                input_method::Event::Commit(content) => {
                    if std::env::var_os("EDITOR_TRACE").is_some() {
                        eprintln!("ime: commit {content:?}");
                    }
                    state.insert(content);
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
            shell.request_redraw();
            // Keep the candidate window at the caret, and let the runtime show the
            // composition if we ever decide not to draw it inline.
            let line_text = state.line_text(state.line());
            let caret_x = column_x(&line_paragraph(&line_text, state.font_size, state.line_height), state.column());
            let caret = Rectangle::new(
                Point::new(caret_x, (state.line() - state.scroll) as f32 * state.line_height),
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

        renderer.fill_quad(renderer::Quad { bounds, ..renderer::Quad::default() }, self.palette.background);

        let first = state.scroll;
        let visible = (bounds.height / line_height).ceil() as usize + 1;
        let last = (first + visible).min(state.line_count());
        let origin = if state.gutter { TEXT_ORIGIN } else { 0.0 };

        // `.cm-lineNumbers`: right-aligned digits ending `GUTTER_NUMBERS_RIGHT` in, in
        // `--fg-line-number` (the caret's line uses `--fg-line-number-current`), with the 1px
        // `--editor-gutter-border` hairline after the gutter.
        //
        // `fill_text` anchors left-aligned text reliably in this backend while `fill_paragraph`
        // does not draw at all, so the digits are padded with spaces to the widest number and
        // placed by measuring one glyph — the font is monospace, which keeps it exact.
        if state.gutter {
            let caret_line = state.line();
            let widest = state.line_count().max(1).to_string().len();
            let digit = line_paragraph("0", state.font_size, line_height).min_bounds().width;
            let left = GUTTER_NUMBERS_RIGHT - widest as f32 * digit;

            for line in first..last {
                let y = bounds.y + (line - first) as f32 * line_height;
                let color =
                    if line == caret_line { self.palette.gutter_text_current } else { self.palette.gutter_text };
                renderer.fill_text(
                    text::Text {
                        content: format!("{:>widest$}", line + 1),
                        bounds: Size::new(GUTTER_BORDER, line_height),
                        size,
                        line_height: text::LineHeight::Absolute(Pixels::from(line_height)),
                        font,
                        align_x: text::Alignment::Left,
                        align_y: iced::alignment::Vertical::Top,
                        shaping: text::Shaping::Advanced,
                        wrapping: text::Wrapping::None,
                    },
                    Point::new(bounds.x + left, y),
                    color,
                    bounds,
                );
            }

            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle::new(
                        Point::new(bounds.x + GUTTER_BORDER, bounds.y),
                        Size::new(1.0, bounds.height),
                    ),
                    ..renderer::Quad::default()
                },
                self.palette.gutter_border,
            );
        }

        if let Some((start, end)) = state.selection() {
            for line in first..last {
                let line_start = state.rope.line_to_char(line);
                let length = state.line_text(line).chars().count();
                let line_end = line_start + length;
                let (from, to) = (start.max(line_start), end.min(line_end + 1));
                if from >= to {
                    continue;
                }
                let text = state.line_text(line);
                let paragraph = line_paragraph(&text, state.font_size, state.line_height);
                let x0 = origin + column_x(&paragraph, from - line_start);
                let x1 = origin + column_x(&paragraph, (to - line_start).min(length));
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
                    self.palette.selection,
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
                Point::new(bounds.x + origin, y),
                self.palette.text,
                bounds,
            );

            // Re-draw the highlighted runs on top, in their own colours. Spans never overlap
            // and glyph positions are identical, so overdrawing is exact and cheap.
            let line_start = state.rope.line_to_char(line);
            let line_text = state.line_text(line);
            let paragraph = line_paragraph(&line_text, state.font_size, state.line_height);
            for highlight in &state.highlights {
                if highlight.end <= line_start {
                    continue;
                }
                if highlight.start >= line_start + line_text.chars().count() {
                    break;
                }
                let from = highlight.start.saturating_sub(line_start);
                let to = (highlight.end - line_start).min(line_text.chars().count());
                if from >= to {
                    continue;
                }
                let run: String = line_text.chars().skip(from).take(to - from).collect();
                let x = bounds.x + origin + column_x(&paragraph, from);
                renderer.fill_text(
                    text::Text {
                        content: run,
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
                    self.palette.syntax.color(highlight.class),
                    bounds,
                );
                let _ = highlight.class;
            }
        }

        // The IME composition, drawn on the spot: underlined, at the caret.
        if let Some((content, _)) = &state.preedit
            && state.focused
        {
            let line = state.line();
            let line_text = state.line_text(line);
            let x =
                bounds.x + column_x(&line_paragraph(&line_text, state.font_size, state.line_height), state.column());
            let y = bounds.y + (line - state.scroll) as f32 * line_height;
            let preedit_width = line_paragraph(content, state.font_size, state.line_height).min_bounds().width;
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle::new(Point::new(x, y + line_height - 2.0), Size::new(preedit_width, 1.0)),
                    ..renderer::Quad::default()
                },
                self.palette.preedit_underline,
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
                self.palette.preedit,
                bounds,
            );
        }

        // Overlay scrollbar, like the web editor's: a thin thumb on the right edge.
        let lines = state.line_count();
        let visible = (bounds.height / line_height).ceil() as usize;
        if lines > visible && visible > 0 {
            let track = 10.0;
            let total = bounds.height - 8.0;
            let thumb_height = (visible as f32 / lines as f32 * total).max(24.0);
            let max_scroll = (lines - visible) as f32;
            let offset =
                if max_scroll > 0.0 { (state.scroll as f32 / max_scroll) * (total - thumb_height) } else { 0.0 };
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle::new(
                        Point::new(bounds.x + bounds.width - track, bounds.y + 4.0 + offset),
                        Size::new(track - 4.0, thumb_height),
                    ),
                    border: iced::Border { radius: 3.0.into(), ..iced::Border::default() },
                    ..renderer::Quad::default()
                },
                self.palette.scrollbar,
            );
        }

        if state.focused {
            let line = state.line();
            let text = state.line_text(line);
            let paragraph = line_paragraph(&text, state.font_size, state.line_height);
            let x = bounds.x + origin + column_x(&paragraph, state.column());
            let y = bounds.y + (line - state.scroll) as f32 * line_height;
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle::new(Point::new(x, y), Size::new(1.5, line_height)),
                    ..renderer::Quad::default()
                },
                self.palette.caret,
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

/// Lays out one line, unbounded, with the same parameters the draw pass uses.
fn line_paragraph(content: &str, size: f32, line_height: f32) -> <iced::Renderer as text::Renderer>::Paragraph {
    <iced::Renderer as text::Renderer>::Paragraph::with_text(text::Text {
        content,
        bounds: Size::new(f32::INFINITY, line_height),
        size: Pixels::from(size),
        line_height: text::LineHeight::Absolute(Pixels::from(line_height)),
        font: Font::MONOSPACE,
        align_x: text::Alignment::Left,
        align_y: iced::alignment::Vertical::Top,
        shaping: text::Shaping::Advanced,
        wrapping: text::Wrapping::None,
    })
}

/// x of a caret sitting before `column` characters of `line` (0-based, grapheme-ish).
fn column_x(paragraph: &<iced::Renderer as text::Renderer>::Paragraph, column: usize) -> f32 {
    use iced::advanced::text::Paragraph as _;

    paragraph.grapheme_position(0, column).map_or(0.0, |point| point.x)
}

/// Nearest column for a click at `x`, using the same metrics the draw pass uses.
fn column_at_x(paragraph: &<iced::Renderer as text::Renderer>::Paragraph, length: usize, x: f32) -> usize {
    for column in 0..=length {
        if x < column_x(paragraph, column) {
            return column.saturating_sub(1);
        }
    }
    length
}

impl<'a, Message> From<Editor> for Element<'a, Message>
where
    Message: 'a,
{
    fn from(editor: Editor) -> Self {
        Element::new(editor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: f32 = 14.0;
    const LINE_HEIGHT: f32 = 20.0;

    /// A CJK glyph must be wider than an ASCII one, and the positions must grow.
    #[test]
    fn cjk_is_wider_than_ascii() {
        let ascii = paragraph("aaa", SIZE, LINE_HEIGHT);
        let cjk = paragraph("中中中", SIZE, LINE_HEIGHT);

        assert!(column_x(&cjk, 1) > column_x(&ascii, 1) * 1.5);
        assert!(column_x(&ascii, 1) > column_x(&ascii, 0));
        assert_eq!(column_x(&ascii, 0), 0.0);
    }

    /// Clicking at the x of a column must give that column back, for mixed text too.
    #[test]
    fn x_and_column_round_trip() {
        for line in ["fn main() {", "// 中文（IBus）注释", "a中b文c", ""] {
            let paragraph = paragraph(line, SIZE, LINE_HEIGHT);
            let length = line.chars().count();
            for column in 0..=length {
                let x = column_x(&paragraph, column);
                assert_eq!(column_at_x(&paragraph, length, x), column, "line {line:?} column {column} at x {x}");
            }
        }
    }

    /// The rope operations the widget drives: insert, delete, move.
    #[test]
    fn edits_go_through_the_rope() {
        let mut state = EditorState::new("ab\ncd");

        state.caret = 1;
        state.anchor = 1;
        state.insert("中");
        assert_eq!(state.len_chars(), 6);
        assert_eq!(state.line(), 0);
        assert_eq!(state.column(), 2);

        state.backspace();
        assert_eq!(state.len_chars(), 5);

        state.move_vertical(1, false);
        assert_eq!(state.line(), 1);
    }
}
