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

use std::marker::PhantomData;

use crate::{
    ecs::{Ecs, Soa},
    material::Material,
    math::MathVec,
    physics::{PhysicsObject2D, PhysicsObject3D, hitbox::Hitbox},
    shader::Shader,
    texture::Texture,
};
use vagus_essential::collections::StableVec;

pub trait GameObject<V>
where
    V: MathVec,
{
    fn hitbox(&self) -> &impl Hitbox<V>;
}

pub trait GameObject2D<V>
where
    V: MathVec,
{
    fn texture(&self) -> &Texture;
    fn physics_object(&self) -> &PhysicsObject2D;
}

pub trait GameObject3D<V>
where
    V: MathVec,
{
    fn material(&self) -> &Material;
    fn physics_object(&self) -> &PhysicsObject3D;
}

pub trait Scene<V>
where
    V: MathVec,
{
    fn shaders(&self) -> &StableVec<Shader>;
    fn is_rtx_on(&self) -> bool;
}

/// Compatibility marker for scenes used by the game-scene API.
pub trait GameScene<V>: Scene<V>
where
    V: MathVec,
{
}

impl<V, S> GameScene<V> for S
where
    V: MathVec,
    S: Scene<V>,
{
}

#[derive(Soa)]
struct SceneEntry<S> {
    scene: S,
}

/// Stores scenes in ECS columns and tracks the active scene by row index.
pub struct SceneManager<S, V>
where
    V: MathVec,
    S: Scene<V>,
{
    scenes: Ecs<SceneEntry<S>>,
    current: Option<usize>,
    _vector: PhantomData<fn() -> V>,
}

impl<S, V> SceneManager<S, V>
where
    V: MathVec,
    S: Scene<V>,
{
    pub fn new() -> Self {
        Self {
            scenes: Ecs::new(),
            current: None,
            _vector: std::marker::PhantomData,
        }
    }

    pub fn with_capacity(capacity: usize) -> std::io::Result<Self> {
        Ok(Self {
            scenes: Ecs::with_capacity(capacity)?,
            current: None,
            _vector: std::marker::PhantomData,
        })
    }

    /// Creates scene storage with an initial capacity that grows as scenes are added.
    pub fn with_growable_capacity(capacity: usize) -> std::io::Result<Self> {
        Ok(Self {
            scenes: Ecs::with_growable_capacity(capacity)?,
            current: None,
            _vector: std::marker::PhantomData,
        })
    }

    /// Adds a scene and selects it when it is the first scene.
    pub fn add_scene(&mut self, scene: S) -> std::io::Result<usize> {
        let index = self.scenes.len();
        self.scenes.push(SceneEntry { scene })?;
        if self.current.is_none() {
            self.current = Some(index);
        }
        Ok(index)
    }

    pub fn select_scene(&mut self, index: usize) -> bool {
        if index >= self.scenes.len() {
            return false;
        }
        self.current = Some(index);
        true
    }

    pub fn remove_scene(&mut self, index: usize) -> Option<S> {
        let scene = self.scenes.remove(index)?.scene;
        self.current = match self.current {
            Some(_) if self.scenes.is_empty() => None,
            Some(current) if current > index => Some(current - 1),
            Some(current) if current == index => Some(index.min(self.scenes.len() - 1)),
            current => current,
        };
        Some(scene)
    }

    pub fn current_scene(&self) -> Option<&S> {
        self.current
            .and_then(|index| self.scenes.storage().scene.get(index))
    }

    pub fn current_scene_mut(&mut self) -> Option<&mut S> {
        self.current
            .and_then(|index| self.scenes.storage_mut().scene.get_mut(index))
    }

    pub fn current_index(&self) -> Option<usize> {
        self.current
    }

    pub fn len(&self) -> usize {
        self.scenes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.scenes.is_empty()
    }
}

pub mod default {
    use std::{
        collections::hash_map::DefaultHasher,
        hash::{Hash, Hasher},
        io,
        marker::PhantomData,
    };

    use crate::{
        ecs::{Ecs, Soa},
        heightmap::HeightMap,
        material::Material,
        math::{MathVec, Vec2, Vec3},
        physics::{PhysicsBody, PhysicsObject2D, PhysicsObject3D, PhysicsStepper},
        rtx::Triangle,
        scene::{GameObject as GameObjectTrait, Scene},
        shader::Shader,
        texture::Texture,
    };
    use vagus_essential::collections::StableVec;

    /// A default 2D game object with an RGBA texture and a physics body.
    #[derive(Soa)]
    pub struct GameObject2D {
        texture: Texture,
        physics_object: PhysicsObject2D,
    }

    impl GameObject2D {
        pub fn new(texture: Texture, physics_object: PhysicsObject2D) -> Self {
            Self {
                texture,
                physics_object,
            }
        }

        pub fn texture(&self) -> &Texture {
            &self.texture
        }

        pub fn physics_object(&self) -> &PhysicsObject2D {
            &self.physics_object
        }

        pub fn physics_object_mut(&mut self) -> &mut PhysicsObject2D {
            &mut self.physics_object
        }
    }

    impl Default for GameObject2D {
        fn default() -> Self {
            Self::new(
                Texture::from_rgba8(1, 1, &[255, 255, 255, 255])
                    .expect("the built-in white texel is valid"),
                PhysicsObject2D::default(),
            )
        }
    }

    impl GameObjectTrait<Vec2> for GameObject2D {
        fn hitbox(&self) -> &impl crate::physics::hitbox::Hitbox<Vec2> {
            &self.physics_object.hitbox
        }
    }

    impl super::GameObject2D<Vec2> for GameObject2D {
        fn texture(&self) -> &Texture {
            &self.texture
        }

        fn physics_object(&self) -> &PhysicsObject2D {
            &self.physics_object
        }
    }

    impl PhysicsBody for GameObject2D {
        fn integrate_body(&mut self, delta_time: f32) {
            self.physics_object.integrate_body(delta_time);
        }

        fn resolve_body_collision(&mut self, other: &mut Self, friction: f32) -> bool {
            self.physics_object
                .resolve_body_collision(&mut other.physics_object, friction)
        }

        fn broadphase_bounds(&self) -> Option<[(f64, f64); 3]> {
            self.physics_object.broadphase_bounds()
        }
    }

    /// A local-space triangle mesh. Physics hitboxes do not define visible geometry.
    #[derive(Clone, Debug)]
    pub struct Mesh3D {
        triangles: Vec<Triangle>,
        fingerprint: u64,
        bounds: Option<([f32; 3], [f32; 3])>,
        flat_normals: std::sync::OnceLock<Option<Vec<[f32; 3]>>>,
    }

    fn mesh_fingerprint(triangles: &[Triangle]) -> u64 {
        let mut hasher = DefaultHasher::new();
        for triangle in triangles {
            for value in triangle
                .vertices()
                .iter()
                .flatten()
                .chain(triangle.uvs().iter().flatten())
            {
                value.to_bits().hash(&mut hasher);
            }
        }
        triangles.len().hash(&mut hasher);
        hasher.finish()
    }

    fn mesh_bounds(triangles: &[Triangle]) -> Option<([f32; 3], [f32; 3])> {
        let first = *triangles.first()?.vertices().first()?;
        let mut minimum = first;
        let mut maximum = first;
        for point in triangles.iter().flat_map(|triangle| triangle.vertices()) {
            for axis in 0..3 {
                minimum[axis] = minimum[axis].min(point[axis]);
                maximum[axis] = maximum[axis].max(point[axis]);
            }
        }
        Some((minimum, maximum))
    }

    impl Mesh3D {
        pub fn new(triangles: Vec<Triangle>) -> Self {
            let fingerprint = mesh_fingerprint(&triangles);
            let bounds = mesh_bounds(&triangles);
            Self {
                triangles,
                fingerprint,
                bounds,
                flat_normals: std::sync::OnceLock::new(),
            }
        }

        pub fn triangles(&self) -> &[Triangle] {
            &self.triangles
        }

        pub(crate) fn fingerprint(&self) -> u64 {
            self.fingerprint
        }

        pub(crate) fn bounds(&self) -> Option<([f32; 3], [f32; 3])> {
            self.bounds
        }

        pub(crate) fn flat_normals(&self) -> Result<Option<&[[f32; 3]]>, String> {
            if let Some(normals) = self.flat_normals.get() {
                return Ok(normals.as_deref());
            }
            let normals = compute_mesh_flat_normals(&self.triangles)?;
            let _ = self.flat_normals.set(normals);
            Ok(self.flat_normals.get().and_then(|cached| cached.as_deref()))
        }
    }

    fn compute_mesh_flat_normals(triangles: &[Triangle]) -> Result<Option<Vec<[f32; 3]>>, String> {
        let mut normals = Vec::new();
        normals
            .try_reserve_exact(triangles.len())
            .map_err(|error| format!("reserving scene mesh normals failed: {error}"))?;
        for triangle in triangles {
            let points = triangle.vertices();
            let edge_a: [f64; 3] =
                std::array::from_fn(|axis| f64::from(points[1][axis]) - f64::from(points[0][axis]));
            let edge_b: [f64; 3] =
                std::array::from_fn(|axis| f64::from(points[2][axis]) - f64::from(points[0][axis]));
            let normal = [
                edge_a[1] * edge_b[2] - edge_a[2] * edge_b[1],
                edge_a[2] * edge_b[0] - edge_a[0] * edge_b[2],
                edge_a[0] * edge_b[1] - edge_a[1] * edge_b[0],
            ];
            let length = normal.iter().map(|value| value * value).sum::<f64>().sqrt();
            if length == 0.0 || !length.is_finite() {
                return Ok(None);
            }
            normals.push(normal.map(|value| (value / length) as f32));
        }
        Ok(Some(normals))
    }

    impl PartialEq for Mesh3D {
        fn eq(&self, other: &Self) -> bool {
            self.triangles == other.triangles && self.fingerprint == other.fingerprint
        }
    }

    impl Default for Mesh3D {
        fn default() -> Self {
            Self::new(Vec::new())
        }
    }

    #[cfg(test)]
    mod mesh_fingerprint_tests {
        use super::{Mesh3D, Transform3D};
        use crate::api::math::Vec3;
        use crate::api::rtx::Triangle;

        fn mesh(first_x: f32, uvs: [[f32; 2]; 3]) -> Mesh3D {
            let triangle = Triangle::new_with_uvs(
                [[first_x, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                uvs,
            )
            .expect("create test triangle");
            Mesh3D::new(vec![triangle])
        }

        #[test]
        fn mesh_bounds_cover_all_triangle_vertices() {
            let mesh = Mesh3D::new(vec![
                Triangle::new([[-2.0, -1.0, 0.0], [1.0, -1.0, 3.0], [0.0, 2.0, -2.0]]).unwrap(),
                Triangle::new([[4.0, 1.0, 1.0], [2.0, -3.0, 5.0], [3.0, 0.0, -4.0]]).unwrap(),
            ]);

            assert_eq!(mesh.bounds(), Some(([-2.0, -3.0, -4.0], [4.0, 2.0, 5.0])));
        }

        #[test]
        fn empty_mesh_has_no_local_bounds() {
            assert_eq!(Mesh3D::default().bounds(), None);
        }

        #[test]
        fn transformed_mesh_bounds_enclose_rotated_scaled_geometry() {
            let transform = Transform3D::new(
                Vec3 {
                    x: 3.0,
                    y: -4.0,
                    z: 2.0,
                },
                Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: std::f32::consts::FRAC_PI_2,
                },
                Vec3 {
                    x: 2.0,
                    y: 1.0,
                    z: 1.0,
                },
            )
            .unwrap();
            let bounds = transform
                .matrix()
                .transform_bounds(([-1.0; 3], [1.0; 3]))
                .unwrap();

            for (actual, expected) in bounds.0.into_iter().zip([2.0, -6.0, 1.0]) {
                assert!((actual - expected).abs() < 1.0e-5);
            }
            for (actual, expected) in bounds.1.into_iter().zip([4.0, -2.0, 3.0]) {
                assert!((actual - expected).abs() < 1.0e-5);
            }
        }

        #[test]
        fn cached_mesh_fingerprint_covers_geometry_uvs_and_clone() {
            let uvs = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]];
            let original = mesh(0.0, uvs);
            let fingerprint = original.fingerprint();

            assert_eq!(original.clone().fingerprint(), fingerprint);
            assert_ne!(mesh(0.5, uvs).fingerprint(), fingerprint);
            assert_ne!(
                mesh(0.0, [[0.25, 0.0], [1.0, 0.0], [0.0, 1.0]]).fingerprint(),
                fingerprint
            );
            assert_eq!(
                Mesh3D::default().fingerprint(),
                Mesh3D::new(Vec::new()).fingerprint()
            );
        }

        #[test]
        fn mesh_flat_normals_are_cached_without_changing_mesh_equality() {
            let mesh = mesh(0.0, [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]]);
            let first = mesh.flat_normals().unwrap().unwrap();
            let first_pointer = first.as_ptr();

            assert_eq!(first, &[[0.0, 0.0, 1.0]]);
            assert_eq!(
                mesh.flat_normals().unwrap().unwrap().as_ptr(),
                first_pointer
            );
            assert_eq!(mesh, mesh.clone());
        }

        #[test]
        fn degenerate_mesh_flat_normals_keep_the_instancing_fallback() {
            let triangle =
                Triangle::new([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]]).unwrap();
            let mesh = Mesh3D::new(vec![triangle]);

            assert!(mesh.flat_normals().unwrap().is_none());
            assert!(mesh.flat_normals().unwrap().is_none());
        }
    }

    /// Per-object transform: scale, then X/Y/Z Euler rotations in radians, then translation.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Transform3D {
        translation: Vec3,
        rotation: Vec3,
        scale: Vec3,
    }

    #[derive(Clone, Copy)]
    pub(crate) struct Transform3DMatrix {
        rows: [[f64; 4]; 3],
    }

    impl Transform3DMatrix {
        pub(crate) fn transform_point(&self, point: [f32; 3]) -> Result<[f32; 3], String> {
            if point.iter().any(|component| !component.is_finite()) {
                return Err("mesh vertices must contain only finite values".into());
            }
            let point = point.map(f64::from);
            let transformed = std::array::from_fn(|axis| {
                self.rows[axis][0] * point[0]
                    + self.rows[axis][1] * point[1]
                    + self.rows[axis][2] * point[2]
                    + self.rows[axis][3]
            });
            if transformed
                .iter()
                .any(|component| !component.is_finite() || component.abs() > f64::from(f32::MAX))
            {
                return Err("transformed mesh vertex is outside the finite f32 range".into());
            }
            Ok(transformed.map(|component| component as f32))
        }

        pub(crate) fn transform_bounds(
            &self,
            bounds: ([f32; 3], [f32; 3]),
        ) -> Result<([f32; 3], [f32; 3]), String> {
            let (local_minimum, local_maximum) = bounds;
            let mut minimum = [f32::INFINITY; 3];
            let mut maximum = [f32::NEG_INFINITY; 3];
            for corner_index in 0..8 {
                let corner = std::array::from_fn(|axis| {
                    if corner_index & (1 << axis) == 0 {
                        local_minimum[axis]
                    } else {
                        local_maximum[axis]
                    }
                });
                let point = self.transform_point(corner)?;
                for axis in 0..3 {
                    minimum[axis] = minimum[axis].min(point[axis]);
                    maximum[axis] = maximum[axis].max(point[axis]);
                }
            }
            Ok((minimum, maximum))
        }

        pub(crate) fn gpu_instance(&self) -> Result<nucleus::GpuInstanceTransform, String> {
            let [a, b, c, tx] = self.rows[0];
            let [d, e, f, ty] = self.rows[1];
            let [g, h, i, tz] = self.rows[2];
            let cofactors = [
                [e * i - f * h, f * g - d * i, d * h - e * g],
                [c * h - b * i, a * i - c * g, b * g - a * h],
                [b * f - c * e, c * d - a * f, a * e - b * d],
            ];
            let determinant = a * cofactors[0][0] + b * cofactors[0][1] + c * cofactors[0][2];
            if !determinant.is_finite() || determinant == 0.0 {
                return Err("3D transform has no finite normal matrix".into());
            }
            let model = [
                [a, d, g, 0.0],
                [b, e, h, 0.0],
                [c, f, i, 0.0],
                [tx, ty, tz, 1.0],
            ];
            let normal: [[f64; 4]; 3] = std::array::from_fn(|column| {
                [
                    cofactors[0][column] / determinant.abs(),
                    cofactors[1][column] / determinant.abs(),
                    cofactors[2][column] / determinant.abs(),
                    0.0,
                ]
            });
            let to_f32 = |value: f64| {
                if value.is_finite() && value.abs() <= f64::from(f32::MAX) {
                    Ok(value as f32)
                } else {
                    Err("3D instance transform exceeds finite GPU range".to_owned())
                }
            };
            let mut model_f32 = [[0.0; 4]; 4];
            for (output_column, column) in model_f32.iter_mut().zip(model) {
                for (output, value) in output_column.iter_mut().zip(column) {
                    *output = to_f32(value)?;
                }
            }
            let mut normal_f32 = [[0.0; 4]; 3];
            for (output_column, column) in normal_f32.iter_mut().zip(normal) {
                for (output, value) in output_column.iter_mut().zip(column) {
                    *output = to_f32(value)?;
                }
            }
            nucleus::GpuInstanceTransform::new(model_f32, normal_f32)
        }
    }

    impl Transform3D {
        pub fn new(translation: Vec3, rotation: Vec3, scale: Vec3) -> Result<Self, String> {
            if [translation, rotation, scale]
                .into_iter()
                .flat_map(|vector| [vector.x, vector.y, vector.z])
                .any(|component| !component.is_finite())
                || scale.x == 0.0
                || scale.y == 0.0
                || scale.z == 0.0
            {
                return Err(
                    "3D transform components must be finite and scale must be nonzero".into(),
                );
            }
            Ok(Self {
                translation,
                rotation,
                scale,
            })
        }

        pub fn transform_point(&self, point: [f32; 3]) -> Result<[f32; 3], String> {
            self.matrix().transform_point(point)
        }

        pub(crate) fn matrix(&self) -> Transform3DMatrix {
            let scale = [self.scale.x, self.scale.y, self.scale.z].map(f64::from);
            let rotation = [self.rotation.x, self.rotation.y, self.rotation.z].map(f64::from);
            let (sin_x, cos_x) = rotation[0].sin_cos();
            let (sin_y, cos_y) = rotation[1].sin_cos();
            let (sin_z, cos_z) = rotation[2].sin_cos();
            let columns: [[f64; 3]; 3] = std::array::from_fn(|axis| {
                let mut point = [0.0; 3];
                point[axis] = scale[axis];
                point = [
                    point[0],
                    point[1] * cos_x - point[2] * sin_x,
                    point[1] * sin_x + point[2] * cos_x,
                ];
                point = [
                    point[0] * cos_y + point[2] * sin_y,
                    point[1],
                    -point[0] * sin_y + point[2] * cos_y,
                ];
                [
                    point[0] * cos_z - point[1] * sin_z,
                    point[0] * sin_z + point[1] * cos_z,
                    point[2],
                ]
            });
            let translation =
                [self.translation.x, self.translation.y, self.translation.z].map(f64::from);
            Transform3DMatrix {
                rows: std::array::from_fn(|axis| {
                    [
                        columns[0][axis],
                        columns[1][axis],
                        columns[2][axis],
                        translation[axis],
                    ]
                }),
            }
        }

        fn sync_translation_to_physics(&mut self, position: Vec3) {
            if position.x.is_finite() && position.y.is_finite() && position.z.is_finite() {
                self.translation = position;
            }
        }
    }

    impl Default for Transform3D {
        fn default() -> Self {
            Self {
                translation: Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                rotation: Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                scale: Vec3 {
                    x: 1.0,
                    y: 1.0,
                    z: 1.0,
                },
            }
        }
    }

    /// A default 3D game object with a material, local mesh, transform, and physics body.
    #[derive(Soa)]
    pub struct GameObject3D {
        material: Material,
        physics_object: PhysicsObject3D,
        mesh: Mesh3D,
        transform: Transform3D,
    }

    impl GameObject3D {
        pub fn new(material: Material, physics_object: PhysicsObject3D) -> Self {
            Self {
                material,
                physics_object,
                mesh: Mesh3D::default(),
                transform: Transform3D::default(),
            }
        }

        pub fn material(&self) -> &Material {
            &self.material
        }

        pub fn physics_object(&self) -> &PhysicsObject3D {
            &self.physics_object
        }

        pub fn physics_object_mut(&mut self) -> &mut PhysicsObject3D {
            &mut self.physics_object
        }

        pub fn mesh(&self) -> &Mesh3D {
            &self.mesh
        }

        pub fn set_mesh(&mut self, mesh: Mesh3D) {
            self.mesh = mesh;
        }

        pub fn transform(&self) -> Transform3D {
            self.transform
        }

        pub fn set_transform(&mut self, transform: Transform3D) {
            self.transform = transform;
        }
    }

    impl Default for GameObject3D {
        fn default() -> Self {
            let texture = Texture::from_rgba8(1, 1, &[255, 255, 255, 255])
                .expect("the built-in white texel is valid");
            let heightmap =
                HeightMap::new(1, 1, vec![0.0]).expect("the built-in flat heightmap is valid");
            Self::new(
                Material::new(texture, heightmap),
                PhysicsObject3D::default(),
            )
        }
    }

    impl GameObjectTrait<Vec3> for GameObject3D {
        fn hitbox(&self) -> &impl crate::physics::hitbox::Hitbox<Vec3> {
            &self.physics_object.hitbox
        }
    }

    impl super::GameObject3D<Vec3> for GameObject3D {
        fn material(&self) -> &Material {
            &self.material
        }

        fn physics_object(&self) -> &PhysicsObject3D {
            &self.physics_object
        }
    }

    impl PhysicsBody for GameObject3D {
        fn integrate_body(&mut self, delta_time: f32) {
            self.physics_object.integrate_body(delta_time);
        }

        fn resolve_body_collision(&mut self, other: &mut Self, friction: f32) -> bool {
            self.physics_object
                .resolve_body_collision(&mut other.physics_object, friction)
        }

        fn broadphase_bounds(&self) -> Option<[(f64, f64); 3]> {
            self.physics_object.broadphase_bounds()
        }
    }

    /// Generic ECS-backed scene used to build custom 2D or 3D scenes.
    pub struct GameScene<V: MathVec, G: GameObjectTrait<V> + Soa> {
        shaders: StableVec<Shader>,
        rtx_on: bool,
        objects: Ecs<G>,
        _marker: PhantomData<fn() -> V>,
    }

    impl<V: MathVec, G: GameObjectTrait<V> + Soa> GameScene<V, G> {
        pub fn new() -> Self {
            Self {
                shaders: StableVec::new(),
                rtx_on: false,
                objects: Ecs::new(),
                _marker: PhantomData,
            }
        }

        pub fn with_capacity(capacity: usize) -> io::Result<Self> {
            Ok(Self {
                shaders: StableVec::new(),
                rtx_on: false,
                objects: Ecs::with_capacity(capacity)?,
                _marker: PhantomData,
            })
        }

        pub fn shaders_mut(&mut self) -> &mut StableVec<Shader> {
            &mut self.shaders
        }

        pub fn set_rtx_on(&mut self, enabled: bool) {
            self.rtx_on = enabled;
        }

        pub fn objects(&self) -> &Ecs<G> {
            &self.objects
        }

        pub fn objects_mut(&mut self) -> &mut Ecs<G> {
            &mut self.objects
        }
    }

    impl<V: MathVec, G: GameObjectTrait<V> + Soa> Default for GameScene<V, G> {
        fn default() -> Self {
            Self::new()
        }
    }

    impl<V: MathVec, G: GameObjectTrait<V> + Soa> Scene<V> for GameScene<V, G> {
        fn shaders(&self) -> &StableVec<Shader> {
            &self.shaders
        }

        fn is_rtx_on(&self) -> bool {
            self.rtx_on
        }
    }

    /// ECS-backed default scene for 2D game objects.
    pub struct GameScene2D {
        shaders: StableVec<Shader>,
        rtx_on: bool,
        objects: Ecs<GameObject2D>,
        physics_stepper: PhysicsStepper<GameObject2D>,
    }

    impl GameScene2D {
        pub fn new() -> Self {
            Self {
                shaders: StableVec::new(),
                rtx_on: false,
                objects: Ecs::new(),
                physics_stepper: PhysicsStepper::new(),
            }
        }

        pub fn with_capacity(capacity: usize) -> io::Result<Self> {
            Ok(Self {
                shaders: StableVec::new(),
                rtx_on: false,
                objects: Ecs::with_capacity(capacity)?,
                physics_stepper: PhysicsStepper::new(),
            })
        }

        pub fn shaders_mut(&mut self) -> &mut StableVec<Shader> {
            &mut self.shaders
        }

        pub fn set_rtx_on(&mut self, enabled: bool) {
            self.rtx_on = enabled;
        }

        pub fn objects(&self) -> &Ecs<GameObject2D> {
            &self.objects
        }

        pub fn objects_mut(&mut self) -> &mut Ecs<GameObject2D> {
            &mut self.objects
        }

        pub(crate) fn try_for_each_object<E>(
            &self,
            mut f: impl FnMut(&Texture, &PhysicsObject2D) -> Result<(), E>,
        ) -> Result<(), E> {
            let columns = self.objects.storage();
            for (texture, physics_object) in columns
                .texture
                .iter()
                .zip(columns.physics_object.iter())
                .take(self.objects.len())
            {
                f(texture, physics_object)?;
            }
            Ok(())
        }

        /// Integrates scene objects and resolves collisions in ECS row order.
        pub fn step_physics(&mut self, delta_time: f32, friction: f32) -> io::Result<usize> {
            self.physics_stepper
                .step(&mut self.objects, delta_time, friction)
        }
    }

    impl Default for GameScene2D {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Scene<Vec2> for GameScene2D {
        fn shaders(&self) -> &StableVec<Shader> {
            &self.shaders
        }

        fn is_rtx_on(&self) -> bool {
            self.rtx_on
        }
    }

    /// ECS-backed default scene for 3D game objects.
    pub struct GameScene3D {
        shaders: StableVec<Shader>,
        rtx_on: bool,
        objects: Ecs<GameObject3D>,
        physics_stepper: PhysicsStepper<GameObject3D>,
    }

    impl GameScene3D {
        pub fn new() -> Self {
            Self {
                shaders: StableVec::new(),
                rtx_on: false,
                objects: Ecs::new(),
                physics_stepper: PhysicsStepper::new(),
            }
        }

        pub fn with_capacity(capacity: usize) -> io::Result<Self> {
            Ok(Self {
                shaders: StableVec::new(),
                rtx_on: false,
                objects: Ecs::with_capacity(capacity)?,
                physics_stepper: PhysicsStepper::new(),
            })
        }

        pub fn shaders_mut(&mut self) -> &mut StableVec<Shader> {
            &mut self.shaders
        }

        pub fn set_rtx_on(&mut self, enabled: bool) {
            self.rtx_on = enabled;
        }

        pub fn objects(&self) -> &Ecs<GameObject3D> {
            &self.objects
        }

        pub fn objects_mut(&mut self) -> &mut Ecs<GameObject3D> {
            &mut self.objects
        }

        /// Integrates scene objects, resolves collisions, and syncs render positions to bodies.
        pub fn step_physics(&mut self, delta_time: f32, friction: f32) -> io::Result<usize> {
            let resolved = self
                .physics_stepper
                .step(&mut self.objects, delta_time, friction)?;
            if !delta_time.is_finite() || delta_time < 0.0 {
                return Ok(resolved);
            }
            let object_count = self.objects.len();
            let columns = self.objects.storage_mut();
            for (physics_object, transform) in columns
                .physics_object
                .iter()
                .zip(columns.transform.iter_mut())
                .take(object_count)
            {
                transform.sync_translation_to_physics(physics_object.position());
            }
            Ok(resolved)
        }

        pub(crate) fn mesh_at(&self, index: usize) -> Option<&Mesh3D> {
            if index >= self.objects.len() {
                return None;
            }
            self.objects.storage().mesh.get(index)
        }

        pub(crate) fn try_for_each_mesh<E>(
            &self,
            mut f: impl FnMut(usize, &Material, &Mesh3D, Transform3D) -> Result<(), E>,
        ) -> Result<(), E> {
            let columns = self.objects.storage();
            for (index, ((material, mesh), transform)) in columns
                .material
                .iter()
                .zip(columns.mesh.iter())
                .zip(columns.transform.iter())
                .take(self.objects.len())
                .enumerate()
            {
                f(index, material, mesh, *transform)?;
            }
            Ok(())
        }
    }

    impl Default for GameScene3D {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Scene<Vec3> for GameScene3D {
        fn shaders(&self) -> &StableVec<Shader> {
            &self.shaders
        }

        fn is_rtx_on(&self) -> bool {
            self.rtx_on
        }
    }
}
