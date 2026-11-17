//! The 6px band between a tool window and the editor, draggable like the web's splitter.
//!
//! iced has no built-in draggable divider, so this is a small custom widget: it tracks hover
//! and drag state in its tree node and publishes the cursor's x while dragging, which the app
//! turns into a panel width.

use iced::advanced::renderer::Renderer as _;
use iced::advanced::widget::{Tree, tree::State as TreeState};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, renderer};
use iced::{Color, Element, Length, Rectangle, Size};

#[derive(Debug, Default)]
pub struct State {
    hovering: bool,
    dragging: bool,
}

pub struct Splitter<'a, Message> {
    /// `--bg-frame`; the band sits in the gap between islands.
    pub color: Color,
    /// `--accent` at low opacity while the pointer is on it.
    pub hover: Color,
    pub on_drag: Box<dyn Fn(f32) -> Message + 'a>,
}

impl<'a, Message> Splitter<'a, Message> {
    pub fn new(color: Color, hover: Color, on_drag: impl Fn(f32) -> Message + 'a) -> Self {
        Self { color, hover, on_drag: Box::new(on_drag) }
    }
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for Splitter<'_, Message> {
    fn size(&self) -> Size<Length> {
        Size { width: Length::Fixed(6.0), height: Length::Fill }
    }

    fn state(&self) -> TreeState {
        TreeState::new(State::default())
    }

    fn layout(&mut self, _tree: &mut Tree, _renderer: &iced::Renderer, limits: &layout::Limits) -> layout::Node {
        layout::atomic(limits, Length::Fixed(6.0), Length::Fill)
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &iced::Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &iced::Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();
        let bounds = layout.bounds();

        match event {
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) if cursor.is_over(bounds) => {
                state.dragging = true;
                shell.capture_event();
            }
            iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if state.dragging {
                    state.dragging = false;
                    shell.request_redraw();
                }
            }
            iced::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if state.dragging {
                    if let Some(position) = cursor.position() {
                        shell.publish((self.on_drag)(position.x));
                    }
                } else {
                    let hovering = cursor.is_over(bounds);
                    if hovering != state.hovering {
                        state.hovering = hovering;
                        shell.request_redraw();
                    }
                }
            }
            _ => {}
        }
    }

    fn draw(
        &self,
        tree: &Tree,
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
        let state = tree.state.downcast_ref::<State>();
        let color = if state.dragging || state.hovering { self.hover } else { self.color };
        renderer.fill_quad(renderer::Quad { bounds, ..renderer::Quad::default() }, color);
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) {
            mouse::Interaction::ResizingHorizontally
        } else {
            mouse::Interaction::default()
        }
    }
}

impl<'a, Message> From<Splitter<'a, Message>> for Element<'a, Message>
where
    Message: 'a,
{
    fn from(splitter: Splitter<'a, Message>) -> Self {
        Element::new(splitter)
    }
}
