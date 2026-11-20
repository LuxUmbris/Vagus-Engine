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

use vagus::heightmap::HeightMap;
use vagus::material::Material;
use vagus::math::Vec3;
use vagus::physics::PhysicsObject3D;
use vagus::render::{Color, NamedColor, Renderer};
use vagus::rtx::{Camera, Triangle};
use vagus::scene::SceneManager;
use vagus::scene::default::{GameObject3D, GameScene3D, Mesh3D, Transform3D};
use vagus::texture::Texture;
use vagus::window::Window;

fn point3(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3 { x, y, z }
}

#[test]
fn renderer_draws_only_the_selected_3d_scene_from_scene_manager() {
    let mut window = Window::create_window("vagus-selected-3d-scene-render-test", 32, 32).unwrap();
    let mut renderer = Renderer::new(&mut window).expect("initialize Vulkan renderer");
    let mut manager = SceneManager::<GameScene3D, Vec3>::with_capacity(2).unwrap();
    let camera = Camera::new(
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 0.0],
        90.0,
        0.01,
        10_000.0,
    )
    .unwrap();

    assert!(
        renderer
            .draw_current_scene_3d(&manager, &camera, NamedColor::White, [0.0, 0.0, 1.0], 1.0)
            .is_err()
    );

    manager
        .add_scene(GameScene3D::with_capacity(0).unwrap())
        .unwrap();
    let mut scene = GameScene3D::with_capacity(1).unwrap();
    let texture = Texture::from_rgba8(1, 1, &[255, 255, 255, 255]).unwrap();
    let material = Material::new(texture, HeightMap::new(1, 1, vec![0.0]).unwrap());
    let triangle = Triangle::new_with_uvs(
        [[-0.5, -0.5, 0.0], [0.5, -0.5, 0.0], [0.0, 0.5, 0.0]],
        [[0.0, 0.0]; 3],
    )
    .unwrap();
    let mut object = GameObject3D::new(material, PhysicsObject3D::default());
    object.set_mesh(Mesh3D::new(vec![triangle]));
    object.set_transform(
        Transform3D::new(
            point3(0.0, 0.0, 2.0),
            point3(0.0, 0.0, 0.0),
            point3(1.0, 1.0, 1.0),
        )
        .unwrap(),
    );
    scene.objects_mut().push(object).unwrap();
    manager.add_scene(scene).unwrap();
    assert!(manager.select_scene(1));

    assert_eq!(
        renderer
            .draw_current_scene_3d(&manager, &camera, NamedColor::Cyan, [0.0, 0.0, 1.0], 1.0,)
            .unwrap(),
        1
    );
    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("read back selected 3D ECS scene");
    assert_eq!(pixels[16 * 32 + 16], [0, 255, 255, 255]);
    assert_eq!(pixels[0], [0, 0, 0, 255]);
}
