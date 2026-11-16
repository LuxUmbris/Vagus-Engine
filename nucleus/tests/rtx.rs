use nucleus::rtx::{Camera, RtxRenderer, Triangle};

#[test]
fn hardware_renderer_dispatches_triangle_scene_and_returns_rgba_pixels() {
    let mut renderer = RtxRenderer::new(16, 16).expect("hardware ray tracing renderer");
    let triangle = Triangle {
        vertices: [[-1.0, -1.0, 0.0], [1.0, -1.0, 0.0], [0.0, 1.0, 0.0]],
    };
    let camera = Camera {
        origin: [0.0, 0.0, 2.0],
        target: [0.0, 0.0, 0.0],
        up: [0.0, 1.0, 0.0],
        vertical_fov_degrees: 60.0,
    };
    let pixels = renderer
        .trace(&[triangle], camera)
        .expect("trace triangle scene");

    assert_eq!(pixels.len(), 256);
    assert!(pixels.iter().any(|pixel| *pixel == [230, 64, 31, 255]));
    assert!(pixels.iter().any(|pixel| *pixel == [5, 10, 31, 255]));

    let repeated_pixels = renderer
        .trace(&[triangle], camera)
        .expect("trace the scene again with rebuilt acceleration structures");
    assert_eq!(repeated_pixels, pixels);
}
