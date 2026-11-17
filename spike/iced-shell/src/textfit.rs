//! Text measurement for the chrome: the tab strip caps a label at 260px and ends it with an
//! ellipsis (`Tabs.module.css` `.tabLabel { max-width: 260px; text-overflow: ellipsis }`).
//!
//! `text-overflow` has no counterpart in iced, so the label is measured with the same
//! `Paragraph` the renderer uses and truncated to fit.

use iced::advanced::text::{Paragraph as _, Text};
use iced::{Font, Pixels, Size};

/// The longest prefix of `label` that fits `max_width`, with `…` appended when it is shortened.
pub fn ellipsize(label: &str, max_width: f32, size: f32, font: Font) -> String {
    if width(label, size, font) <= max_width {
        return label.to_owned();
    }

    let chars: Vec<char> = label.chars().collect();
    let mut best = String::new();
    for take in (1..chars.len()).rev() {
        let candidate: String = chars[..take].iter().collect::<String>() + "…";
        if width(&candidate, size, font) <= max_width {
            best = candidate;
            break;
        }
    }
    if best.is_empty() { "…".to_owned() } else { best }
}

/// Laid-out width of `text` at `size`, using the app's renderer.
pub fn width(text: &str, size: f32, font: Font) -> f32 {
    let paragraph = <iced::Renderer as iced::advanced::text::Renderer>::Paragraph::with_text(Text {
        content: text,
        bounds: Size::new(f32::INFINITY, size * 1.4),
        size: Pixels::from(size),
        line_height: iced::advanced::text::LineHeight::Absolute(Pixels::from(size * 1.4)),
        font,
        align_x: iced::advanced::text::Alignment::Left,
        align_y: iced::alignment::Vertical::Top,
        shaping: iced::advanced::text::Shaping::Advanced,
        wrapping: iced::advanced::text::Wrapping::None,
    });

    paragraph.min_bounds().width
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_labels_are_left_alone() {
        assert_eq!(ellipsize("main.rs", 260.0, 13.0, Font::default()), "main.rs");
    }

    #[test]
    fn long_labels_are_shortened_with_an_ellipsis() {
        let long = "a_very_long_file_name_that_should_not_fit_inside_the_tab_width_at_all.rs";
        let fitted = ellipsize(long, 260.0, 13.0, Font::default());
        assert!(fitted.ends_with('…'));
        assert!(width(&fitted, 13.0, Font::default()) <= 260.0);
    }
}
