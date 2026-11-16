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

use vagus::rtx::{Camera, RtxRenderer, Scene, Triangle};

#[test]
fn rtx_rejects_zero_sized_targets_before_backend_initialization() {
    assert!(RtxRenderer::new(0, 1).is_err());
    assert!(RtxRenderer::new(1, 0).is_err());
    assert!(RtxRenderer::new(u32::MAX, u32::MAX).is_err());
}

#[test]
fn rtx_rejects_invalid_scene_and_camera_data() {
    assert!(Scene::new(Vec::new()).is_err());
    assert!(Camera::new([0.0; 3], [0.0; 3], [0.0, 1.0, 0.0], 60.0).is_err());
    assert!(Camera::new([0.0; 3], [0.0, 0.0, 1.0], [0.0, 0.0, 2.0], 60.0).is_err());
    assert!(Camera::new([0.0; 3], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0], 180.0).is_err());
    assert!(Triangle::new([[f32::NAN, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]).is_err());
}

#[test]
fn hardware_ray_tracer_renders_triangle_hit_and_miss_pixels() {
    if !RtxRenderer::is_hardware_supported() {
        eprintln!("skipping: no Vulkan KHR hardware ray-tracing device is available");
        return;
    }

    let triangle = Triangle::new([[-0.6, -0.6, 1.0], [0.6, -0.6, 1.0], [0.0, 0.6, 1.0]])
        .expect("finite triangle");
    let scene = Scene::new(vec![triangle]).expect("nonempty scene");
    let camera = Camera::new([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0], 60.0)
        .expect("valid perspective camera");
    let mut renderer = RtxRenderer::new(8, 8).expect("create offscreen RT renderer");
    let frame = renderer
        .render(&scene, &camera)
        .expect("trace triangle scene");

    assert_eq!((frame.width(), frame.height()), (8, 8));
    assert_eq!(frame.pixel(8, 0), None);
    let hit = frame.pixel(4, 4).expect("center pixel");
    let miss = frame.pixel(0, 0).expect("corner pixel");
    assert_eq!(hit, [230, 64, 31, 255]);
    assert_eq!(miss, [5, 10, 31, 255]);
}
