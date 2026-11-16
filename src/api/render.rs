use crate::api::math::Vec2;
use crate::api::window::Window;

/// An opaque RGB color used by the immediate-mode renderer.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Color(pub u8, pub u8, pub u8);

impl Color {
    pub const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self(red, green, blue)
    }
}

/// Common named colors, convertible to their 8-bit RGB values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NamedColor {
    Black,
    White,
    Red,
    Green,
    Blue,
    Yellow,
    Cyan,
    Magenta,
    Orange,
    Purple,
    Gray,
    LightGray,
    DarkGray,
}

impl From<NamedColor> for Color {
    fn from(color: NamedColor) -> Self {
        match color {
            NamedColor::Black => Self(0, 0, 0),
            NamedColor::White => Self(255, 255, 255),
            NamedColor::Red => Self(255, 0, 0),
            NamedColor::Green => Self(0, 255, 0),
            NamedColor::Blue => Self(0, 0, 255),
            NamedColor::Yellow => Self(255, 255, 0),
            NamedColor::Cyan => Self(0, 255, 255),
            NamedColor::Magenta => Self(255, 0, 255),
            NamedColor::Orange => Self(255, 165, 0),
            NamedColor::Purple => Self(128, 0, 128),
            NamedColor::Gray => Self(128, 128, 128),
            NamedColor::LightGray => Self(211, 211, 211),
            NamedColor::DarkGray => Self(64, 64, 64),
        }
    }
}

struct Canvas {
    width: u32,
    height: u32,
    pixels: Vec<Color>,
}

impl Canvas {
    fn new(width: u32, height: u32) -> Result<Self, String> {
        let count = usize::try_from(width)
            .ok()
            .and_then(|width| usize::try_from(height).ok()?.checked_mul(width))
            .ok_or_else(|| "render target dimensions overflow address space".to_owned())?;
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(count)
            .map_err(|error| format!("could not allocate render target: {error}"))?;
        pixels.resize(count, Color::default());
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    fn clear(&mut self, color: Color) {
        self.pixels.fill(color);
    }

    fn pixel(&self, x: u32, y: u32) -> Color {
        self.pixels[(y * self.width + x) as usize]
    }

    fn dot(&mut self, x: i32, y: i32, color: Color) {
        self.set_pixel(x, y, color);
    }

    fn dot_vec2(&mut self, point: Vec2, color: Color) {
        if let Some((x, y)) = pixel_point(point) {
            self.dot(x, y, color);
        }
    }

    fn line(&mut self, start: (i32, i32), end: (i32, i32), color: Color) {
        let Some(((mut x0, mut y0), (x1, y1))) = self.clip_line(start, end) else {
            return;
        };
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut error = dx + dy;
        loop {
            self.set_pixel(x0, y0, color);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let twice_error = 2 * error;
            if twice_error >= dy {
                error += dy;
                x0 += sx;
            }
            if twice_error <= dx {
                error += dx;
                y0 += sy;
            }
        }
    }

    fn line_vec2(&mut self, start: Vec2, end: Vec2, color: Color) {
        if let (Some(start), Some(end)) = (pixel_point(start), pixel_point(end)) {
            self.line(start, end, color);
        }
    }

    fn triangle(&mut self, a: (i32, i32), b: (i32, i32), c: (i32, i32), color: Color) {
        let area = edge(a, b, c.0 as i64, c.1 as i64);
        if area == 0 {
            self.line(a, b, color);
            self.line(b, c, color);
            self.line(c, a, color);
            return;
        }
        let min_x = a.0.min(b.0).min(c.0).max(0) as u32;
        let min_y = a.1.min(b.1).min(c.1).max(0) as u32;
        let max_x = a.0.max(b.0).max(c.0).min(self.width as i32 - 1);
        let max_y = a.1.max(b.1).max(c.1).min(self.height as i32 - 1);
        if max_x < min_x as i32 || max_y < min_y as i32 {
            return;
        }
        for y in min_y..=max_y as u32 {
            for x in min_x..=max_x as u32 {
                let (x, y) = (x as i64, y as i64);
                let e0 = edge(a, b, x, y);
                let e1 = edge(b, c, x, y);
                let e2 = edge(c, a, x, y);
                if (area > 0 && e0 >= 0 && e1 >= 0 && e2 >= 0)
                    || (area < 0 && e0 <= 0 && e1 <= 0 && e2 <= 0)
                {
                    self.set_pixel(x as i32, y as i32, color);
                }
            }
        }
    }

    fn triangle_vec2(&mut self, a: Vec2, b: Vec2, c: Vec2, color: Color) {
        if let (Some(a), Some(b), Some(c)) = (pixel_point(a), pixel_point(b), pixel_point(c)) {
            self.triangle(a, b, c, color);
        }
    }

    fn triangle_outline(&mut self, a: (i32, i32), b: (i32, i32), c: (i32, i32), color: Color) {
        self.line(a, b, color);
        self.line(b, c, color);
        self.line(c, a, color);
    }

    fn triangle_outline_vec2(&mut self, a: Vec2, b: Vec2, c: Vec2, color: Color) {
        if let (Some(a), Some(b), Some(c)) = (pixel_point(a), pixel_point(b), pixel_point(c)) {
            self.triangle_outline(a, b, c, color);
        }
    }

    fn rectangle(&mut self, x: i32, y: i32, width: u32, height: u32, color: Color) {
        if width == 0 || height == 0 {
            return;
        }
        let left = (x as i64).max(0);
        let top = (y as i64).max(0);
        let right = (x as i64 + width as i64).min(self.width as i64);
        let bottom = (y as i64 + height as i64).min(self.height as i64);
        for py in top..bottom {
            for px in left..right {
                self.set_pixel_i64(px, py, color);
            }
        }
    }

    fn rectangle_outline(&mut self, x: i32, y: i32, width: u32, height: u32, color: Color) {
        if width == 0 || height == 0 {
            return;
        }
        let left = x as i64;
        let top = y as i64;
        let right = left + width as i64 - 1;
        let bottom = top + height as i64 - 1;
        let visible_left = left.max(0);
        let visible_right = right.min(self.width as i64 - 1);
        for px in visible_left..=visible_right {
            self.set_pixel_i64(px, top, color);
            self.set_pixel_i64(px, bottom, color);
        }
        let visible_top = (top + 1).max(0);
        let visible_bottom = (bottom - 1).min(self.height as i64 - 1);
        for py in visible_top..=visible_bottom {
            self.set_pixel_i64(left, py, color);
            self.set_pixel_i64(right, py, color);
        }
    }

    fn square(&mut self, x: i32, y: i32, size: u32, color: Color) {
        if size == 0 || self.width == 0 || self.height == 0 {
            return;
        }
        let left = (x as i64).max(0);
        let top = (y as i64).max(0);
        let right = (x as i64 + size as i64).min(self.width as i64);
        let bottom = (y as i64 + size as i64).min(self.height as i64);
        for py in top..bottom {
            for px in left..right {
                self.set_pixel(px as i32, py as i32, color);
            }
        }
    }

    fn circle(&mut self, center: (i32, i32), radius: u32, color: Color) {
        if self.width == 0 || self.height == 0 {
            return;
        }
        let (cx, cy) = (center.0 as i64, center.1 as i64);
        let radius = radius as i64;
        let left = (cx - radius).max(0);
        let right = (cx + radius).min(self.width as i64 - 1);
        let top = (cy - radius).max(0);
        let bottom = (cy + radius).min(self.height as i64 - 1);
        let radius_sq = (radius as i128) * (radius as i128);
        for y in top..=bottom {
            for x in left..=right {
                let dx = x - cx;
                let dy = y - cy;
                if (dx as i128) * (dx as i128) + (dy as i128) * (dy as i128) <= radius_sq {
                    self.set_pixel(x as i32, y as i32, color);
                }
            }
        }
    }

    fn circle_vec2(&mut self, center: Vec2, radius: u32, color: Color) {
        if let Some(center) = pixel_point(center) {
            self.circle(center, radius, color);
        }
    }

    fn circle_outline(&mut self, center: (i32, i32), radius: u32, color: Color) {
        if radius == 0 {
            self.set_pixel(center.0, center.1, color);
            return;
        }
        if self.width == 0 || self.height == 0 {
            return;
        }
        let (cx, cy) = (center.0 as i64, center.1 as i64);
        let radius = radius as i64;
        let outer_sq = (radius as i128) * (radius as i128);
        let inner = radius - 1;
        let inner_sq = (inner as i128) * (inner as i128);
        let left = (cx - radius).max(0);
        let right = (cx + radius).min(self.width as i64 - 1);
        let top = (cy - radius).max(0);
        let bottom = (cy + radius).min(self.height as i64 - 1);
        for y in top..=bottom {
            for x in left..=right {
                let dx = x - cx;
                let dy = y - cy;
                let distance_sq = (dx as i128) * (dx as i128) + (dy as i128) * (dy as i128);
                if distance_sq <= outer_sq && distance_sq > inner_sq {
                    self.set_pixel_i64(x, y, color);
                }
            }
        }
    }

    fn circle_outline_vec2(&mut self, center: Vec2, radius: u32, color: Color) {
        if let Some(center) = pixel_point(center) {
            self.circle_outline(center, radius, color);
        }
    }

    fn set_pixel(&mut self, x: i32, y: i32, color: Color) {
        self.set_pixel_i64(x as i64, y as i64, color);
    }

    fn set_pixel_i64(&mut self, x: i64, y: i64, color: Color) {
        if x < 0 || y < 0 || x >= self.width as i64 || y >= self.height as i64 {
            return;
        }
        self.pixels[y as usize * self.width as usize + x as usize] = color;
    }

    fn clip_line(&self, start: (i32, i32), end: (i32, i32)) -> Option<((i32, i32), (i32, i32))> {
        if self.width == 0 || self.height == 0 {
            return None;
        }
        let (mut x0, mut y0) = (start.0 as f64, start.1 as f64);
        let (mut x1, mut y1) = (end.0 as f64, end.1 as f64);
        let (max_x, max_y) = (self.width as f64 - 1.0, self.height as f64 - 1.0);
        for _ in 0..8 {
            let c0 = out_code(x0, y0, max_x, max_y);
            let c1 = out_code(x1, y1, max_x, max_y);
            if c0 | c1 == 0 {
                return Some((
                    (x0.round() as i32, y0.round() as i32),
                    (x1.round() as i32, y1.round() as i32),
                ));
            }
            if c0 & c1 != 0 {
                return None;
            }
            let code = if c0 != 0 { c0 } else { c1 };
            let (x, y) = if code & 8 != 0 {
                (x0 + (x1 - x0) * (max_y - y0) / (y1 - y0), max_y)
            } else if code & 4 != 0 {
                (x0 + (x1 - x0) * (0.0 - y0) / (y1 - y0), 0.0)
            } else if code & 2 != 0 {
                (max_x, y0 + (y1 - y0) * (max_x - x0) / (x1 - x0))
            } else {
                (0.0, y0 + (y1 - y0) * (0.0 - x0) / (x1 - x0))
            };
            if code == c0 {
                (x0, y0) = (x, y);
            } else {
                (x1, y1) = (x, y);
            }
        }
        None
    }
}

fn pixel_point(point: Vec2) -> Option<(i32, i32)> {
    if !point.x.is_finite() || !point.y.is_finite() {
        return None;
    }
    let x = f64::from(point.x.round());
    let y = f64::from(point.y.round());
    if x < i32::MIN as f64 || x > i32::MAX as f64 || y < i32::MIN as f64 || y > i32::MAX as f64 {
        return None;
    }
    Some((x as i32, y as i32))
}

fn edge(a: (i32, i32), b: (i32, i32), x: i64, y: i64) -> i128 {
    (b.0 as i128 - a.0 as i128) * (y as i128 - a.1 as i128)
        - (b.1 as i128 - a.1 as i128) * (x as i128 - a.0 as i128)
}

fn out_code(x: f64, y: f64, max_x: f64, max_y: f64) -> u8 {
    (if x < 0.0 {
        1
    } else if x > max_x {
        2
    } else {
        0
    }) | (if y < 0.0 {
        4
    } else if y > max_y {
        8
    } else {
        0
    })
}

/// A high-level drawing context attached to one native window.
pub struct Renderer<'window> {
    window: &'window mut Window,
    canvas: Canvas,
    presenter: nucleus::Renderer,
}

impl<'window> Renderer<'window> {
    pub fn new(window: &'window mut Window) -> Result<Self, String> {
        let (width, height) = window.framebuffer_size;
        let canvas = Canvas::new(width, height)?;
        let presenter = nucleus::Renderer::new(&window.backend, width, height)?;
        Ok(Self {
            window,
            canvas,
            presenter,
        })
    }

    pub fn window_mut(&mut self) -> &mut Window {
        self.window
    }

    pub fn clear(&mut self, color: impl Into<Color>) {
        self.canvas.clear(color.into());
    }

    pub fn draw_dot(&mut self, x: i32, y: i32, color: impl Into<Color>) {
        self.canvas.dot(x, y, color.into());
    }

    /// Draws a point at rounded `Vec2` coordinates; invalid coordinates are ignored.
    pub fn draw_dot_vec2(&mut self, point: Vec2, color: impl Into<Color>) {
        self.canvas.dot_vec2(point, color.into());
    }

    pub fn draw_line(&mut self, start: (i32, i32), end: (i32, i32), color: impl Into<Color>) {
        self.canvas.line(start, end, color.into());
    }

    /// Draws between rounded `Vec2` endpoints; invalid endpoints are ignored.
    pub fn draw_line_vec2(&mut self, start: Vec2, end: Vec2, color: impl Into<Color>) {
        self.canvas.line_vec2(start, end, color.into());
    }

    pub fn draw_triangle(
        &mut self,
        a: (i32, i32),
        b: (i32, i32),
        c: (i32, i32),
        color: impl Into<Color>,
    ) {
        self.canvas.triangle(a, b, c, color.into());
    }

    /// Draws a filled triangle from rounded `Vec2` vertices.
    pub fn draw_triangle_vec2(&mut self, a: Vec2, b: Vec2, c: Vec2, color: impl Into<Color>) {
        self.canvas.triangle_vec2(a, b, c, color.into());
    }

    pub fn draw_triangle_outline(
        &mut self,
        a: (i32, i32),
        b: (i32, i32),
        c: (i32, i32),
        color: impl Into<Color>,
    ) {
        self.canvas.triangle_outline(a, b, c, color.into());
    }

    /// Draws a triangle outline from rounded `Vec2` vertices.
    pub fn draw_triangle_outline_vec2(
        &mut self,
        a: Vec2,
        b: Vec2,
        c: Vec2,
        color: impl Into<Color>,
    ) {
        self.canvas.triangle_outline_vec2(a, b, c, color.into());
    }

    pub fn draw_rectangle(
        &mut self,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        color: impl Into<Color>,
    ) {
        self.canvas.rectangle(x, y, width, height, color.into());
    }

    pub fn draw_rectangle_outline(
        &mut self,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        color: impl Into<Color>,
    ) {
        self.canvas
            .rectangle_outline(x, y, width, height, color.into());
    }

    pub fn draw_square(&mut self, x: i32, y: i32, size: u32, color: impl Into<Color>) {
        self.canvas.square(x, y, size, color.into());
    }

    pub fn draw_circle(&mut self, center: (i32, i32), radius: u32, color: impl Into<Color>) {
        self.canvas.circle(center, radius, color.into());
    }

    /// Draws a circle at a rounded `Vec2` center.
    pub fn draw_circle_vec2(&mut self, center: Vec2, radius: u32, color: impl Into<Color>) {
        self.canvas.circle_vec2(center, radius, color.into());
    }

    pub fn draw_circle_outline(
        &mut self,
        center: (i32, i32),
        radius: u32,
        color: impl Into<Color>,
    ) {
        self.canvas.circle_outline(center, radius, color.into());
    }

    /// Draws a circle outline at a rounded `Vec2` center.
    pub fn draw_circle_outline_vec2(&mut self, center: Vec2, radius: u32, color: impl Into<Color>) {
        self.canvas
            .circle_outline_vec2(center, radius, color.into());
    }

    pub fn present(&mut self) -> Result<(), String> {
        let (width, height) = self.window.framebuffer_size;
        if (width, height) != (self.canvas.width, self.canvas.height) {
            self.canvas = Canvas::new(width, height)?;
        }
        let pixels: Vec<_> = self
            .canvas
            .pixels
            .iter()
            .map(|Color(red, green, blue)| [*red, *green, *blue])
            .collect();
        self.presenter.present(&pixels, width, height)
    }
}

#[cfg(test)]
mod tests {
    use super::{Canvas, Color, NamedColor};
    use crate::api::math::Vec2;

    #[test]
    fn named_colors_map_to_rgb_values() {
        assert_eq!(Color::from(NamedColor::Black), Color(0, 0, 0));
        assert_eq!(Color::from(NamedColor::White), Color(255, 255, 255));
        assert_eq!(Color::from(NamedColor::Red), Color(255, 0, 0));
        assert_eq!(Color::from(NamedColor::Blue), Color(0, 0, 255));
    }

    #[test]
    fn software_canvas_draws_primitives_with_clipping() {
        let mut canvas = Canvas::new(12, 12).unwrap();
        let red = Color::from(NamedColor::Red);
        canvas.clear(Color(0, 0, 0));
        canvas.dot(0, 0, red);
        canvas.line((1, 2), (5, 2), red);
        canvas.triangle((1, 4), (5, 4), (3, 8), red);
        canvas.square(7, 1, 3, red);
        canvas.circle((8, 8), 2, red);

        assert_eq!(canvas.pixel(0, 0), red);
        assert_eq!(canvas.pixel(3, 2), red);
        assert_eq!(canvas.pixel(3, 5), red);
        assert_eq!(canvas.pixel(8, 2), red);
        assert_eq!(canvas.pixel(8, 8), red);
        assert_eq!(canvas.pixel(11, 11), Color(0, 0, 0));
        canvas.dot(-1, 0, red);
        canvas.dot(12, 0, red);
    }

    #[test]
    fn vec2_coordinates_round_to_pixels_and_ignore_invalid_points() {
        let mut canvas = Canvas::new(6, 6).unwrap();
        let red = Color::from(NamedColor::Red);
        canvas.dot_vec2(Vec2 { x: 1.6, y: 2.2 }, red);
        canvas.line_vec2(Vec2 { x: 0.0, y: 4.0 }, Vec2 { x: 2.9, y: 4.0 }, red);
        canvas.dot_vec2(
            Vec2 {
                x: f32::NAN,
                y: 0.0,
            },
            red,
        );
        canvas.dot_vec2(
            Vec2 {
                x: i32::MAX as f32,
                y: 0.0,
            },
            red,
        );

        assert_eq!(canvas.pixel(2, 2), red);
        assert_eq!(canvas.pixel(2, 4), red);
        assert_eq!(canvas.pixel(0, 0), Color::default());
    }

    #[test]
    fn rectangle_primitives_clip_and_preserve_the_requested_color() {
        let mut canvas = Canvas::new(8, 8).unwrap();
        let background = Color(0, 0, 0);
        let fill = Color(20, 80, 160);
        let outline = Color(200, 40, 10);
        canvas.clear(background);

        canvas.rectangle(-1, 1, 4, 3, fill);
        canvas.rectangle_outline(4, 1, 4, 3, outline);

        assert_eq!(canvas.pixel(0, 1), fill);
        assert_eq!(canvas.pixel(2, 2), fill);
        assert_eq!(canvas.pixel(3, 3), background);
        assert_eq!(canvas.pixel(4, 1), outline);
        assert_eq!(canvas.pixel(7, 2), outline);
        assert_eq!(canvas.pixel(6, 2), background);
    }

    #[test]
    fn circle_and_triangle_outlines_use_the_passed_color() {
        let mut canvas = Canvas::new(10, 10).unwrap();
        let background = Color(0, 0, 0);
        let circle = Color(0, 180, 40);
        let triangle = Color(80, 30, 200);
        canvas.clear(background);

        canvas.circle_outline((2, 2), 2, circle);
        canvas.triangle_outline((5, 1), (9, 1), (7, 5), triangle);

        assert_eq!(canvas.pixel(2, 0), circle);
        assert_eq!(canvas.pixel(2, 2), background);
        assert_eq!(canvas.pixel(5, 1), triangle);
        assert_eq!(canvas.pixel(7, 5), triangle);
        assert_eq!(canvas.pixel(7, 2), background);
    }
}
