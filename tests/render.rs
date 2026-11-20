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

use vagus::heightmap::HeightMap;
use vagus::material::Material;
use vagus::math::{Vec2, Vec3};
use vagus::physics::hitbox::Hitbox2D;
use vagus::physics::{PhysicsObject2D, PhysicsObject3D};
use vagus::render::{Color, NamedColor, Renderer, RgbaColor};
use vagus::rtx::{Camera, Scene, Triangle};
use vagus::scene::SceneManager;
use vagus::scene::default::{
    GameObject2D, GameObject3D, GameScene2D, GameScene3D, Mesh3D, Transform3D,
};
use vagus::shader::{Shader, ShaderStage};
use vagus::texture::Texture;
use vagus::window::Window as NativeWindow;

use std::ops::{Deref, DerefMut};
use std::sync::{Mutex, MutexGuard};

static GPU_TEST_LOCK: Mutex<()> = Mutex::new(());

struct Window {
    _gpu_test_lock: MutexGuard<'static, ()>,
    native: NativeWindow,
}

impl Window {
    fn create_window(title: &str, width: u32, height: u32) -> Result<Self, String> {
        let gpu_test_lock = GPU_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let native = NativeWindow::create_window(title, width, height)?;
        Ok(Self {
            _gpu_test_lock: gpu_test_lock,
            native,
        })
    }
}

impl Deref for Window {
    type Target = NativeWindow;

    fn deref(&self) -> &Self::Target {
        &self.native
    }
}

impl DerefMut for Window {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.native
    }
}

fn point(x: f32, y: f32) -> Vec2 {
    Vec2 { x, y }
}

fn point3(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3 { x, y, z }
}

#[test]
fn ecs_2d_scene_queues_cached_textures_at_physics_bounds() {
    let mut window = Window::create_window("vagus-ecs-scene-render-test", 32, 32)
        .expect("create a native window for ECS scene rendering");
    let mut renderer =
        Renderer::new(&mut window).expect("initialize Vulkan for ECS scene rendering");
    let sprite_textures = [
        Texture::from_rgba8(1, 1, &[255, 0, 0, 255]).expect("create red sprite"),
        Texture::from_rgba8(1, 1, &[255, 0, 0, 255]).expect("create equal red sprite"),
    ];
    let mut scene = GameScene2D::with_capacity(2).expect("create a 2D ECS scene");

    for (x, texture) in [4.0, 20.0].into_iter().zip(sprite_textures) {
        let hitbox = Hitbox2D::rectangular(
            point(x, 4.0),
            point(x + 8.0, 4.0),
            point(x + 8.0, 12.0),
            point(x, 12.0),
        );
        let physics = PhysicsObject2D::new(hitbox, 0.0, 0.0, 0.0, point(0.0, 0.0));
        scene
            .objects_mut()
            .push(GameObject2D::new(texture, physics))
            .expect("insert sprite in the ECS scene");
    }

    assert_eq!(
        renderer
            .draw_game_scene_2d(&scene)
            .expect("queue ECS sprites on the renderer device"),
        2
    );
    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("render and read back the ECS scene");

    assert_eq!(pixels[8 * 32 + 8], [255, 0, 0, 255]);
    assert_eq!(pixels[8 * 32 + 24], [255, 0, 0, 255]);
    assert_eq!(pixels[16 * 32 + 16], [0, 0, 0, 255]);
}

#[test]
fn ecs_2d_scene_batches_repeated_sprites_into_one_instanced_draw() {
    let mut window = Window::create_window("vagus-ecs-2d-instanced-test", 32, 32)
        .expect("create a native window for 2D sprite instancing");
    let mut renderer = Renderer::new(&mut window).expect("initialize Vulkan for sprite instancing");
    let texture = Texture::from_rgba8(1, 1, &[255, 0, 0, 255]).expect("create sprite texture");
    let mut scene = GameScene2D::with_capacity(4).expect("create a four-sprite ECS scene");

    for x in [2.0, 10.0, 18.0, 26.0] {
        let hitbox = Hitbox2D::rectangular(
            point(x, 4.0),
            point(x + 4.0, 4.0),
            point(x + 4.0, 8.0),
            point(x, 8.0),
        );
        let physics = PhysicsObject2D::new(hitbox, 0.0, 0.0, 0.0, point(0.0, 0.0));
        scene
            .objects_mut()
            .push(GameObject2D::new(texture.clone(), physics))
            .expect("insert sprite into the ECS scene");
    }

    assert_eq!(renderer.draw_game_scene_2d(&scene).unwrap(), 4);
    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("render and read back the instanced 2D scene");

    for x in [4, 12, 20, 28] {
        assert_eq!(pixels[6 * 32 + x], [255, 0, 0, 255]);
    }
    assert_eq!(renderer.last_frame_instanced_draw_count(), 1);
}

#[test]
fn ecs_2d_scene_batches_only_contiguous_repeated_sprites_preserving_painter_order() {
    let mut window = Window::create_window("vagus-ecs-2d-painter-order-test", 32, 32)
        .expect("create a native window for 2D sprite painter order");
    let mut renderer =
        Renderer::new(&mut window).expect("initialize Vulkan for sprite painter order");
    let red = Texture::from_rgba8(1, 1, &[255, 0, 0, 255]).expect("create red sprite texture");
    let green = Texture::from_rgba8(1, 1, &[0, 255, 0, 255]).expect("create green sprite texture");
    let mut scene = GameScene2D::with_capacity(4).expect("create a four-sprite ECS scene");

    for (texture, x) in [
        (red.clone(), 2.0),
        (red.clone(), 6.0),
        (green, 8.0),
        (red, 12.0),
    ] {
        let hitbox = Hitbox2D::rectangular(
            point(x, 4.0),
            point(x + 8.0, 4.0),
            point(x + 8.0, 12.0),
            point(x, 12.0),
        );
        let physics = PhysicsObject2D::new(hitbox, 0.0, 0.0, 0.0, point(0.0, 0.0));
        scene
            .objects_mut()
            .push(GameObject2D::new(texture, physics))
            .expect("insert sprite into the ECS scene");
    }

    assert_eq!(renderer.draw_game_scene_2d(&scene).unwrap(), 4);
    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("render and read back the interleaved sprite scene");

    assert_eq!(pixels[6 * 32 + 4], [255, 0, 0, 255]);
    assert_eq!(pixels[6 * 32 + 9], [0, 255, 0, 255]);
    assert_eq!(pixels[6 * 32 + 13], [255, 0, 0, 255]);
    assert_eq!(pixels[6 * 32 + 24], [0, 0, 0, 255]);
    assert_eq!(renderer.last_frame_instanced_draw_count(), 1);
}

#[test]
fn renderer_draws_only_the_selected_2d_scene_from_scene_manager() {
    let mut window = Window::create_window("vagus-selected-scene-render-test", 32, 32)
        .expect("create a native window for selected scene rendering");
    let mut renderer =
        Renderer::new(&mut window).expect("initialize Vulkan for selected scene rendering");
    let mut manager =
        SceneManager::<GameScene2D, Vec2>::with_capacity(2).expect("create a scene manager");

    assert!(renderer.draw_current_scene_2d(&manager).is_err());

    for (x, color) in [(4.0, [255, 0, 0, 255]), (20.0, [0, 255, 0, 255])] {
        let texture = Texture::from_rgba8(1, 1, &color).expect("create scene sprite texture");
        let hitbox = Hitbox2D::rectangular(
            point(x, 4.0),
            point(x + 8.0, 4.0),
            point(x + 8.0, 12.0),
            point(x, 12.0),
        );
        let physics = PhysicsObject2D::new(hitbox, 0.0, 0.0, 0.0, point(0.0, 0.0));
        let mut scene = GameScene2D::with_capacity(1).expect("create a 2D scene");
        scene
            .objects_mut()
            .push(GameObject2D::new(texture, physics))
            .expect("insert sprite into the scene ECS");
        manager.add_scene(scene).expect("register the scene");
    }

    assert!(manager.select_scene(1));
    assert_eq!(
        renderer
            .draw_current_scene_2d(&manager)
            .expect("draw the selected scene"),
        1
    );
    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("read back the selected scene");

    assert_eq!(pixels[8 * 32 + 8], [0, 0, 0, 255]);
    assert_eq!(pixels[8 * 32 + 24], [0, 255, 0, 255]);
    assert_eq!(pixels[16 * 32 + 16], [0, 0, 0, 255]);
}

#[test]
fn ecs_3d_scene_transforms_meshes_and_draws_them_on_gpu() {
    let mut window = Window::create_window("vagus-ecs-3d-scene-render-test", 32, 32)
        .expect("create a native window for ECS 3D scene rendering");
    let mut renderer =
        Renderer::new(&mut window).expect("initialize Vulkan for ECS 3D scene rendering");
    let mut scene = GameScene3D::with_capacity(1).expect("create a 3D ECS scene");
    let triangle = Triangle::new_with_uvs(
        [[-0.5, -0.5, 0.0], [0.5, -0.5, 0.0], [0.0, 0.5, 0.0]],
        [[0.0, 1.0]; 3],
    )
    .expect("create textured local-space triangle");
    let material_texture = Texture::from_rgba8(
        2,
        2,
        &[
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ],
    )
    .expect("create material texture with distinct texels");
    let material = Material::new(
        material_texture,
        HeightMap::new(1, 1, vec![0.0]).expect("create flat material heightmap"),
    );
    let mut object = GameObject3D::new(material, PhysicsObject3D::default());
    object.set_mesh(Mesh3D::new(vec![triangle]));
    object.set_transform(
        Transform3D::new(
            point3(0.0, 0.0, 2.0),
            point3(0.0, 0.0, 0.0),
            point3(1.0, 1.0, 1.0),
        )
        .expect("create object transform"),
    );
    scene
        .objects_mut()
        .push(object)
        .expect("insert mesh in ECS");
    let camera = Camera::new(
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 0.0],
        90.0,
        0.01,
        10_000.0,
    )
    .expect("create scene camera");

    assert_eq!(
        renderer
            .draw_game_scene_3d(&scene, &camera, NamedColor::White, [0.0, 0.0, 1.0], 1.0)
            .expect("queue ECS mesh through the GPU scene path"),
        1
    );
    renderer.clear(Color::rgb(0, 0, 0));
    assert_eq!(
        renderer
            .draw_game_scene_3d(&scene, &camera, NamedColor::White, [0.0, 0.0, 1.0], 1.0)
            .expect("queue the scene again using retained staging storage"),
        1
    );
    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("render and read back the ECS 3D scene");
    assert_eq!(pixels[16 * 32 + 16], [0, 0, 255, 255]);
    assert_eq!(pixels[0], [0, 0, 0, 255]);
}

#[test]
fn ecs_3d_scene_culls_large_offscreen_mesh_bounds_without_hiding_visible_meshes() {
    let mut window = Window::create_window("vagus-ecs-3d-bounds-cull-test", 32, 32)
        .expect("create a native window for ECS mesh bounds culling");
    let mut renderer =
        Renderer::new(&mut window).expect("initialize Vulkan for mesh bounds culling");
    let mut scene = GameScene3D::with_capacity(2).expect("create a two-object 3D ECS scene");
    let camera = Camera::new(
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 0.0],
        90.0,
        0.1,
        10.0,
    )
    .expect("create the ECS scene camera");
    let texture = || Texture::from_rgba8(1, 1, &[255, 0, 0, 255]).unwrap();
    let material = |texture| {
        Material::new(
            texture,
            HeightMap::new(1, 1, vec![0.0]).expect("create a flat heightmap"),
        )
    };
    let triangle = || {
        Triangle::new_with_uvs(
            [[-0.5, -0.5, 0.0], [0.5, -0.5, 0.0], [0.0, 0.5, 0.0]],
            [[0.0, 0.0]; 3],
        )
        .expect("create a local-space triangle")
    };

    let mut offscreen = GameObject3D::new(material(texture()), PhysicsObject3D::default());
    offscreen.set_mesh(Mesh3D::new((0..8).map(|_| triangle()).collect()));
    offscreen.set_transform(
        Transform3D::new(
            point3(20.0, 0.0, 2.0),
            point3(0.0, 0.0, 0.0),
            point3(1.0, 1.0, 1.0),
        )
        .expect("create the offscreen object transform"),
    );
    scene.objects_mut().push(offscreen).unwrap();

    let mut visible = GameObject3D::new(material(texture()), PhysicsObject3D::default());
    visible.set_mesh(Mesh3D::new(vec![triangle()]));
    visible.set_transform(
        Transform3D::new(
            point3(0.0, 0.0, 2.0),
            point3(0.0, 0.0, 0.0),
            point3(1.0, 1.0, 1.0),
        )
        .expect("create the visible object transform"),
    );
    scene.objects_mut().push(visible).unwrap();

    assert_eq!(
        renderer
            .draw_game_scene_3d(&scene, &camera, NamedColor::White, [0.0, 0.0, 1.0], 1.0)
            .expect("queue visible ECS geometry"),
        1
    );
    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("read back the culled ECS scene");
    assert_eq!(pixels[16 * 32 + 16], [255, 0, 0, 255]);
    assert_eq!(pixels[0], [0, 0, 0, 255]);
}

#[test]
fn ecs_3d_instancing_culls_offscreen_transforms_by_mesh_bounds() {
    let mut window = Window::create_window("vagus-ecs-3d-instanced-bounds-cull-test", 32, 32)
        .expect("create a native window for instanced mesh bounds culling");
    let mut renderer =
        Renderer::new(&mut window).expect("initialize Vulkan for instanced mesh bounds culling");
    let mut scene = GameScene3D::with_capacity(2).expect("create a two-object 3D ECS scene");
    let triangles = (0..8)
        .map(|_| {
            Triangle::new_with_uvs(
                [[-0.5, -0.5, 0.0], [0.5, -0.5, 0.0], [0.0, 0.5, 0.0]],
                [[0.0, 0.0]; 3],
            )
            .expect("create shared instanced geometry")
        })
        .collect();
    let mesh = Mesh3D::new(triangles);
    let texture =
        Texture::from_rgba8(1, 1, &[0, 255, 0, 255]).expect("create an opaque green texture");

    for x in [0.0, 20.0] {
        let material = Material::new(
            texture.clone(),
            HeightMap::new(1, 1, vec![0.0]).expect("create a flat heightmap"),
        );
        let mut object = GameObject3D::new(material, PhysicsObject3D::default());
        object.set_mesh(mesh.clone());
        object.set_transform(
            Transform3D::new(
                point3(x, 0.0, 2.0),
                point3(0.0, 0.0, 0.0),
                point3(1.0, 1.0, 1.0),
            )
            .expect("create an instanced object transform"),
        );
        scene.objects_mut().push(object).unwrap();
    }

    let camera = Camera::new(
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 0.0],
        90.0,
        0.1,
        10.0,
    )
    .expect("create the instanced scene camera");
    assert_eq!(
        renderer
            .draw_game_scene_3d(&scene, &camera, NamedColor::White, [0.0, 0.0, 1.0], 1.0)
            .expect("queue the visible instance"),
        8
    );
    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("read back the instanced scene");
    assert_eq!(pixels[16 * 32 + 16], [0, 255, 0, 255]);
    assert_eq!(pixels[0], [0, 0, 0, 255]);
}

#[test]
fn ecs_3d_scene_renders_shared_mesh_geometry_at_multiple_transforms() {
    let mut window = Window::create_window("vagus-ecs-3d-instancing-contract-test", 32, 32)
        .expect("create a native window for multiple ECS mesh transforms");
    let mut renderer =
        Renderer::new(&mut window).expect("initialize Vulkan for multiple ECS mesh transforms");
    let mut scene = GameScene3D::with_capacity(2).expect("create a two-object 3D ECS scene");

    for x in [-0.5, 0.5] {
        let texture =
            Texture::from_rgba8(1, 1, &[255, 0, 0, 255]).expect("create an opaque red texture");
        let material = Material::new(
            texture,
            HeightMap::new(1, 1, vec![0.0]).expect("create a flat material heightmap"),
        );
        let triangle = Triangle::new_with_uvs(
            [[-0.3, -0.3, 0.0], [0.3, -0.3, 0.0], [0.0, 0.3, 0.0]],
            [[0.0, 0.0]; 3],
        )
        .expect("create shared local-space triangle geometry");
        let mut object = GameObject3D::new(material, PhysicsObject3D::default());
        object.set_mesh(Mesh3D::new(vec![triangle]));
        object.set_transform(
            Transform3D::new(
                point3(x, 0.0, 2.0),
                point3(0.0, 0.0, 0.0),
                point3(1.0, 1.0, 1.0),
            )
            .expect("create object transform"),
        );
        scene
            .objects_mut()
            .push(object)
            .expect("insert mesh in ECS");
    }
    let camera = Camera::new(
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 0.0],
        90.0,
        0.01,
        10_000.0,
    )
    .expect("create scene camera");

    assert_eq!(
        renderer
            .draw_game_scene_3d(&scene, &camera, NamedColor::White, [0.0, 0.0, 1.0], 1.0)
            .expect("queue both ECS mesh instances"),
        2
    );
    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("render and read back both mesh transforms");

    assert_eq!(pixels[16 * 32 + 12], [255, 0, 0, 255]);
    assert_eq!(pixels[16 * 32 + 20], [255, 0, 0, 255]);
    assert_eq!(pixels[16 * 32 + 16], [0, 0, 0, 255]);
    assert_eq!(renderer.last_frame_instanced_draw_count(), 1);
}

#[test]
fn ecs_3d_scene_instanced_shared_multi_triangle_mesh_on_gpu() {
    let mut window = Window::create_window("vagus-ecs-3d-multi-triangle-instancing-test", 32, 32)
        .expect("create a native window for multi-triangle mesh instancing");
    let mut renderer =
        Renderer::new(&mut window).expect("initialize Vulkan for multi-triangle mesh instancing");
    let mut scene = GameScene3D::with_capacity(2).expect("create a two-object 3D ECS scene");
    let triangles = vec![
        Triangle::new_with_uvs(
            [[-0.3, -0.3, 0.0], [0.3, -0.3, 0.0], [0.3, 0.3, 0.0]],
            [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
        )
        .expect("create first local-space triangle"),
        Triangle::new_with_uvs(
            [[-0.3, -0.3, 0.0], [0.3, 0.3, 0.0], [-0.3, 0.3, 0.0]],
            [[0.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        )
        .expect("create second local-space triangle"),
    ];

    for x in [-0.5, 0.5] {
        let texture =
            Texture::from_rgba8(1, 1, &[255, 0, 0, 255]).expect("create an opaque red texture");
        let material = Material::new(
            texture,
            HeightMap::new(1, 1, vec![0.0]).expect("create a flat material heightmap"),
        );
        let mut object = GameObject3D::new(material, PhysicsObject3D::default());
        object.set_mesh(Mesh3D::new(triangles.clone()));
        object.set_transform(
            Transform3D::new(
                point3(x, 0.0, 2.0),
                point3(0.0, 0.0, 0.0),
                point3(1.0, 1.0, 1.0),
            )
            .expect("create object transform"),
        );
        scene
            .objects_mut()
            .push(object)
            .expect("insert multi-triangle mesh in ECS");
    }
    let camera = Camera::new(
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 0.0],
        90.0,
        0.01,
        10_000.0,
    )
    .expect("create scene camera");

    assert_eq!(
        renderer
            .draw_game_scene_3d(&scene, &camera, NamedColor::White, [0.0, 0.0, 1.0], 1.0)
            .expect("queue both multi-triangle mesh instances"),
        4
    );
    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("render and read back both multi-triangle transforms");

    assert_eq!(pixels[16 * 32 + 12], [255, 0, 0, 255]);
    assert_eq!(pixels[16 * 32 + 20], [255, 0, 0, 255]);
    assert_eq!(pixels[16 * 32 + 16], [0, 0, 0, 255]);
    assert_eq!(renderer.last_frame_instanced_draw_count(), 1);

    assert_eq!(
        renderer
            .draw_game_scene_3d(&scene, &camera, NamedColor::White, [0.0, 0.0, 1.0], 1.0)
            .expect("queue the same instanced scene again"),
        4
    );
    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("render the repeated instanced scene using recycled mesh buffers");
    assert_eq!(pixels[16 * 32 + 12], [255, 0, 0, 255]);
    assert_eq!(pixels[16 * 32 + 20], [255, 0, 0, 255]);
    assert_eq!(pixels[16 * 32 + 16], [0, 0, 0, 255]);
    assert_eq!(renderer.last_frame_instanced_draw_count(), 1);

    let texture = Texture::from_rgba8(1, 1, &[255, 0, 0, 255]).expect("create opaque red texture");
    let material = Material::new(
        texture,
        HeightMap::new(1, 1, vec![0.0]).expect("create a flat material heightmap"),
    );
    let mut object = GameObject3D::new(material, PhysicsObject3D::default());
    object.set_mesh(Mesh3D::new(triangles));
    object.set_transform(
        Transform3D::new(
            point3(0.5, 0.0, 2.0),
            point3(0.0, 0.0, 0.0),
            point3(1.0, 1.0, 1.0),
        )
        .expect("create single-object transform"),
    );
    let mut smaller_scene = GameScene3D::with_capacity(1).expect("create a one-object 3D scene");
    smaller_scene
        .objects_mut()
        .push(object)
        .expect("insert the single mesh into ECS");
    assert_eq!(
        renderer
            .draw_game_scene_3d(&smaller_scene, &camera, NamedColor::White, [0.0, 0.0, 1.0], 1.0)
            .expect("queue the shorter scene after the instanced scene"),
        2
    );
    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("render the shorter scene without stale instance data");

    assert_eq!(pixels[16 * 32 + 12], [0, 0, 0, 255]);
    assert_eq!(pixels[16 * 32 + 20], [255, 0, 0, 255]);
    assert_eq!(renderer.last_frame_instanced_draw_count(), 0);
}

#[test]
fn ecs_3d_material_texture_alpha_blends_with_gpu_scene() {
    let mut window = Window::create_window("vagus-ecs-3d-alpha-render-test", 32, 32)
        .expect("create a native window for ECS 3D texture blending");
    let mut renderer =
        Renderer::new(&mut window).expect("initialize Vulkan for ECS 3D texture blending");
    let texture =
        Texture::from_rgba8(1, 1, &[255, 0, 0, 128]).expect("create a half-alpha material texture");
    let material = Material::new(
        texture,
        HeightMap::new(1, 1, vec![0.0]).expect("create a flat material heightmap"),
    );
    let triangle = Triangle::new_with_uvs(
        [[-0.5, -0.5, 0.0], [0.5, -0.5, 0.0], [0.0, 0.5, 0.0]],
        [[0.0, 0.0]; 3],
    )
    .expect("create a textured local-space triangle");
    let mut object = GameObject3D::new(material, PhysicsObject3D::default());
    object.set_mesh(Mesh3D::new(vec![triangle]));
    object.set_transform(
        Transform3D::new(
            point3(0.0, 0.0, 2.0),
            point3(0.0, 0.0, 0.0),
            point3(1.0, 1.0, 1.0),
        )
        .expect("create object transform"),
    );
    let mut scene = GameScene3D::with_capacity(1).expect("create a 3D ECS scene");
    scene
        .objects_mut()
        .push(object)
        .expect("insert textured mesh in ECS");
    let camera = Camera::new(
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 0.0],
        90.0,
        0.01,
        10_000.0,
    )
    .expect("create scene camera");

    assert_eq!(
        renderer
            .draw_game_scene_3d(&scene, &camera, NamedColor::White, [0.0, 0.0, 1.0], 1.0)
            .expect("queue the ECS mesh through the GPU scene path"),
        1
    );
    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 255))
        .expect("render and read back the blended ECS 3D material");
    let blended = pixels[16 * 32 + 16];

    assert!(
        (120..=190).contains(&blended[0]),
        "unexpected red channel: {blended:?}"
    );
    assert_eq!(blended[1], 0);
    assert!(
        (120..=190).contains(&blended[2]),
        "unexpected blue channel: {blended:?}"
    );
    assert_eq!(blended[3], 255);
    assert_eq!(pixels[0], [0, 0, 255, 255]);
    assert_eq!(renderer.last_frame_instanced_draw_count(), 0);
}

#[test]
fn ecs_3d_transparent_meshes_blend_back_to_front_independent_of_entity_order() {
    let mut window = Window::create_window("vagus-ecs-3d-transparent-order-test", 32, 32)
        .expect("create a native window for transparent ECS mesh ordering");
    let mut renderer =
        Renderer::new(&mut window).expect("initialize Vulkan for transparent ECS mesh ordering");
    let mut scene = GameScene3D::with_capacity(2).expect("create a two-object 3D ECS scene");
    let triangle = Triangle::new_with_uvs(
        [[-0.5, -0.5, 0.0], [0.5, -0.5, 0.0], [0.0, 0.5, 0.0]],
        [[0.0, 0.0]; 3],
    )
    .expect("create transparent local-space triangle geometry");

    for (depth, rgba) in [(2.0, [255, 0, 0, 128]), (4.0, [0, 0, 255, 128])] {
        let texture =
            Texture::from_rgba8(1, 1, &rgba).expect("create transparent material texture");
        let material = Material::new(
            texture,
            HeightMap::new(1, 1, vec![0.0]).expect("create a flat material heightmap"),
        );
        let mut object = GameObject3D::new(material, PhysicsObject3D::default());
        object.set_mesh(Mesh3D::new(vec![triangle.clone()]));
        object.set_transform(
            Transform3D::new(
                point3(0.0, 0.0, depth),
                point3(0.0, 0.0, 0.0),
                point3(1.0, 1.0, 1.0),
            )
            .expect("create transparent mesh transform"),
        );
        scene
            .objects_mut()
            .push(object)
            .expect("insert transparent mesh in ECS");
    }
    let camera = Camera::new(
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 0.0],
        90.0,
        0.01,
        10_000.0,
    )
    .expect("create scene camera");

    assert_eq!(
        renderer
            .draw_game_scene_3d(&scene, &camera, NamedColor::White, [0.0, 0.0, 1.0], 1.0)
            .expect("queue transparent ECS meshes"),
        2
    );
    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("render and read back depth-sorted transparent ECS meshes");
    let overlap = pixels[16 * 32 + 16];

    assert_eq!(overlap[1], 0);
    assert!(
        overlap[0] > overlap[2],
        "near red mesh should blend over the far blue mesh: {overlap:?}"
    );
    assert!(
        overlap[2] > 80,
        "far blue mesh should remain visible beneath near red: {overlap:?}"
    );
    assert_eq!(overlap[3], 255);
    assert_eq!(pixels[0], [0, 0, 0, 255]);
    assert_eq!(renderer.last_frame_instanced_draw_count(), 0);
}

fn minimal_vertex_spirv() -> [u32; 29] {
    [
        0x0723_0203,
        0x0001_0000,
        0,
        5,
        0,
        0x0002_0011,
        1,
        0x0003_000e,
        0,
        1,
        0x0005_000f,
        0,
        1,
        0x6e69_616d,
        0,
        0x0002_0013,
        2,
        0x0003_0021,
        3,
        2,
        0x0005_0036,
        2,
        1,
        0,
        3,
        0x0002_00f8,
        4,
        0x0001_00fd,
        0x0001_0038,
    ]
}

#[test]
fn public_renderer_draws_and_presents_a_window_frame() {
    let mut window = Window::create_window("vagus-render-test", 64, 64)
        .expect("create a native window for renderer integration");
    let shader_module;
    let mut ray_tracer = None;
    let triangle = Triangle::new([[-0.6, -0.6, 1.0], [0.6, -0.6, 1.0], [0.0, 0.6, 1.0]]).unwrap();
    let scene = Scene::new(vec![triangle]).unwrap();
    let camera = Camera::new(
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 0.0],
        60.0,
        0.01,
        10_000.0,
    )
    .unwrap();
    {
        let mut renderer = Renderer::new(&mut window).expect("initialize Vulkan for the window");
        let shader =
            Shader::from_spirv(ShaderStage::Vertex, &minimal_vertex_spirv(), "main").unwrap();
        shader_module = renderer
            .shader_device()
            .load(&shader)
            .expect("load shader module on the renderer's Vulkan device");
        assert_eq!(shader_module.stage(), ShaderStage::Vertex);
        assert_eq!(shader_module.entry_point(), "main");

        renderer.clear(NamedColor::Black);
        renderer.draw_dot(point(1.0, 1.0), NamedColor::White);
        renderer.draw_line(point(2.0, 2.0), point(12.0, 2.0), NamedColor::Red);
        renderer.draw_triangle(
            point(10.0, 10.0),
            point(20.0, 10.0),
            point(15.0, 20.0),
            NamedColor::Green,
        );
        renderer.draw_triangle_gpu(
            point(34.0, 10.0),
            point(46.0, 10.0),
            point(40.0, 22.0),
            NamedColor::Magenta,
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

        if renderer.ray_tracing_supported() {
            ray_tracer = Some(
                renderer
                    .rtx_renderer(8, 8)
                    .expect("create RTX renderer from the presentation device"),
            );
        } else {
            let error = match renderer.rtx_renderer(8, 8) {
                Ok(_) => panic!("unsupported shared-device RTX unexpectedly initialized"),
                Err(error) => error,
            };
            assert!(error.contains("presentation device was created without ray-tracing support"));
        }
    };

    if let Some(mut ray_tracer) = ray_tracer {
        let frame = ray_tracer
            .render(&scene, &camera)
            .expect("trace a triangle after the presentation renderer was dropped");
        assert_eq!(frame.pixel(4, 4), Some([230, 64, 31, 255]));
        assert_eq!(frame.pixel(0, 0), Some([5, 10, 31, 255]));
    }
    assert_eq!(shader_module.stage(), ShaderStage::Vertex);
    drop(shader_module);
}

#[test]
fn public_renderer_presents_a_gpu_only_frame() {
    let mut window = Window::create_window("vagus-gpu-only-render-test", 32, 32)
        .expect("create a native window for GPU-only rendering");
    let mut renderer = Renderer::new(&mut window).expect("initialize Vulkan for the window");
    renderer.draw_triangle_gpu(
        point(4.0, 4.0),
        point(28.0, 4.0),
        point(16.0, 28.0),
        NamedColor::Magenta,
    );
    renderer.draw_triangle_gpu(
        point(4.0, 28.0),
        point(16.0, 4.0),
        point(28.0, 28.0),
        NamedColor::Cyan,
    );
    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("render and read back a GPU-only frame");

    assert_eq!(pixels.len(), 32 * 32);
    assert_eq!(pixels[8 * 32 + 8], [255, 0, 255, 255]);
    assert_eq!(pixels[24 * 32 + 8], [0, 255, 255, 255]);
    assert_eq!(pixels[0], [0, 0, 0, 255]);
}

#[test]
fn gpu_triangle_vertex_storage_updates_between_frames() {
    let mut window = Window::create_window("vagus-gpu-frame-storage-test", 32, 32)
        .expect("create a native window for successive GPU frames");
    let mut renderer = Renderer::new(&mut window).expect("initialize Vulkan for successive frames");

    renderer.draw_triangle_gpu(
        point(4.0, 4.0),
        point(16.0, 4.0),
        point(10.0, 16.0),
        NamedColor::Red,
    );
    let first = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("read the first GPU frame");
    assert_eq!(first[8 * 32 + 10], [255, 0, 0, 255]);

    renderer.draw_triangle_gpu(
        point(18.0, 18.0),
        point(30.0, 18.0),
        point(24.0, 30.0),
        NamedColor::Green,
    );
    let second = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("read the second GPU frame");
    assert_eq!(second[24 * 32 + 24], [0, 255, 0, 255]);
    assert_eq!(second[8 * 32 + 10], [0, 0, 0, 255]);
    assert_eq!(second[0], [0, 0, 0, 255]);
}

#[test]
fn gpu_lines_render_horizontal_diagonal_and_degenerate_segments() {
    let mut window = Window::create_window("vagus-gpu-line-test", 32, 32)
        .expect("create a native window for GPU line rendering");
    let mut renderer = Renderer::new(&mut window).expect("initialize Vulkan for GPU lines");
    renderer.draw_line_gpu(point(4.0, 8.0), point(28.0, 8.0), NamedColor::Red);
    renderer.draw_line_gpu(point(4.0, 18.0), point(12.0, 26.0), NamedColor::Green);
    renderer.draw_line_gpu(point(25.0, 25.0), point(25.0, 25.0), NamedColor::Blue);
    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("render and read back GPU lines");

    assert_eq!(pixels[8 * 32 + 16], [255, 0, 0, 255]);
    assert_eq!(pixels[22 * 32 + 8], [0, 255, 0, 255]);
    assert_eq!(pixels[25 * 32 + 25], [0, 0, 255, 255]);
    assert_eq!(pixels[12 * 32 + 16], [0, 0, 0, 255]);
}

#[test]
fn gpu_rectangle_renders_as_a_batched_2d_primitive() {
    let mut window = Window::create_window("vagus-gpu-rectangle-test", 32, 32)
        .expect("create a native window for GPU rectangle rendering");
    let mut renderer = Renderer::new(&mut window).expect("initialize Vulkan for GPU rectangles");
    renderer.draw_rectangle_gpu(point(8.0, 8.0), 12, 10, NamedColor::Green);
    renderer.draw_rectangle_gpu(point(-4.0, -4.0), 8, 8, NamedColor::Blue);
    renderer.draw_rectangle_gpu(point(24.0, 24.0), 0, 8, NamedColor::Red);
    renderer.draw_rectangle_gpu(point(2_147_483_520.0, 8.0), 256, 8, NamedColor::Red);
    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("render and read back a GPU rectangle");

    assert_eq!(pixels[12 * 32 + 12], [0, 255, 0, 255]);
    assert_eq!(pixels[1 * 32 + 1], [0, 0, 255, 255]);
    assert_eq!(pixels[4 * 32 + 4], [0, 0, 0, 255]);
    assert_eq!(pixels[24 * 32 + 24], [0, 0, 0, 255]);
}

#[test]
fn default_rectangle_draw_uses_the_gpu_batch() {
    let mut window = Window::create_window("vagus-default-gpu-rectangle-test", 32, 32)
        .expect("create a native window for default GPU rectangle rendering");
    let mut renderer = Renderer::new(&mut window).expect("initialize Vulkan for GPU rectangles");
    renderer.draw_rectangle(point(8.0, 8.0), 12, 10, NamedColor::Green);
    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("render and read back a default rectangle");

    assert_eq!(pixels[12 * 32 + 12], [0, 255, 0, 255]);
    assert_eq!(pixels[4 * 32 + 4], [0, 0, 0, 255]);
}

#[test]
fn default_2d_shapes_share_gpu_painter_order() {
    let mut window = Window::create_window("vagus-default-gpu-order-test", 32, 32)
        .expect("create a native window for default GPU drawing");
    let mut renderer = Renderer::new(&mut window).expect("initialize Vulkan for GPU drawing");
    renderer.clear(NamedColor::Black);
    renderer.draw_rectangle(point(8.0, 8.0), 12, 12, NamedColor::Red);
    renderer.draw_triangle(
        point(8.0, 8.0),
        point(20.0, 8.0),
        point(14.0, 20.0),
        NamedColor::Blue,
    );
    let pixels = renderer
        .present_gpu_readback(NamedColor::Black)
        .expect("render and read back default 2D shapes");

    assert_eq!(pixels[12 * 32 + 14], [0, 0, 255, 255]);
    assert_eq!(pixels[16 * 32 + 9], [255, 0, 0, 255]);
    assert_eq!(pixels[4 * 32 + 4], [0, 0, 0, 255]);
}

#[test]
fn default_2d_primitives_render_on_the_gpu() {
    let mut window = Window::create_window("vagus-default-gpu-primitives-test", 64, 48)
        .expect("create a native window for default GPU primitives");
    let mut renderer = Renderer::new(&mut window).expect("initialize Vulkan for GPU primitives");
    renderer.clear(NamedColor::Black);
    renderer.draw_dot(point(2.0, 2.0), NamedColor::White);
    renderer.draw_line(point(4.0, 4.0), point(14.0, 4.0), NamedColor::Red);
    renderer.draw_triangle(
        point(20.0, 4.0),
        point(30.0, 4.0),
        point(25.0, 14.0),
        NamedColor::Blue,
    );
    renderer.draw_triangle_outline(
        point(34.0, 4.0),
        point(44.0, 4.0),
        point(39.0, 14.0),
        NamedColor::Cyan,
    );
    renderer.draw_rectangle(point(4.0, 20.0), 8, 8, NamedColor::Green);
    renderer.draw_rectangle_outline(point(14.0, 20.0), 8, 8, NamedColor::Orange);
    renderer.draw_square(point(24.0, 20.0), 8, NamedColor::Purple);
    renderer.draw_circle(point(38.0, 24.0), 5, NamedColor::Yellow);
    renderer.draw_circle_outline(point(52.0, 24.0), 5, NamedColor::Magenta);
    let pixels = renderer
        .present_gpu_readback(NamedColor::Black)
        .expect("render and read back default 2D primitives");

    assert_eq!(pixels[2 * 64 + 2], [255, 255, 255, 255]);
    assert_eq!(pixels[4 * 64 + 8], [255, 0, 0, 255]);
    assert_eq!(pixels[8 * 64 + 25], [0, 0, 255, 255]);
    assert_eq!(pixels[4 * 64 + 39], [0, 255, 255, 255]);
    assert_eq!(pixels[22 * 64 + 6], [0, 255, 0, 255]);
    assert_eq!(pixels[23 * 64 + 14], [255, 210, 0, 255]);
    assert_eq!(pixels[22 * 64 + 26], [188, 0, 188, 255]);
    assert_eq!(pixels[24 * 64 + 38], [255, 255, 0, 255]);
    assert_eq!(pixels[24 * 64 + 57], [255, 0, 255, 255]);
    assert_eq!(pixels[40 * 64 + 60], [0, 0, 0, 255]);
}

#[test]
fn gpu_rectangle_rgba_composites_over_existing_framebuffer_color() {
    let mut window = Window::create_window("vagus-gpu-rgba-rectangle-test", 32, 32)
        .expect("create a native window for RGBA primitive rendering");
    let mut renderer = Renderer::new(&mut window).expect("initialize Vulkan for RGBA primitives");
    renderer.draw_rectangle_gpu_rgba(point(8.0, 8.0), 12, 12, RgbaColor::rgba(255, 0, 0, 128));
    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 255))
        .expect("render and read back an RGBA primitive");
    let blended = pixels[12 * 32 + 12];

    assert!(
        (120..=190).contains(&blended[0]),
        "unexpected red channel: {blended:?}"
    );
    assert_eq!(blended[1], 0);
    assert!(
        (120..=190).contains(&blended[2]),
        "unexpected blue channel: {blended:?}"
    );
    assert_eq!(blended[3], 255);
    assert_eq!(pixels[20 * 32 + 20], [0, 0, 255, 255]);
}

#[test]
fn gpu_circle_renders_as_a_tessellated_2d_primitive() {
    let mut window = Window::create_window("vagus-gpu-circle-test", 32, 32)
        .expect("create a native window for GPU circle rendering");
    let mut renderer = Renderer::new(&mut window).expect("initialize Vulkan for GPU circles");
    renderer.draw_circle_gpu(point(16.0, 16.0), 8, NamedColor::Yellow);
    renderer.draw_circle_gpu(point(0.0, 0.0), 4, NamedColor::Blue);
    renderer.draw_circle_gpu(point(24.0, 24.0), 0, NamedColor::Red);
    renderer.draw_circle_gpu(point(0.0, 16.0), 2_147_483_648, NamedColor::Red);
    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("render and read back GPU circles");

    assert_eq!(pixels[16 * 32 + 16], [255, 255, 0, 255]);
    assert_eq!(pixels[16 * 32 + 21], [255, 255, 0, 255]);
    assert_eq!(pixels[16 * 32 + 26], [0, 0, 0, 255]);
    assert_eq!(pixels[1 * 32 + 1], [0, 0, 255, 255]);
    assert_eq!(pixels[4 * 32 + 4], [0, 0, 0, 255]);
    assert_eq!(pixels[16 * 32 + 1], [0, 0, 0, 255]);
    assert_eq!(pixels[24 * 32 + 24], [255, 0, 0, 255]);
}

#[test]
fn gpu_3d_triangles_use_depth_instead_of_submission_order() {
    let mut window = Window::create_window("vagus-gpu-depth-test", 32, 32)
        .expect("create a native window for GPU depth testing");
    let mut renderer = Renderer::new(&mut window).expect("initialize Vulkan for depth testing");
    let vertex = |x, y, z| Vec3 { x, y, z };
    assert!(
        renderer
            .draw_triangle_gpu_3d(
                vertex(4.0, 4.0, -0.1),
                vertex(28.0, 4.0, 0.2),
                vertex(16.0, 28.0, 0.2),
                NamedColor::Red,
            )
            .is_err()
    );
    renderer
        .draw_triangle_gpu_3d(
            vertex(4.0, 4.0, 0.2),
            vertex(28.0, 4.0, 0.2),
            vertex(16.0, 28.0, 0.2),
            NamedColor::Cyan,
        )
        .expect("queue near GPU triangle");
    renderer
        .draw_triangle_gpu_3d(
            vertex(4.0, 4.0, 0.8),
            vertex(28.0, 4.0, 0.8),
            vertex(16.0, 28.0, 0.8),
            NamedColor::Magenta,
        )
        .expect("queue far GPU triangle");
    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("render and read back depth-tested GPU triangles");

    assert_eq!(pixels[12 * 32 + 16], [0, 255, 255, 255]);
    assert_eq!(pixels[0], [0, 0, 0, 255]);
}

#[test]
fn direct_translucent_3d_triangle_sorts_with_transparent_scene() {
    let mut window = Window::create_window("vagus-mixed-transparent-3d-test", 32, 32)
        .expect("create a native window for mixed transparent 3D sorting");
    let mut renderer =
        Renderer::new(&mut window).expect("initialize Vulkan for mixed transparent 3D sorting");
    let vertex = |x, y, z| Vec3 { x, y, z };
    renderer
        .draw_triangle_gpu_3d_rgba(
            vertex(4.0, 4.0, 0.2),
            vertex(28.0, 4.0, 0.2),
            vertex(16.0, 28.0, 0.2),
            RgbaColor::rgba(255, 0, 0, 128),
        )
        .expect("queue near translucent direct triangle");

    let far_scene = Scene::new(vec![
        Triangle::new([[-1.5, -1.5, 4.0], [1.5, -1.5, 4.0], [0.0, 1.5, 4.0]]).unwrap(),
    ])
    .unwrap();
    let camera = Camera::new(
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 0.0],
        90.0,
        0.01,
        10_000.0,
    )
    .unwrap();
    renderer
        .draw_scene_gpu_rgba(
            &far_scene,
            &camera,
            RgbaColor::rgba(0, 0, 255, 128),
            [0.0, 0.0, 1.0],
            1.0,
        )
        .expect("queue far translucent scene triangle");
    renderer
        .draw_triangle_gpu_3d(
            vertex(2.0, 2.0, 0.95),
            vertex(30.0, 2.0, 0.95),
            vertex(16.0, 30.0, 0.95),
            NamedColor::White,
        )
        .expect("queue opaque background triangle");
    renderer.draw_rectangle_gpu_rgba(point(0.0, 0.0), 2, 2, RgbaColor::rgba(0, 255, 0, 255));

    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("read back mixed transparent 3D submissions");
    let overlap = pixels[16 * 32 + 16];
    assert!(
        overlap[0] > overlap[2],
        "near red should blend over far blue: {overlap:?}"
    );
    assert!(
        overlap[2] > 32,
        "far scene blue should remain visible: {overlap:?}"
    );
    assert_eq!(pixels[0], [0, 255, 0, 255], "2D overlay remains on top");
}

#[test]
fn world_space_scene_projects_depth_tests_and_lights_on_gpu() {
    let mut window = Window::create_window("vagus-camera-scene-test", 32, 32)
        .expect("create a native window for camera projection");
    let mut renderer = Renderer::new(&mut window).expect("initialize Vulkan for camera projection");
    let near_scene = Scene::new(vec![
        Triangle::new([[-0.75, -0.75, 2.0], [0.75, -0.75, 2.0], [0.0, 0.75, 2.0]]).unwrap(),
    ])
    .unwrap();
    let far_scene = Scene::new(vec![
        Triangle::new([[-1.5, -1.5, 4.0], [1.5, -1.5, 4.0], [0.0, 1.5, 4.0]]).unwrap(),
    ])
    .unwrap();
    let camera = Camera::new(
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 0.0],
        90.0,
        0.01,
        10_000.0,
    )
    .unwrap();

    assert_eq!(
        renderer
            .draw_scene_gpu(&near_scene, &camera, NamedColor::Cyan, [0.0, 0.0, 1.0], 1.0)
            .unwrap(),
        1
    );
    assert_eq!(
        renderer
            .draw_scene_gpu(
                &far_scene,
                &camera,
                NamedColor::Magenta,
                [0.0, 0.0, 1.0],
                1.0
            )
            .unwrap(),
        1
    );
    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("present projected world-space scene");
    assert_eq!(pixels[16 * 32 + 16], [0, 255, 255, 255]);
    assert_eq!(pixels[0], [0, 0, 0, 255]);

    let bright_scene = Scene::new(vec![
        Triangle::new([[-1.7, -0.6, 2.0], [-0.5, -0.6, 2.0], [-1.1, 0.6, 2.0]]).unwrap(),
    ])
    .unwrap();
    let dark_scene = Scene::new(vec![
        Triangle::new([[0.5, -0.6, 2.0], [1.7, -0.6, 2.0], [1.1, 0.6, 2.0]]).unwrap(),
    ])
    .unwrap();
    renderer
        .draw_scene_gpu(
            &bright_scene,
            &camera,
            Color::rgb(220, 80, 40),
            [0.0, 0.0, 1.0],
            0.2,
        )
        .expect("queue a scene facing the directional light");
    renderer
        .draw_scene_gpu(
            &dark_scene,
            &camera,
            Color::rgb(220, 80, 40),
            [0.0, 0.0, -1.0],
            0.2,
        )
        .expect("queue a scene facing away from the directional light");
    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("render and read back lit GPU scenes");
    let lit = pixels[16 * 32 + 7];
    let shaded = pixels[16 * 32 + 24];
    assert!(
        lit[0] > shaded[0] + 50,
        "expected directional shading: {lit:?} vs {shaded:?}"
    );
}

#[test]
fn translated_rotated_camera_projects_world_space_vertices_on_gpu() {
    let mut window = Window::create_window("vagus-rotated-camera-test", 32, 32)
        .expect("create a native window for rotated camera projection");
    let mut renderer =
        Renderer::new(&mut window).expect("initialize Vulkan for rotated camera projection");
    let scene = Scene::new(vec![
        Triangle::new([
            [12.0, -0.75, -4.75],
            [12.0, -0.75, -3.25],
            [12.0, 0.75, -4.0],
        ])
        .unwrap(),
    ])
    .unwrap();
    let camera = Camera::new(
        [10.0, 0.0, -4.0],
        [11.0, 0.0, -4.0],
        [0.0, 1.0, 0.0],
        90.0,
        0.01,
        10_000.0,
    )
    .unwrap();

    assert_eq!(
        renderer
            .draw_scene_gpu(&scene, &camera, NamedColor::Cyan, [0.0, 0.0, 1.0], 1.0)
            .unwrap(),
        1
    );
    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("present rotated-camera world-space scene");

    assert_eq!(pixels[16 * 32 + 16], [0, 255, 255, 255]);
    assert_eq!(pixels[0], [0, 0, 0, 255]);
}

#[test]
fn world_geometry_draws_over_texture_background() {
    let mut window = Window::create_window("vagus-world-texture-depth-test", 32, 32)
        .expect("create a native window for world/texture depth ordering");
    let mut renderer =
        Renderer::new(&mut window).expect("initialize Vulkan for world/texture ordering");
    let texture = Texture::from_rgba8(1, 1, &[0, 255, 0, 255]).expect("create green background");
    let gpu_texture = renderer
        .texture_device()
        .expect("create texture uploader")
        .upload(&texture)
        .expect("upload background texture");
    renderer
        .draw_texture_gpu(&gpu_texture, point(0.0, 0.0), 32, 32)
        .expect("queue full-window texture background");

    let scene = Scene::new(vec![
        Triangle::new([[-0.75, -0.75, 2.0], [0.75, -0.75, 2.0], [0.0, 0.75, 2.0]]).unwrap(),
    ])
    .unwrap();
    let camera = Camera::new(
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 0.0],
        90.0,
        0.01,
        10_000.0,
    )
    .unwrap();
    renderer
        .draw_scene_gpu(&scene, &camera, NamedColor::Cyan, [0.0, 0.0, 1.0], 1.0)
        .unwrap();

    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("read back world geometry over texture");
    assert_eq!(pixels[16 * 32 + 16], [0, 255, 255, 255]);
    assert_eq!(pixels[0], [0, 255, 0, 255]);
}

#[test]
fn translucent_world_scenes_sort_back_to_front_and_preserve_2d_overlays() {
    let mut window = Window::create_window("vagus-transparent-scene-test", 32, 32)
        .expect("create a native window for transparent scene sorting");
    let mut renderer =
        Renderer::new(&mut window).expect("initialize Vulkan for transparent scene sorting");
    let near_scene = Scene::new(vec![
        Triangle::new([[-0.75, -0.75, 2.0], [0.75, -0.75, 2.0], [0.0, 0.75, 2.0]]).unwrap(),
    ])
    .unwrap();
    let far_scene = Scene::new(vec![
        Triangle::new([[-1.5, -1.5, 4.0], [1.5, -1.5, 4.0], [0.0, 1.5, 4.0]]).unwrap(),
    ])
    .unwrap();
    let opaque_background = Scene::new(vec![
        Triangle::new([[-2.0, -2.0, 5.0], [2.0, -2.0, 5.0], [0.0, 2.0, 5.0]]).unwrap(),
    ])
    .unwrap();
    let camera = Camera::new(
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 0.0],
        90.0,
        0.01,
        10_000.0,
    )
    .unwrap();

    renderer
        .draw_scene_gpu_rgba(
            &near_scene,
            &camera,
            RgbaColor::rgba(255, 0, 0, 128),
            [0.0, 0.0, 1.0],
            1.0,
        )
        .unwrap();
    renderer
        .draw_scene_gpu_rgba(
            &far_scene,
            &camera,
            RgbaColor::rgba(0, 0, 255, 128),
            [0.0, 0.0, 1.0],
            1.0,
        )
        .unwrap();
    renderer
        .draw_scene_gpu(
            &opaque_background,
            &camera,
            NamedColor::White,
            [0.0, 0.0, 1.0],
            1.0,
        )
        .unwrap();
    renderer.draw_rectangle_gpu_rgba(point(0.0, 0.0), 2, 2, RgbaColor::rgba(0, 255, 0, 255));

    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("present sorted transparent world scenes");
    let overlap = pixels[16 * 32 + 16];
    assert!(
        overlap[0] > overlap[2],
        "near red scene should blend over far blue scene: {overlap:?}"
    );
    assert!(
        overlap[2] > 32,
        "far blue scene should remain visible: {overlap:?}"
    );
    assert!(
        overlap[1] > 32,
        "opaque scene should remain visible behind translucency: {overlap:?}"
    );
    assert_eq!(pixels[0], [0, 255, 0, 255], "2D overlay remains on top");
}

#[test]
fn world_space_triangles_are_clipped_by_vulkan_and_rasterized() {
    let mut window = Window::create_window("vagus-near-plane-clip-test", 32, 32)
        .expect("create a native window for near-plane clipping");
    let mut renderer =
        Renderer::new(&mut window).expect("initialize Vulkan for near-plane clipping");
    let scene = Scene::new(vec![
        Triangle::new([[0.0, 0.5, -0.5], [-1.0, -1.0, 2.0], [1.0, -1.0, 2.0]]).unwrap(),
    ])
    .unwrap();
    let camera = Camera::new(
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 0.0],
        90.0,
        0.01,
        10_000.0,
    )
    .unwrap();

    assert_eq!(
        renderer
            .draw_scene_gpu(&scene, &camera, NamedColor::Cyan, [0.0, 0.0, 1.0], 1.0)
            .unwrap(),
        1,
        "the original near-plane-crossing triangle is queued without CPU fan clipping"
    );
    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("present clipped world-space triangle");
    assert_eq!(pixels[16 * 32 + 16], [0, 255, 255, 255]);
    assert_eq!(pixels[31 * 32 + 31], [0, 0, 0, 255]);

    let moderate_side_scene = Scene::new(vec![
        Triangle::new([[-2.0, 0.0, 1.0], [0.0, -1.0, 1.0], [0.0, 1.0, 1.0]]).unwrap(),
    ])
    .unwrap();
    assert_eq!(
        renderer
            .draw_scene_gpu(
                &moderate_side_scene,
                &camera,
                NamedColor::Green,
                [0.0, 0.0, 1.0],
                1.0
            )
            .unwrap(),
        1,
        "ordinary side-frustum clipping leaves the original triangle for Vulkan"
    );
    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("present Vulkan-clipped side-frustum triangle");
    assert_eq!(pixels[16 * 32 + 8], [0, 255, 0, 255]);

    let crossing_scene = Scene::new(vec![
        Triangle::new([[-1.0e38, 0.0, 1.0], [1.0, -1.0, 1.0], [1.0, 1.0, 1.0]]).unwrap(),
    ])
    .unwrap();
    assert_eq!(
        renderer
            .draw_scene_gpu(
                &crossing_scene,
                &camera,
                NamedColor::Magenta,
                [0.0, 0.0, 1.0],
                1.0
            )
            .unwrap(),
        2,
        "pathological projected outliers use bounded CPU safety clipping"
    );
    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("present side-frustum-clipped world-space triangle");
    assert_eq!(pixels[16 * 32 + 8], [255, 0, 255, 255]);
}

#[test]
fn world_space_triangles_beyond_far_plane_are_trivially_rejected() {
    let mut window = Window::create_window("vagus-far-plane-clip-test", 16, 16)
        .expect("create a native window for far-plane clipping");
    let mut renderer =
        Renderer::new(&mut window).expect("initialize Vulkan for far-plane clipping");
    let scene = Scene::new(vec![
        Triangle::new([[-1.0, -1.0, 6.0], [1.0, -1.0, 6.0], [0.0, 1.0, 6.0]]).unwrap(),
    ])
    .unwrap();
    let camera = Camera::new(
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 0.0],
        90.0,
        0.01,
        5.0,
    )
    .unwrap();

    assert_eq!(
        renderer
            .draw_scene_gpu(&scene, &camera, NamedColor::Cyan, [0.0, 0.0, 1.0], 1.0)
            .unwrap(),
        0,
        "a triangle wholly beyond the far plane should be rejected before submission"
    );
    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("read back the clipped far plane");
    assert!(pixels.iter().all(|pixel| *pixel == [0, 0, 0, 255]));
}

#[test]
fn degenerate_triangles_outside_frustum_are_culled_before_lighting_validation() {
    let mut window = Window::create_window("vagus-degenerate-frustum-test", 16, 16)
        .expect("create a native window for degenerate frustum test");
    let mut renderer =
        Renderer::new(&mut window).expect("initialize Vulkan for degenerate frustum test");
    let scene = Scene::new(vec![Triangle::new([[0.0, 0.0, 6.0]; 3]).unwrap()]).unwrap();
    let camera = Camera::new(
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 0.0],
        90.0,
        0.01,
        5.0,
    )
    .unwrap();

    assert_eq!(
        renderer
            .draw_scene_gpu(&scene, &camera, NamedColor::Cyan, [0.0, 0.0, 1.0], 1.0)
            .unwrap(),
        0,
        "off-frustum triangles do not need lighting normals"
    );
    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("read back frame without visible triangles");
    assert!(pixels.iter().all(|pixel| *pixel == [0, 0, 0, 255]));

    let visible_degenerate =
        Scene::new(vec![Triangle::new([[0.0, 0.0, 2.0]; 3]).unwrap()]).unwrap();
    assert!(
        renderer
            .draw_scene_gpu(
                &visible_degenerate,
                &camera,
                NamedColor::Cyan,
                [0.0, 0.0, 1.0],
                1.0,
            )
            .is_err()
    );
}

#[test]
fn world_space_triangles_crossing_far_plane_are_clipped_by_vulkan() {
    let mut window = Window::create_window("vagus-far-plane-cross-test", 16, 16)
        .expect("create a native window for far-plane crossing test");
    let mut renderer =
        Renderer::new(&mut window).expect("initialize Vulkan for far-plane crossing");
    let scene = Scene::new(vec![
        Triangle::new([[-1.0, -1.0, 4.0], [1.0, -1.0, 4.0], [0.0, 1.0, 6.0]]).unwrap(),
    ])
    .unwrap();
    let camera = Camera::new(
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 0.0],
        90.0,
        0.1,
        5.0,
    )
    .unwrap();

    assert_eq!(
        renderer
            .draw_scene_gpu(&scene, &camera, NamedColor::Cyan, [0.0, 0.0, 1.0], 1.0)
            .unwrap(),
        1,
        "a triangle crossing the far plane remains queued for Vulkan clipping"
    );
    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("read back far-plane-clipped geometry");
    assert_eq!(pixels[8 * 16 + 8], [0, 255, 255, 255]);
}

#[test]
fn fully_outside_world_triangles_are_rejected_before_gpu_submission() {
    let mut window = Window::create_window("vagus-frustum-cull-test", 16, 16)
        .expect("create a native window for frustum-culling test");
    let mut renderer = Renderer::new(&mut window).expect("initialize Vulkan for frustum culling");
    let scene = Scene::new(vec![
        Triangle::new([[8.0, 0.0, 1.0], [10.0, -1.0, 1.0], [10.0, 1.0, 1.0]]).unwrap(),
        Triangle::new([[-10.0, 0.0, 1.0], [-8.0, -1.0, 1.0], [-8.0, 1.0, 1.0]]).unwrap(),
        Triangle::new([[-1.0, 8.0, 1.0], [1.0, 8.0, 1.0], [0.0, 10.0, 1.0]]).unwrap(),
        Triangle::new([[-1.0, -10.0, 1.0], [1.0, -10.0, 1.0], [0.0, -8.0, 1.0]]).unwrap(),
        Triangle::new([[-1.0, -1.0, 0.05], [1.0, -1.0, 0.05], [0.0, 1.0, 0.05]]).unwrap(),
        Triangle::new([[-1.0, -1.0, -2.0], [1.0, -1.0, -2.0], [0.0, 1.0, -2.0]]).unwrap(),
        Triangle::new([[-1.0, -1.0, 6.0], [1.0, -1.0, 6.0], [0.0, 1.0, 6.0]]).unwrap(),
    ])
    .unwrap();
    let camera = Camera::new(
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 0.0],
        90.0,
        0.1,
        5.0,
    )
    .unwrap();

    assert_eq!(
        renderer
            .draw_scene_gpu(&scene, &camera, NamedColor::Cyan, [0.0, 0.0, 1.0], 1.0)
            .unwrap(),
        0,
        "triangles wholly outside the near, far, or side planes should not be queued"
    );
    let outside_scene = Scene::new(vec![
        Triangle::new([[8.0, -1.0, 1.0], [9.0, -1.0, 1.0], [8.5, 0.0, 1.0]]).unwrap(),
        Triangle::new([[9.0, -1.0, 1.0], [10.0, -1.0, 1.0], [9.5, 0.0, 1.0]]).unwrap(),
        Triangle::new([[10.0, -1.0, 1.0], [11.0, -1.0, 1.0], [10.5, 0.0, 1.0]]).unwrap(),
        Triangle::new([[11.0, -1.0, 1.0], [12.0, -1.0, 1.0], [11.5, 0.0, 1.0]]).unwrap(),
    ])
    .unwrap();
    assert_eq!(
        renderer
            .draw_scene_gpu(
                &outside_scene,
                &camera,
                NamedColor::Cyan,
                [0.0, 0.0, 1.0],
                1.0,
            )
            .unwrap(),
        0,
        "a multi-triangle scene wholly outside one frustum plane should be culled"
    );
    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("read back the culled scene");
    assert!(pixels.iter().all(|pixel| *pixel == [0, 0, 0, 255]));
}

#[test]
fn renderer_texture_upload_keeps_its_device_alive_after_renderer_drop() {
    let mut window = Window::create_window("vagus-render-texture-test", 8, 8)
        .expect("create a native window for shared texture upload");
    let texture = Texture::from_rgba8(1, 1, &[12, 34, 56, 255]).unwrap();
    let gpu_texture;
    {
        let renderer = Renderer::new(&mut window).expect("initialize Vulkan for the window");
        gpu_texture = renderer
            .texture_device()
            .expect("create texture uploader on the renderer device")
            .upload(&texture)
            .expect("upload texture on the renderer's Vulkan device");
    }

    assert_eq!(gpu_texture.width(), 1);
    assert_eq!(gpu_texture.height(), 1);
    drop(gpu_texture);
}

#[test]
fn renderer_rejects_foreign_device_textures_when_queued() {
    let texture = Texture::from_rgba8(1, 1, &[12, 34, 56, 255]).unwrap();
    let foreign_texture = {
        let mut source_window = Window::create_window("vagus-foreign-texture-source", 8, 8)
            .expect("create a native window for the source renderer");
        let source_renderer =
            Renderer::new(&mut source_window).expect("initialize the source renderer");
        source_renderer
            .texture_device()
            .expect("create a texture uploader on the source renderer's device")
            .upload(&texture)
            .expect("upload a texture owned by the source renderer's device")
    };

    let mut window = Window::create_window("vagus-foreign-texture-test", 8, 8)
        .expect("create a native window for the destination renderer");
    let mut renderer = Renderer::new(&mut window).expect("initialize destination Vulkan renderer");

    let error = renderer
        .draw_texture_gpu(&foreign_texture, point(0.0, 0.0), 1, 1)
        .expect_err("reject a texture owned by another Vulkan device");

    assert!(error.contains("different Vulkan device"), "{error}");

    let pixels = renderer
        .present_gpu_readback(NamedColor::Black)
        .expect("a rejected texture must not poison the next frame");
    assert!(pixels.iter().all(|pixel| *pixel == [0, 0, 0, 255]));
}

#[test]
fn gpu_textures_render_as_scaled_quads_before_gpu_primitives() {
    let mut window = Window::create_window("vagus-render-texture-quad-test", 32, 32)
        .expect("create a native window for GPU texture rendering");
    let mut renderer = Renderer::new(&mut window).expect("initialize Vulkan for texture rendering");
    let texture = Texture::from_rgba8(
        2,
        2,
        &[
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ],
    )
    .expect("create a 2x2 RGBA texture");
    let gpu_texture = renderer
        .texture_device()
        .expect("create a texture uploader on the renderer device")
        .upload(&texture)
        .expect("upload the texture");
    let solid_texture = Texture::from_rgba8(1, 1, &[0, 255, 0, 255]).expect("create green texture");
    let gpu_solid_texture = renderer
        .texture_device()
        .expect("create a second texture uploader on the renderer device")
        .upload(&solid_texture)
        .expect("upload the second texture");
    let red_texture =
        Texture::from_rgba8(1, 1, &[255, 0, 0, 255]).expect("create red overlay texture");
    let gpu_red_texture = renderer
        .texture_device()
        .expect("create a third texture uploader on the renderer device")
        .upload(&red_texture)
        .expect("upload the red overlay texture");

    renderer
        .draw_texture_gpu(&gpu_texture, point(8.0, 8.0), 16, 16)
        .expect("queue first GPU texture quad");
    renderer
        .draw_texture_gpu(&gpu_texture, point(24.0, 24.0), 8, 8)
        .expect("queue a second GPU texture quad using the same image");
    renderer
        .draw_texture_gpu(&gpu_solid_texture, point(0.0, 24.0), 8, 8)
        .expect("queue a different texture between matching quads");
    renderer
        .draw_texture_gpu(&gpu_texture, point(24.0, 0.0), 8, 8)
        .expect("queue the first texture after a different image");
    renderer
        .draw_texture_gpu(&gpu_red_texture, point(12.0, 12.0), 8, 8)
        .expect("queue the first red overlay");
    renderer
        .draw_texture_gpu(&gpu_solid_texture, point(14.0, 14.0), 4, 4)
        .expect("interleave a green texture over the red overlay");
    renderer
        .draw_texture_gpu(&gpu_red_texture, point(16.0, 16.0), 4, 4)
        .expect("queue the red texture again after the interleaved draw");
    drop(gpu_texture);
    drop(gpu_solid_texture);
    drop(gpu_red_texture);
    renderer.draw_rectangle_gpu(point(26.0, 8.0), 4, 4, NamedColor::Cyan);
    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("present and read back the textured quad");

    assert_eq!(pixels[10 * 32 + 10], [255, 0, 0, 255]);
    assert_eq!(pixels[24 * 32 + 24], [255, 0, 0, 255]);
    assert_eq!(pixels[10 * 32 + 22], [0, 255, 0, 255]);
    assert_eq!(pixels[22 * 32 + 10], [0, 0, 255, 255]);
    assert_eq!(pixels[22 * 32 + 22], [255, 255, 255, 255]);
    assert_eq!(pixels[27 * 32 + 3], [0, 255, 0, 255]);
    assert_eq!(pixels[0 * 32 + 24], [255, 0, 0, 255]);
    assert_eq!(pixels[13 * 32 + 13], [255, 0, 0, 255]);
    assert_eq!(pixels[15 * 32 + 15], [0, 255, 0, 255]);
    assert_eq!(pixels[17 * 32 + 17], [255, 0, 0, 255]);
    assert_eq!(pixels[10 * 32 + 27], [0, 255, 255, 255]);
    assert_eq!(pixels[3 * 32 + 3], [0, 0, 0, 255]);
    assert_eq!(
        renderer.last_frame_instanced_draw_count(),
        1,
        "the adjacent pair of matching textures should use one instanced draw"
    );
}

#[test]
fn gpu_texture_alpha_blends_over_the_destination_pixel() {
    let mut window = Window::create_window("vagus-render-alpha-blend-test", 32, 32)
        .expect("create a native window for texture blending");
    let mut renderer = Renderer::new(&mut window).expect("initialize Vulkan for texture blending");
    let texture = Texture::from_rgba8(1, 1, &[255, 0, 0, 128]).expect("create half-alpha red");
    let gpu_texture = renderer
        .texture_device()
        .expect("create a texture uploader on the renderer device")
        .upload(&texture)
        .expect("upload half-alpha red");

    renderer
        .draw_texture_gpu(&gpu_texture, point(0.0, 0.0), 8, 8)
        .expect("queue a translucent texture quad");
    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 255))
        .expect("present and read back the blended texture");
    let blended = pixels[4 * 32 + 4];

    assert!(
        (120..=190).contains(&blended[0]),
        "unexpected red channel: {blended:?}"
    );
    assert_eq!(blended[1], 0);
    assert!(
        (120..=190).contains(&blended[2]),
        "unexpected blue channel: {blended:?}"
    );
    assert_eq!(blended[3], 255);
    assert_eq!(pixels[16 * 32 + 16], [0, 0, 255, 255]);
}

#[test]
fn gpu_texture_descriptor_is_updated_between_presented_frames() {
    let mut window = Window::create_window("vagus-render-texture-reuse-test", 32, 32)
        .expect("create a native window for descriptor reuse");
    let mut renderer = Renderer::new(&mut window).expect("initialize Vulkan for descriptor reuse");
    let red = Texture::from_rgba8(1, 1, &[255, 0, 0, 255]).expect("create red texture");
    let gpu_red = renderer
        .texture_device()
        .expect("create a texture uploader")
        .upload(&red)
        .expect("upload red texture");
    let gpu_red_repeat = gpu_red.clone();
    renderer
        .draw_texture_gpu(&gpu_red, point(0.0, 0.0), 8, 8)
        .expect("queue red texture");
    drop(gpu_red);

    let first_frame = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("present red texture");
    assert_eq!(first_frame[4 * 32 + 4], [255, 0, 0, 255]);

    renderer
        .draw_texture_gpu(&gpu_red_repeat, point(0.0, 0.0), 8, 8)
        .expect("queue the same texture in the reused descriptor slot");
    let repeated_frame = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("present the same texture again");
    assert_eq!(repeated_frame[4 * 32 + 4], [255, 0, 0, 255]);
    drop(gpu_red_repeat);

    let green = Texture::from_rgba8(1, 1, &[0, 255, 0, 255]).expect("create green texture");
    let gpu_green = renderer
        .texture_device()
        .expect("reuse a texture uploader")
        .upload(&green)
        .expect("upload green texture");
    renderer
        .draw_texture_gpu(&gpu_green, point(0.0, 0.0), 8, 8)
        .expect("queue green texture in the reused descriptor slot");

    let second_frame = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("present green texture");
    assert_eq!(second_frame[4 * 32 + 4], [0, 255, 0, 255]);
    assert_eq!(second_frame[16 * 32 + 16], [0, 0, 0, 255]);
}

#[test]
fn gpu_scene_respects_camera_specific_far_plane() {
    let mut window = Window::create_window("vagus-custom-far-plane-test", 16, 16)
        .expect("create a native window for custom far-plane clipping");
    let mut renderer = Renderer::new(&mut window).expect("initialize Vulkan for custom clipping");
    let scene = Scene::new(vec![
        Triangle::new([[-1.0, -1.0, 6.0], [1.0, -1.0, 6.0], [0.0, 1.0, 6.0]]).unwrap(),
    ])
    .unwrap();
    let camera = Camera::new(
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 0.0],
        60.0,
        0.1,
        5.0,
    )
    .unwrap();

    assert_eq!(
        renderer
            .draw_scene_gpu(&scene, &camera, NamedColor::Cyan, [0.0, 0.0, 1.0], 1.0)
            .unwrap(),
        0,
        "a triangle beyond this camera's far plane is rejected before GPU submission"
    );
    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("read back custom far-plane clipping");
    assert!(pixels.iter().all(|pixel| *pixel == [0, 0, 0, 255]));
}
