//! Copyright © 2026 Vagus Engine Contributors
//! The authors of this file (the "Software") are not reliable
//! for any damages caused by the Software.
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
//! NEITHER EXPRESS NOR IMPLIED. EVERY COPY OF THE SOFTWARE MUST
//! CONTAIN THIS TEXT. ANY MODIFICATIONS OF ANY KIND OF THE
//! SOFTWARE MUST BE EXPLICITLY STATED.
//!
//! For further information, see:
//! github.com/LuxUmbris/Vagus-Engine/LICENSE.
//!
//! Authors: Vincent Noll

use std::sync::{Arc, Barrier};
use vagus::math::Vec2;
use vagus::render::{Color, NamedColor, Renderer};
use vagus::rtx::{Camera, Scene, Triangle};
use vagus::texture::Texture;
use vagus::window::Window;

#[test]
fn renderer_gpu_draw_texture_and_optional_rtx_can_submit_from_parallel_threads() {
    let mut window = Window::create_window("vagus-shared-queue-test", 64, 64)
        .expect("create a native window for shared-device queue test");
    let mut renderer = Renderer::new(&mut window).expect("initialize window renderer");
    let ray_tracer = if renderer.ray_tracing_supported() {
        eprintln!("RTX supported; exercising renderer, texture, and RTX submissions");
        Some(
            renderer
                .rtx_renderer(8, 8)
                .expect("create RTX renderer from presentation device"),
        )
    } else {
        eprintln!("RTX unsupported; exercising shared renderer/texture submissions only");
        None
    };
    let texture_device = renderer
        .texture_device()
        .expect("create texture uploader from presentation device");
    let scene = Scene::new(vec![Triangle::new([
        [-0.6, -0.6, 1.0],
        [0.6, -0.6, 1.0],
        [0.0, 0.6, 1.0],
    ])
    .expect("create test triangle")])
    .expect("create test scene");
    let camera = Camera::new(
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 0.0],
        60.0,
        0.01,
        10_000.0,
    )
    .expect("create test camera");
    let start = Arc::new(Barrier::new(if ray_tracer.is_some() { 3 } else { 2 }));

    std::thread::scope(|scope| {
        let ray_thread = ray_tracer.map(|mut ray_tracer| {
            let ray_start = Arc::clone(&start);
            scope.spawn(move || {
                ray_start.wait();
                for _ in 0..8 {
                    ray_tracer
                        .render(&scene, &camera)
                        .expect("trace a frame on the shared Vulkan device");
                }
            })
        });
        let texture_start = Arc::clone(&start);
        let texture_thread = scope.spawn(move || {
            texture_start.wait();
            let texture =
                Texture::from_rgba8(1, 1, &[25, 50, 75, 255]).expect("create test texture");
            let mut uploaded = Vec::new();
            for _ in 0..8 {
                uploaded.push(
                    texture_device
                        .upload(&texture)
                        .expect("upload on the shared Vulkan device"),
                );
            }
            uploaded
        });

        start.wait();
        renderer.draw_triangle_gpu(
            Vec2 { x: 8.0, y: 8.0 },
            Vec2 { x: 56.0, y: 8.0 },
            Vec2 { x: 32.0, y: 56.0 },
            NamedColor::Green,
        );
        let pixels = renderer
            .present_gpu_readback(Color::rgb(0, 0, 0))
            .expect("rasterize and read back a GPU triangle during shared-device submissions");
        assert_eq!(pixels[32 * 64 + 32], [0, 255, 0, 255]);
        assert_eq!(pixels[0], [0, 0, 0, 255]);

        for _ in 0..8 {
            renderer.draw_triangle_gpu(
                Vec2 { x: 8.0, y: 8.0 },
                Vec2 { x: 56.0, y: 8.0 },
                Vec2 { x: 32.0, y: 56.0 },
                NamedColor::Green,
            );
            renderer
                .present()
                .expect("present a frame on the shared Vulkan device");
        }
        if let Some(ray_thread) = ray_thread {
            ray_thread
                .join()
                .expect("RTX submission thread should not panic");
        }
        let _uploaded_textures = texture_thread
            .join()
            .expect("texture submission thread should not panic");
        assert_eq!(_uploaded_textures.len(), 8);
    });
}
