//! Symbols drawn as vectors on a 24 × 24 grid (lines 2 units wide, like Lucide or RustGIS):
//! sharp at every size and in both modes, no emoji font needed. Each symbol is spoken through
//! the widget that shows it (`widgets::icon_button`), never on its own.

use egui::epaint::{CornerRadius, PathShape, PathStroke, StrokeKind};
use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, vec2};

/// The symbols of plnwrk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Icon {
    /// New, add.
    Plus,
    /// Less, zoom out step.
    Minus,
    /// Close, remove.
    Close,
    ChevronLeft,
    ChevronRight,
    ChevronDown,
    ChevronUp,
    /// Done, ticked.
    Check,
    /// A case (Vorgang): a sheet with a folded corner.
    Case,
    /// A project: a briefcase.
    Project,
    /// A task: a ticked box.
    Task,
    /// A milestone: a diamond.
    Milestone,
    Undo,
    Redo,
    Save,
    /// Open a file: a folder.
    Open,
    /// Export: an arrow out of a tray.
    Export,
    /// Jump to today: a calendar with a dot.
    Today,
    /// Show the whole project: four corners.
    WholeProject,
    ZoomIn,
    ZoomOut,
    Filter,
    Person,
    Calendar,
    Clock,
    /// Waits for: two boxes joined by an arrow.
    Dependency,
    /// Delete: a bin.
    Delete,
    /// More: three dots.
    More,
    Board,
    List,
    Schedule,
    Overview,
    Search,
    Settings,
    Warning,
    /// Done: a ticked circle.
    Done,
    Help,
}

impl Icon {
    /// Every symbol, for tests and the overview.
    pub const ALL: [Icon; 37] = [
        Icon::Plus,
        Icon::Minus,
        Icon::Close,
        Icon::ChevronLeft,
        Icon::ChevronRight,
        Icon::ChevronDown,
        Icon::ChevronUp,
        Icon::Check,
        Icon::Case,
        Icon::Project,
        Icon::Task,
        Icon::Milestone,
        Icon::Undo,
        Icon::Redo,
        Icon::Save,
        Icon::Open,
        Icon::Export,
        Icon::Today,
        Icon::WholeProject,
        Icon::ZoomIn,
        Icon::ZoomOut,
        Icon::Filter,
        Icon::Person,
        Icon::Calendar,
        Icon::Clock,
        Icon::Dependency,
        Icon::Delete,
        Icon::More,
        Icon::Board,
        Icon::List,
        Icon::Schedule,
        Icon::Overview,
        Icon::Search,
        Icon::Settings,
        Icon::Warning,
        Icon::Done,
        Icon::Help,
    ];

    /// Draws the symbol into `rect` (square) in `color`.
    pub fn paint(self, painter: &Painter, rect: Rect, color: Color32) {
        painter.extend(self.shapes(rect, color));
    }

    /// The shapes of the symbol in `rect` (square) in `color`.
    pub fn shapes(self, rect: Rect, color: Color32) -> Vec<Shape> {
        let mut pen = Pen::new(rect, color);
        match self {
            Icon::Plus => {
                pen.line(&[(12.0, 5.0), (12.0, 19.0)]);
                pen.line(&[(5.0, 12.0), (19.0, 12.0)]);
            }
            Icon::Minus => pen.line(&[(5.0, 12.0), (19.0, 12.0)]),
            Icon::Close => {
                pen.line(&[(6.0, 6.0), (18.0, 18.0)]);
                pen.line(&[(18.0, 6.0), (6.0, 18.0)]);
            }
            Icon::ChevronLeft => pen.line(&[(15.0, 6.0), (9.0, 12.0), (15.0, 18.0)]),
            Icon::ChevronRight => pen.line(&[(9.0, 6.0), (15.0, 12.0), (9.0, 18.0)]),
            Icon::ChevronDown => pen.line(&[(6.0, 9.0), (12.0, 15.0), (18.0, 9.0)]),
            Icon::ChevronUp => pen.line(&[(6.0, 15.0), (12.0, 9.0), (18.0, 15.0)]),
            Icon::Check => pen.line(&[(5.0, 12.5), (10.0, 17.5), (19.0, 7.0)]),
            Icon::Case => {
                pen.closed(&[
                    (6.0, 3.0),
                    (14.0, 3.0),
                    (19.0, 8.0),
                    (19.0, 21.0),
                    (6.0, 21.0),
                ]);
                pen.line(&[(14.0, 3.0), (14.0, 8.0), (19.0, 8.0)]);
                pen.line(&[(9.0, 13.0), (16.0, 13.0)]);
                pen.line(&[(9.0, 17.0), (16.0, 17.0)]);
            }
            Icon::Project => {
                pen.rect((3.0, 7.0), (21.0, 20.0), 2.0);
                pen.line(&[(9.0, 7.0), (9.0, 4.5), (15.0, 4.5), (15.0, 7.0)]);
                pen.line(&[(3.0, 12.5), (21.0, 12.5)]);
            }
            Icon::Task => {
                pen.rect((4.0, 4.0), (20.0, 20.0), 3.0);
                pen.line(&[(8.0, 12.0), (11.0, 15.0), (16.5, 9.0)]);
            }
            Icon::Milestone => pen.closed(&[(12.0, 3.0), (21.0, 12.0), (12.0, 21.0), (3.0, 12.0)]),
            Icon::Undo => {
                pen.line(&[(9.0, 4.0), (4.0, 9.0), (9.0, 14.0)]);
                let mut path = vec![(4.0, 9.0), (14.0, 9.0)];
                path.extend(arc((14.0, 14.0), 5.0, -90.0, 90.0));
                path.push((8.0, 19.0));
                pen.line(&path);
            }
            Icon::Redo => {
                pen.line(&[(15.0, 4.0), (20.0, 9.0), (15.0, 14.0)]);
                let mut path = vec![(20.0, 9.0), (10.0, 9.0)];
                path.extend(arc((10.0, 14.0), 5.0, 270.0, 90.0));
                path.push((16.0, 19.0));
                pen.line(&path);
            }
            Icon::Save => {
                pen.closed(&[
                    (4.0, 4.0),
                    (16.5, 4.0),
                    (20.0, 7.5),
                    (20.0, 20.0),
                    (4.0, 20.0),
                ]);
                pen.line(&[(8.0, 4.0), (8.0, 9.0), (15.0, 9.0), (15.0, 4.0)]);
                pen.rect((7.5, 13.5), (16.5, 20.0), 1.0);
            }
            Icon::Open => pen.closed(&[
                (3.0, 6.0),
                (9.0, 6.0),
                (11.0, 8.5),
                (21.0, 8.5),
                (21.0, 19.0),
                (3.0, 19.0),
            ]),
            Icon::Export => {
                pen.line(&[(4.0, 14.0), (4.0, 20.0), (20.0, 20.0), (20.0, 14.0)]);
                pen.line(&[(12.0, 15.0), (12.0, 4.0)]);
                pen.line(&[(7.0, 9.0), (12.0, 4.0), (17.0, 9.0)]);
            }
            Icon::Today => {
                calendar(&mut pen);
                pen.dot((12.0, 15.0), 2.2);
            }
            Icon::WholeProject => {
                pen.line(&[(4.0, 9.0), (4.0, 4.0), (9.0, 4.0)]);
                pen.line(&[(15.0, 4.0), (20.0, 4.0), (20.0, 9.0)]);
                pen.line(&[(20.0, 15.0), (20.0, 20.0), (15.0, 20.0)]);
                pen.line(&[(9.0, 20.0), (4.0, 20.0), (4.0, 15.0)]);
            }
            Icon::ZoomIn => {
                magnifier(&mut pen);
                pen.line(&[(8.0, 10.5), (13.0, 10.5)]);
                pen.line(&[(10.5, 8.0), (10.5, 13.0)]);
            }
            Icon::ZoomOut => {
                magnifier(&mut pen);
                pen.line(&[(8.0, 10.5), (13.0, 10.5)]);
            }
            Icon::Search => magnifier(&mut pen),
            Icon::Filter => pen.closed(&[
                (4.0, 5.0),
                (20.0, 5.0),
                (14.0, 12.5),
                (14.0, 19.0),
                (10.0, 17.0),
                (10.0, 12.5),
            ]),
            Icon::Person => {
                pen.circle((12.0, 8.0), 4.0);
                pen.line(&arc((12.0, 21.0), 7.5, 180.0, 360.0));
            }
            Icon::Calendar => calendar(&mut pen),
            Icon::Clock => {
                pen.circle((12.0, 12.0), 9.0);
                pen.line(&[(12.0, 7.0), (12.0, 12.0), (15.5, 14.0)]);
            }
            Icon::Dependency => {
                pen.rect((3.0, 4.0), (10.0, 9.5), 1.5);
                pen.rect((14.0, 14.5), (21.0, 20.0), 1.5);
                pen.line(&[(10.0, 6.75), (12.0, 6.75), (12.0, 17.25), (14.0, 17.25)]);
            }
            Icon::Delete => {
                pen.line(&[(4.0, 7.0), (20.0, 7.0)]);
                pen.line(&[(9.0, 7.0), (9.0, 4.5), (15.0, 4.5), (15.0, 7.0)]);
                pen.line(&[(6.0, 7.0), (7.0, 20.0), (17.0, 20.0), (18.0, 7.0)]);
                pen.line(&[(10.0, 11.0), (10.0, 16.0)]);
                pen.line(&[(14.0, 11.0), (14.0, 16.0)]);
            }
            Icon::More => {
                for x in [6.0, 12.0, 18.0] {
                    pen.dot((x, 12.0), 1.7);
                }
            }
            Icon::Board => {
                pen.rect((3.0, 4.0), (21.0, 20.0), 2.0);
                pen.line(&[(9.0, 4.0), (9.0, 20.0)]);
                pen.line(&[(15.0, 4.0), (15.0, 20.0)]);
            }
            Icon::List => {
                for y in [6.0, 12.0, 18.0] {
                    pen.line(&[(9.0, y), (20.0, y)]);
                    pen.dot((4.5, y), 1.3);
                }
            }
            Icon::Schedule => {
                pen.line(&[(4.0, 6.5), (12.0, 6.5)]);
                pen.line(&[(8.0, 12.0), (17.0, 12.0)]);
                pen.line(&[(12.0, 17.5), (20.0, 17.5)]);
            }
            Icon::Overview => {
                pen.rect((4.0, 4.0), (10.5, 10.5), 1.5);
                pen.rect((13.5, 4.0), (20.0, 10.5), 1.5);
                pen.rect((4.0, 13.5), (10.5, 20.0), 1.5);
                pen.rect((13.5, 13.5), (20.0, 20.0), 1.5);
            }
            Icon::Settings => {
                pen.circle((12.0, 12.0), 3.0);
                pen.circle((12.0, 12.0), 7.0);
                for step in 0..8_u8 {
                    let angle = f32::from(step) * 45.0;
                    let (sin, cos) = angle.to_radians().sin_cos();
                    pen.line(&[
                        (12.0 + 7.0 * cos, 12.0 + 7.0 * sin),
                        (12.0 + 9.5 * cos, 12.0 + 9.5 * sin),
                    ]);
                }
            }
            Icon::Warning => {
                pen.closed(&[(12.0, 3.5), (21.0, 19.5), (3.0, 19.5)]);
                pen.line(&[(12.0, 9.5), (12.0, 13.5)]);
                pen.dot((12.0, 16.5), 1.3);
            }
            Icon::Done => {
                pen.circle((12.0, 12.0), 9.0);
                pen.line(&[(8.0, 12.5), (11.0, 15.5), (16.5, 9.5)]);
            }
            Icon::Help => {
                pen.circle((12.0, 12.0), 9.0);
                let mut path = arc((12.0, 9.5), 2.6, 180.0, 405.0);
                path.push((12.0, 13.5));
                pen.line(&path);
                pen.dot((12.0, 16.8), 1.2);
            }
        }
        pen.shapes
    }
}

fn calendar(pen: &mut Pen) {
    pen.rect((4.0, 5.5), (20.0, 20.0), 2.0);
    pen.line(&[(4.0, 10.0), (20.0, 10.0)]);
    pen.line(&[(8.0, 3.5), (8.0, 7.0)]);
    pen.line(&[(16.0, 3.5), (16.0, 7.0)]);
}

fn magnifier(pen: &mut Pen) {
    pen.circle((10.5, 10.5), 6.5);
    pen.line(&[(15.5, 15.5), (20.0, 20.0)]);
}

/// Points on a circle from `from` to `to` degrees (0 = right, clockwise on screen).
fn arc(center: (f32, f32), radius: f32, from: f32, to: f32) -> Vec<(f32, f32)> {
    (0..=12_u8)
        .map(|step| {
            let angle = (from + (to - from) * f32::from(step) / 12.0).to_radians();
            (
                center.0 + radius * angle.cos(),
                center.1 + radius * angle.sin(),
            )
        })
        .collect()
}

/// Draws in grid units (0–24) scaled into a rectangle.
struct Pen {
    shapes: Vec<Shape>,
    rect: Rect,
    scale: f32,
    stroke: Stroke,
}

impl Pen {
    fn new(rect: Rect, color: Color32) -> Self {
        let scale = rect.width().min(rect.height()) / 24.0;
        // Two grid units, but never thinner than a pixel and a half.
        let stroke = Stroke::new((2.0 * scale).max(1.5), color);
        Self {
            shapes: Vec::new(),
            rect,
            scale,
            stroke,
        }
    }

    fn at(&self, (x, y): (f32, f32)) -> Pos2 {
        self.rect.min + vec2(x, y) * self.scale
    }

    fn line(&mut self, points: &[(f32, f32)]) {
        let points: Vec<Pos2> = points.iter().map(|p| self.at(*p)).collect();
        let stroke = PathStroke::from(self.stroke);
        self.shapes
            .push(Shape::Path(PathShape::line(points, stroke)));
    }

    fn closed(&mut self, points: &[(f32, f32)]) {
        let points: Vec<Pos2> = points.iter().map(|p| self.at(*p)).collect();
        let stroke = PathStroke::from(self.stroke);
        self.shapes
            .push(Shape::Path(PathShape::closed_line(points, stroke)));
    }

    fn rect(&mut self, min: (f32, f32), max: (f32, f32), radius: f32) {
        let rect = Rect::from_min_max(self.at(min), self.at(max));
        self.shapes.push(Shape::rect_stroke(
            rect,
            CornerRadius::from(radius * self.scale),
            self.stroke,
            StrokeKind::Middle,
        ));
    }

    fn circle(&mut self, center: (f32, f32), radius: f32) {
        let center = self.at(center);
        self.shapes.push(Shape::circle_stroke(
            center,
            radius * self.scale,
            self.stroke,
        ));
    }

    fn dot(&mut self, center: (f32, f32), radius: f32) {
        let center = self.at(center);
        self.shapes.push(Shape::circle_filled(
            center,
            radius * self.scale,
            self.stroke.color,
        ));
    }
}

/// A square of this side, centred.
pub fn square(center: Pos2, side: f32) -> Rect {
    Rect::from_center_size(center, vec2(side, side))
}

#[cfg(test)]
mod tests {
    use egui::pos2;

    use super::*;

    /// Every symbol draws something inside its square, nothing outside.
    #[test]
    fn every_symbol_draws_inside_its_square() {
        for side in [16.0, 24.0, 32.0] {
            let rect = square(pos2(100.0, 100.0), side);
            for icon in Icon::ALL {
                let shapes = icon.shapes(rect, Color32::WHITE);
                assert!(!shapes.is_empty(), "{icon:?} draws nothing");
                for shape in shapes {
                    let bounds = shape.visual_bounding_rect();
                    assert!(
                        rect.expand(2.0).contains_rect(bounds),
                        "{icon:?} at {side}: {bounds:?} leaves {rect:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_list_of_all_symbols_is_complete() {
        // A new symbol must be added to ALL (and so to the test above).
        let distinct: std::collections::HashSet<_> = Icon::ALL.iter().collect();
        assert_eq!(distinct.len(), Icon::ALL.len());
    }
}
