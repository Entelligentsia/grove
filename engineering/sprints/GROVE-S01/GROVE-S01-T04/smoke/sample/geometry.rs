//! A tiny Rust fixture the smoke test runs `ops::symbols` against.

/// A point in 2D space.
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    /// Construct a new point.
    pub fn new(x: f64, y: f64) -> Self {
        Point { x, y }
    }

    /// Euclidean distance to another point.
    pub fn distance(&self, other: &Point) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

/// Sum the perimeter of a closed polyline through `points`.
pub fn perimeter(points: &[Point]) -> f64 {
    let mut total = 0.0;
    for pair in points.windows(2) {
        total += pair[0].distance(&pair[1]);
    }
    total
}
