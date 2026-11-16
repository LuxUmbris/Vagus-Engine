//! Copyright © 2026 Vagus Engine Contributors
//! The authors of this file (the "Software") are not reliable
//! for any damages caused by the Software.
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
//! NEITHER EXPRESS OR IMPLIED. EVERY COPY OF THE SOFTWARE MUST
//! CONTAIN THIS TEXT. ANY MODIFICATIONS OF ANY KIND OF THE
//! SOFTWARE MUST BE EXPLICITLY STATED.
//!
//! For further information, see:
//! github.com/LuxUmbris/Vagus-Engine/LICENSE.
//!
//! Authors: Vincent Noll

use vagus::math::Vec2;
use vagus::render::{Color, NamedColor, Renderer};
use vagus::window::Window;

fn point(x: f32, y: f32) -> Vec2 {
    Vec2 { x, y }
}

#[test]
fn public_renderer_draws_and_presents_a_window_frame() {
    let mut window = Window::create_window("vagus-render-test", 64, 64)
        .expect("create a native window for renderer integration");
    let mut renderer = Renderer::new(&mut window).expect("initialize Vulkan for the window");

    renderer.clear(NamedColor::Black);
    renderer.draw_dot(point(1.0, 1.0), NamedColor::White);
    renderer.draw_line(point(2.0, 2.0), point(12.0, 2.0), NamedColor::Red);
    renderer.draw_triangle(
        point(10.0, 10.0),
        point(20.0, 10.0),
        point(15.0, 20.0),
        NamedColor::Green,
    );
    renderer.draw_triangle_outline(
        point(22.0, 10.0),
        point(32.0, 10.0),
        point(27.0, 20.0),
        Color::rgb(180, 40, 220),
    );
    renderer.draw_rectangle(point(2.0, 24.0), 12, 6, NamedColor::Cyan);
    renderer.draw_rectangle_outline(point(16.0, 24.0), 12, 6, NamedColor::Orange);
    renderer.draw_square(point(25.0, 2.0), 8, NamedColor::Blue);
    renderer.draw_circle(point(45.0, 12.0), 6, NamedColor::Yellow);
    renderer.draw_circle_outline(point(45.0, 30.0), 6, Color::rgb(220, 180, 20));
    renderer.present().expect("present a rendered frame");
}
