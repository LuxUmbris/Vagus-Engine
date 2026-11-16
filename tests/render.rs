use vagus::math::Vec2;
use vagus::render::{Color, NamedColor, Renderer};
use vagus::window::Window;

#[test]
fn public_renderer_draws_and_presents_a_window_frame() {
    let mut window = Window::create_window("vagus-render-test", 64, 64)
        .expect("create a native window for renderer integration");
    let mut renderer = Renderer::new(&mut window).expect("initialize Vulkan for the window");

    renderer.clear(NamedColor::Black);
    renderer.draw_dot(1, 1, NamedColor::White);
    renderer.draw_line((2, 2), (12, 2), NamedColor::Red);
    renderer.draw_triangle((10, 10), (20, 10), (15, 20), NamedColor::Green);
    renderer.draw_triangle_outline((22, 10), (32, 10), (27, 20), Color::rgb(180, 40, 220));
    renderer.draw_rectangle(2, 24, 12, 6, NamedColor::Cyan);
    renderer.draw_rectangle_outline(16, 24, 12, 6, NamedColor::Orange);
    renderer.draw_square(25, 2, 8, NamedColor::Blue);
    renderer.draw_circle((45, 12), 6, NamedColor::Yellow);
    renderer.draw_circle_outline((45, 30), 6, Color::rgb(220, 180, 20));
    renderer.draw_dot_vec2(Vec2 { x: 2.5, y: 3.0 }, NamedColor::White);
    renderer.draw_line_vec2(
        Vec2 { x: 2.0, y: 34.0 },
        Vec2 { x: 12.0, y: 34.0 },
        NamedColor::Red,
    );
    renderer.draw_triangle_vec2(
        Vec2 { x: 10.0, y: 36.0 },
        Vec2 { x: 20.0, y: 36.0 },
        Vec2 { x: 15.0, y: 46.0 },
        NamedColor::Green,
    );
    renderer.draw_triangle_outline_vec2(
        Vec2 { x: 22.0, y: 36.0 },
        Vec2 { x: 32.0, y: 36.0 },
        Vec2 { x: 27.0, y: 46.0 },
        NamedColor::Cyan,
    );
    renderer.draw_circle_vec2(Vec2 { x: 45.0, y: 44.0 }, 6, NamedColor::Yellow);
    renderer.draw_circle_outline_vec2(Vec2 { x: 45.0, y: 60.0 }, 3, NamedColor::Orange);
    renderer.present().expect("present a rendered frame");
}
