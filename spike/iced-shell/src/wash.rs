//! The project-colour wash behind the islands.
//!
//! The web paints `radial-gradient(600px 300px at 0 0, project-color 22%, transparent)` over
//! `--bg-frame`. iced only has linear gradients, so this widget approximates the ellipse with a
//! stack of horizontal bands: each band carries a linear gradient from the tint to transparent
//! whose width follows the ellipse, which is what a radial gradient is at that scale.

use iced::advanced::renderer::Renderer as _;
use iced::advanced::widget::Tree;
use iced::advanced::{Layout, Widget, layout, renderer};
use iced::gradient::{Gradient, Linear};
use iced::{Color, Element, Length, Rectangle, Size};

/// The web's radial is 600x300 anchored at the top-left corner.
const RADIUS_X: f32 = 600.0;
const RADIUS_Y: f32 = 300.0;
/// Bands used to approximate the ellipse; 40 is smooth at 22% opacity.
const BANDS: usize = 40;

pub struct Wash {
    pub tint: Color,
    pub frame: Color,
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for Wash {
    fn size(&self) -> Size<Length> {
        Size { width: Length::Fill, height: Length::Fill }
    }

    fn layout(&mut self, _tree: &mut Tree, _renderer: &iced::Renderer, limits: &layout::Limits) -> layout::Node {
        layout::atomic(limits, Length::Fill, Length::Fill)
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut iced::Renderer,
        _theme: &iced::Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: iced::advanced::mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        if !bounds.intersects(viewport) {
            return;
        }

        renderer.fill_quad(renderer::Quad { bounds, ..renderer::Quad::default() }, self.frame);

        let band_height = RADIUS_Y / BANDS as f32;
        for band in 0..BANDS {
            let y = band as f32 * band_height;
            // Mid-band height as a fraction of the ellipse's vertical radius.
            let fy = (y + band_height / 2.0) / RADIUS_Y;
            if fy >= 1.0 {
                break;
            }
            let width = RADIUS_X * (1.0 - fy * fy).sqrt();
            if width <= 0.0 {
                continue;
            }

            let gradient = Gradient::Linear(
                Linear::new(iced::Radians(0.0)).add_stop(0.0, self.tint).add_stop(1.0, Color { a: 0.0, ..self.tint }),
            );

            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle::new(
                        iced::Point::new(bounds.x, bounds.y + y),
                        Size::new(width, band_height + 0.5),
                    ),
                    ..renderer::Quad::default()
                },
                gradient,
            );
        }
    }
}

impl<'a, Message> From<Wash> for Element<'a, Message> {
    fn from(wash: Wash) -> Self {
        Element::new(wash)
    }
}
