//! Bounded Vulkan ECS render preparation and present-latency probe.

use std::time::{Duration, Instant};
use vagus::heightmap::HeightMap;
use vagus::material::Material;
use vagus::math::Vec3;
use vagus::physics::PhysicsObject3D;
use vagus::render::{Color, Renderer};
use vagus::rtx::{Camera, Triangle};
use vagus::scene::default::{GameObject3D, GameScene3D, Mesh3D, Transform3D};
use vagus::texture::Texture;
use vagus::window::Window;

const WIDTH: u32 = 256;
const HEIGHT: u32 = 256;
const OBJECTS_PER_ROW: usize = 32;
const OBJECT_ROWS: usize = 16;
const WARMUP_FRAMES: usize = 4;
const SAMPLE_FRAMES: usize = 24;
const MAX_SAMPLE_FRAMES: usize = 32;

fn point3(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3 { x, y, z }
}

fn build_scene() -> Result<(GameScene3D, GameScene3D, Camera), String> {
    let mut scene = GameScene3D::with_capacity(OBJECTS_PER_ROW * OBJECT_ROWS)
        .map_err(|error| format!("creating benchmark ECS scene failed: {error}"))?;
    let texture = Texture::from_rgba8(1, 1, &[255, 255, 255, 255])?;
    let heightmap = HeightMap::new(1, 1, vec![0.0])
        .map_err(|error| format!("creating heightmap failed: {error}"))?;
    let material = Material::new(texture, heightmap);
    let triangle = Triangle::new_with_uvs(
        [[-0.12, -0.12, 0.0], [0.12, -0.12, 0.0], [0.0, 0.12, 0.0]],
        [[0.0, 0.0]; 3],
    )?;
    let mesh = Mesh3D::new(vec![triangle]);

    for row in 0..OBJECT_ROWS {
        for column in 0..OBJECTS_PER_ROW {
            let x = (column as f32 - (OBJECTS_PER_ROW as f32 - 1.0) * 0.5) * 0.45;
            let y = ((OBJECT_ROWS as f32 - 1.0) * 0.5 - row as f32) * 0.45;
            let mut object = GameObject3D::new(material.clone(), PhysicsObject3D::default());
            object.set_mesh(mesh.clone());
            object.set_transform(Transform3D::new(
                point3(x, y, 20.0),
                point3(0.0, 0.0, 0.0),
                point3(1.0, 1.0, 1.0),
            )?);
            scene
                .objects_mut()
                .push(object)
                .map_err(|error| format!("inserting benchmark object failed: {error}"))?;
        }
    }

    let mut control_scene = GameScene3D::with_capacity(1)
        .map_err(|error| format!("creating benchmark control scene failed: {error}"))?;
    let mut control_object = GameObject3D::new(material, PhysicsObject3D::default());
    control_object.set_mesh(mesh);
    control_object.set_transform(Transform3D::new(
        point3(0.0, 0.0, 20.0),
        point3(0.0, 0.0, 0.0),
        point3(1.0, 1.0, 1.0),
    )?);
    control_scene
        .objects_mut()
        .push(control_object)
        .map_err(|error| format!("inserting benchmark control object failed: {error}"))?;

    let camera = Camera::new(
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 0.0],
        90.0,
        0.1,
        100.0,
    )?;
    Ok((scene, control_scene, camera))
}

fn percentile(samples: &[Duration], numerator: usize) -> Duration {
    let index = (samples.len() * numerator).div_ceil(100).saturating_sub(1);
    samples[index.min(samples.len() - 1)]
}

fn record_sample(
    renderer: &mut Renderer<'_>,
    scene: &GameScene3D,
    camera: &Camera,
    queue_times: &mut Vec<Duration>,
    present_times: &mut Vec<Duration>,
) -> Result<usize, String> {
    let started = Instant::now();
    let triangles = renderer.draw_game_scene_3d(
        scene,
        camera,
        Color::rgb(255, 255, 255),
        [0.0, 0.0, 1.0],
        1.0,
    )?;
    queue_times.push(started.elapsed());

    let started = Instant::now();
    renderer.present_gpu(Color::rgb(0, 0, 0))?;
    present_times.push(started.elapsed());
    Ok(triangles)
}

fn main() -> Result<(), String> {
    assert!(SAMPLE_FRAMES > 0 && SAMPLE_FRAMES <= MAX_SAMPLE_FRAMES);

    let (scene, control_scene, camera) = build_scene()?;
    let mut window = Window::create_window("vagus-render-scene-prepare-bench", WIDTH, HEIGHT)?;
    let mut renderer = Renderer::new(&mut window)?;

    for _ in 0..WARMUP_FRAMES {
        record_sample(
            &mut renderer,
            &scene,
            &camera,
            &mut Vec::new(),
            &mut Vec::new(),
        )?;
        record_sample(
            &mut renderer,
            &control_scene,
            &camera,
            &mut Vec::new(),
            &mut Vec::new(),
        )?;
    }

    let mut scene_queue_times = Vec::with_capacity(SAMPLE_FRAMES);
    let mut control_queue_times = Vec::with_capacity(SAMPLE_FRAMES);
    let mut present_times = Vec::with_capacity(SAMPLE_FRAMES);
    let mut control_present_times = Vec::with_capacity(SAMPLE_FRAMES);
    let mut queued_triangles = 0;
    let mut control_triangles = 0;
    for sample in 0..SAMPLE_FRAMES {
        if sample % 2 == 0 {
            queued_triangles = record_sample(
                &mut renderer,
                &scene,
                &camera,
                &mut scene_queue_times,
                &mut present_times,
            )?;
            control_triangles = record_sample(
                &mut renderer,
                &control_scene,
                &camera,
                &mut control_queue_times,
                &mut control_present_times,
            )?;
        } else {
            control_triangles = record_sample(
                &mut renderer,
                &control_scene,
                &camera,
                &mut control_queue_times,
                &mut control_present_times,
            )?;
            queued_triangles = record_sample(
                &mut renderer,
                &scene,
                &camera,
                &mut scene_queue_times,
                &mut present_times,
            )?;
        }
    }

    let queued = renderer.draw_game_scene_3d(
        &scene,
        &camera,
        Color::rgb(255, 255, 255),
        [0.0, 0.0, 1.0],
        1.0,
    )?;
    let pixels = renderer.present_gpu_readback(Color::rgb(0, 0, 0))?;
    if !pixels
        .iter()
        .any(|pixel| pixel[0] != 0 || pixel[1] != 0 || pixel[2] != 0)
    {
        return Err("benchmark scene produced no visible GPU pixels".into());
    }
    let control_queued = renderer.draw_game_scene_3d(
        &control_scene,
        &camera,
        Color::rgb(255, 255, 255),
        [0.0, 0.0, 1.0],
        1.0,
    )?;
    let control_pixels = renderer.present_gpu_readback(Color::rgb(0, 0, 0))?;
    if !control_pixels
        .iter()
        .any(|pixel| pixel[0] != 0 || pixel[1] != 0 || pixel[2] != 0)
    {
        return Err("benchmark control scene produced no visible GPU pixels".into());
    }

    scene_queue_times.sort_unstable();
    control_queue_times.sort_unstable();
    present_times.sort_unstable();
    control_present_times.sort_unstable();
    println!(
        "scope: CPU draw_game_scene_3d queueing and present_gpu call; present includes fence/present-mode effects, excludes scene construction and readback; target/control order alternates"
    );
    println!(
        "target: {} objects, {} triangles queued per sample; control: 1 object, {} triangles queued per sample; framebuffer {}x{}; warmup {}, samples {} per path (cap {})",
        OBJECTS_PER_ROW * OBJECT_ROWS,
        queued_triangles,
        control_triangles,
        WIDTH,
        HEIGHT,
        WARMUP_FRAMES,
        SAMPLE_FRAMES,
        MAX_SAMPLE_FRAMES,
    );
    println!(
        "target scene queue ns: min={} median={} p95={}",
        scene_queue_times[0].as_nanos(),
        scene_queue_times[scene_queue_times.len() / 2].as_nanos(),
        percentile(&scene_queue_times, 95).as_nanos(),
    );
    println!(
        "target present call ns: min={} median={} p95={}",
        present_times[0].as_nanos(),
        present_times[present_times.len() / 2].as_nanos(),
        percentile(&present_times, 95).as_nanos(),
    );
    println!(
        "control scene queue ns: min={} median={} p95={}",
        control_queue_times[0].as_nanos(),
        control_queue_times[control_queue_times.len() / 2].as_nanos(),
        percentile(&control_queue_times, 95).as_nanos(),
    );
    println!(
        "control present call ns: min={} median={} p95={}",
        control_present_times[0].as_nanos(),
        control_present_times[control_present_times.len() / 2].as_nanos(),
        percentile(&control_present_times, 95).as_nanos(),
    );
    println!(
        "visible GPU readbacks verified; final target/control triangles={queued}/{control_queued}"
    );
    Ok(())
}
