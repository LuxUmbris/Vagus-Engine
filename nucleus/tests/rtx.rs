use nucleus::rtx::{Camera, RtxRenderer, Triangle};

#[test]
fn hardware_renderer_dispatches_triangle_scene_and_returns_rgba_pixels() {
    let mut renderer = RtxRenderer::new_standalone(16, 16).expect("hardware ray tracing renderer");
    let triangle = Triangle {
        vertices: [[-1.0, -1.0, 0.0], [1.0, -1.0, 0.0], [0.0, 1.0, 0.0]],
    };
    let camera = Camera {
        origin: [0.0, 0.0, 2.0],
        target: [0.0, 0.0, 0.0],
        up: [0.0, 1.0, 0.0],
        vertical_fov_degrees: 60.0,
        near_plane: 0.01,
        far_plane: 10_000.0,
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

#[test]
fn ray_tracing_obeys_camera_space_near_and_far_clip_planes() {
    let mut renderer = RtxRenderer::new_standalone(16, 16).expect("hardware ray tracing renderer");
    let camera = Camera {
        origin: [0.0, 0.0, 0.0],
        target: [0.0, 0.0, 1.0],
        up: [0.0, 1.0, 0.0],
        vertical_fov_degrees: 60.0,
        near_plane: 1.0,
        far_plane: 3.0,
    };
    let near_triangle = Triangle {
        vertices: [[-0.1, -0.1, 0.5], [0.1, -0.1, 0.5], [0.0, 0.1, 0.5]],
    };
    let near_pixels = renderer
        .trace(&[near_triangle], camera)
        .expect("trace geometry before the camera near plane");
    assert!(near_pixels.iter().all(|pixel| *pixel == [5, 10, 31, 255]));

    let far_triangle = Triangle {
        vertices: [[-1.5, -1.5, 2.9], [1.5, -1.5, 2.9], [0.0, 1.5, 2.9]],
    };
    let far_pixels = renderer
        .trace(&[far_triangle], camera)
        .expect("trace geometry inside the camera far plane");
    assert_ne!(far_pixels[7 * 16 + 7], [5, 10, 31, 255]);

    let beyond_far_triangle = Triangle {
        vertices: [[-2.0, -2.0, 3.1], [2.0, -2.0, 3.1], [0.0, 2.0, 3.1]],
    };
    let beyond_far_pixels = renderer
        .trace(&[beyond_far_triangle], camera)
        .expect("trace geometry beyond the configured far plane");
    assert!(
        beyond_far_pixels
            .iter()
            .all(|pixel| *pixel == [5, 10, 31, 255])
    );
}
