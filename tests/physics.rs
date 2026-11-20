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

use vagus::essential::collections::StableVec;
use vagus::math::{Vec2, Vec3};
use vagus::physics::hitbox::{Hitbox, Hitbox2D, Hitbox3D};
use vagus::physics::{PhysicsObject2D, PhysicsObject3D, PhysicsWorld2D, PhysicsWorld3D};

fn point(x: f32, y: f32) -> Vec2 {
    Vec2 { x, y }
}

fn point3(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3 { x, y, z }
}

fn box_at(x: f32, y: f32, width: f32, height: f32) -> Hitbox2D {
    Hitbox2D::rectangular(
        point(x, y),
        point(x + width, y),
        point(x + width, y + height),
        point(x, y + height),
    )
}

fn cube_at(x: f32, y: f32, z: f32, half: f32) -> Hitbox3D {
    Hitbox3D::Quad(
        point3(-half, -half, -half),
        point3(half, -half, -half),
        point3(half, half, -half),
        point3(-half, half, -half),
        point3(-half, -half, half),
        point3(half, -half, half),
        point3(half, half, half),
        point3(-half, half, half),
        point3(x, y, z),
    )
}

#[test]
fn intersects_all_shape_pairs_and_separates_overlapping_boxes() {
    let polygon = Hitbox2D::polygon(StableVec::from([
        point(0.0, 0.0),
        point(2.0, 0.0),
        point(2.0, 2.0),
        point(0.0, 2.0),
    ]));
    let triangle = Hitbox2D::triangular(point(0.0, 0.0), point(2.0, 0.0), point(1.0, 2.0));
    let circle = Hitbox2D::circle(1.0, point(0.0, 0.0));
    let box_shape = box_at(-0.5, -0.5, 1.0, 1.0);

    for (left, right) in [
        (&box_shape, &triangle),
        (&box_shape, &circle),
        (&box_shape, &polygon),
        (&triangle, &circle),
        (&triangle, &polygon),
        (&circle, &polygon),
    ] {
        assert!(left.intersects(right), "{left:?} vs {right:?}");
        assert!(right.intersects(left), "{right:?} vs {left:?}");
    }

    let left = box_at(0.0, 0.0, 2.0, 2.0);
    let right = box_at(1.5, 0.0, 2.0, 2.0);
    let displacement = left.separation(&right);
    assert!((displacement.x + 0.5).abs() < 1e-5);
    assert!(displacement.y.abs() < 1e-5);
}

#[test]
fn large_finite_polygons_still_detect_overlap() {
    let large = box_at(-1.0e20, -1.0e20, 2.0e20, 2.0e20);

    assert!(large.intersects(&large));
}

#[test]
fn separated_maximum_scale_2d_polygons_do_not_overlap_from_projection_overflow() {
    let radius = f32::MAX * 0.05;
    let diamond = |center| {
        Hitbox2D::polygon(StableVec::from([
            point(center, center - radius),
            point(center + radius, center),
            point(center, center + radius),
            point(center - radius, center),
        ]))
    };
    let first = diamond(f32::MAX * 0.8);
    let separated = diamond(f32::MAX * 0.88);

    assert!(first.intersects(&first));
    assert!(!first.intersects(&separated));
}

#[test]
fn intersection_handles_touching_disjoint_and_invalid_shapes() {
    let unit = box_at(0.0, 0.0, 1.0, 1.0);
    assert!(unit.intersects(&box_at(1.0, 0.0, 1.0, 1.0)));
    assert!(!unit.intersects(&box_at(1.01, 0.0, 1.0, 1.0)));
    assert!(!unit.intersects(&Hitbox2D::circle(-1.0, point(0.0, 0.0))));
    assert!(!unit.intersects(&Hitbox2D::polygon(StableVec::default())));
    assert_eq!(
        unit.separation(&box_at(2.0, 0.0, 1.0, 1.0)),
        point(0.0, 0.0)
    );
}

#[test]
fn circles_use_their_centers_and_contained_shapes_get_full_separation() {
    let first = Hitbox2D::circle(1.0, point(0.0, 0.0));
    let touching = Hitbox2D::circle(1.0, point(2.0, 0.0));
    let distant = Hitbox2D::circle(1.0, point(2.01, 0.0));
    assert!(first.intersects(&touching));
    assert!(!first.intersects(&distant));

    let outer = box_at(-2.0, -2.0, 4.0, 4.0);
    let inner = box_at(-0.5, -0.5, 1.0, 1.0);
    let displacement = inner.separation(&outer);
    assert!((displacement.x.abs() - 2.5).abs() < 1e-5 || (displacement.y.abs() - 2.5).abs() < 1e-5);
}

#[test]
fn rectangular_origin_offsets_local_vertices() {
    let rect = Hitbox2D::rectangular_at(
        point(0.0, 0.0),
        point(1.0, 0.0),
        point(1.0, 1.0),
        point(0.0, 1.0),
        point(10.0, 10.0),
    );
    assert!(rect.intersects(&Hitbox2D::circle(0.25, point(10.5, 10.5))));
    assert!(!rect.intersects(&Hitbox2D::circle(0.25, point(0.5, 0.5))));

    let triangle = Hitbox2D::triangular_at(
        point(0.0, 0.0),
        point(1.0, 0.0),
        point(0.5, 1.0),
        point(10.0, 10.0),
    );
    assert!(triangle.intersects(&Hitbox2D::circle(0.1, point(10.5, 10.25))));

    let polygon = Hitbox2D::polygon_at(
        StableVec::from([
            point(0.0, 0.0),
            point(1.0, 0.0),
            point(1.0, 1.0),
            point(0.0, 1.0),
        ]),
        point(10.0, 10.0),
    );
    assert!(polygon.intersects(&Hitbox2D::circle(0.1, point(10.5, 10.5))));
}

#[test]
fn three_dimensional_hitboxes_intersect_and_separate() {
    let sphere = Hitbox3D::Sphere(1.0, point3(0.0, 0.0, 0.0));
    let touching = Hitbox3D::Sphere(1.0, point3(2.0, 0.0, 0.0));
    let distant = Hitbox3D::Sphere(1.0, point3(2.01, 0.0, 0.0));
    assert!(sphere.intersects(&touching));
    assert!(!sphere.intersects(&distant));

    let first = cube_at(0.0, 0.0, 0.0, 1.0);
    let overlap = cube_at(1.5, 0.0, 0.0, 1.0);
    let displacement = first.separation(&overlap);
    assert!((displacement.x + 0.5).abs() < 1e-5);
    assert!(displacement.y.abs() < 1e-5 && displacement.z.abs() < 1e-5);
    let near_sphere = Hitbox3D::Sphere(0.25, point3(1.1, 0.0, 0.0));
    assert!(first.intersects(&near_sphere));
    let sphere_escape = near_sphere.separation(&first);
    assert!((sphere_escape.x - 0.15).abs() < 1e-5);
    assert!(sphere_escape.y.abs() < 1e-5 && sphere_escape.z.abs() < 1e-5);
    let box_escape = first.separation(&near_sphere);
    assert!((box_escape.x + 0.15).abs() < 1e-5);

    let contained = Hitbox3D::Sphere(0.25, point3(0.0, 0.0, 0.0));
    let escape = contained.separation(&first);
    let escape_length = (escape.x * escape.x + escape.y * escape.y + escape.z * escape.z).sqrt();
    assert!((escape_length - 1.25).abs() < 1e-5);
}

#[test]
fn large_finite_3d_polygons_keep_overlap_and_separation_finite() {
    let large = cube_at(0.0, 0.0, 0.0, 1.0e20);
    let overlapping = cube_at(1.5e20, 0.0, 0.0, 1.0e20);
    let separated = cube_at(3.0e20, 0.0, 0.0, 1.0e20);

    assert!(large.intersects(&large));
    assert!(large.intersects(&overlapping));
    let separation = large.separation(&overlapping);
    let distance = f64::from(separation.x)
        .hypot(f64::from(separation.y))
        .hypot(f64::from(separation.z));
    assert!((distance - 5.0e19).abs() / 5.0e19 < 1e-5);
    assert!(separation.x.is_finite() && separation.y.is_finite() && separation.z.is_finite());
    assert!(!large.intersects(&separated));
}

#[test]
fn near_maximum_finite_3d_polygons_keep_sat_arithmetic_finite() {
    let large = cube_at(0.0, 0.0, 0.0, 2.0e38);
    let overlapping = cube_at(1.0e38, 0.0, 0.0, 2.0e38);
    let separated = cube_at(3.0e38, 0.0, 0.0, 1.0e37);

    assert!(large.intersects(&large));
    assert_eq!(large.separation(&large), point3(0.0, 0.0, 0.0));
    assert!(large.intersects(&overlapping));
    let separation = large.separation(&overlapping);
    assert!(separation.x.is_finite() && separation.y.is_finite() && separation.z.is_finite());
    let distance = f64::from(separation.x)
        .hypot(f64::from(separation.y))
        .hypot(f64::from(separation.z));
    assert!((distance - 3.0e38).abs() / 3.0e38 < 1e-5);
    assert!(!large.intersects(&separated));
}

#[test]
fn large_finite_spheres_keep_overlap_and_separation_finite() {
    let first = Hitbox3D::sphere(1.0e20, point3(0.0, 0.0, 0.0));
    let second = Hitbox3D::sphere(1.0e20, point3(1.5e20, 0.0, 0.0));

    assert!(first.intersects(&second));
    let separation = first.separation(&second);
    assert!(separation.x.is_finite());
    assert!((separation.x + 5.0e19).abs() / 5.0e19 < 1e-5);
    assert_eq!(separation.y, 0.0);
    assert_eq!(separation.z, 0.0);

    let distant = Hitbox3D::sphere(1.0e20, point3(3.0e20, 0.0, 0.0));
    assert!(!first.intersects(&distant));
    assert_eq!(first.separation(&distant), point3(0.0, 0.0, 0.0));

    let unrepresentable = Hitbox3D::sphere(f32::MAX, point3(0.0, 0.0, 0.0));
    assert!(unrepresentable.intersects(&unrepresentable));
    assert_eq!(
        unrepresentable.separation(&unrepresentable),
        point3(0.0, 0.0, 0.0)
    );
}

#[test]
fn large_spheres_against_convex_polygons_keep_overlap_and_separation_finite() {
    let cube = cube_at(0.0, 0.0, 0.0, 1.0e20);
    let overlapping = Hitbox3D::sphere(1.0e19, point3(1.05e20, 0.0, 0.0));

    assert!(overlapping.intersects(&cube));
    assert!(cube.intersects(&overlapping));
    let separation = overlapping.separation(&cube);
    assert!(separation.x.is_finite() && separation.y.is_finite() && separation.z.is_finite());
    assert!((separation.x - 5.0e18).abs() / 5.0e18 < 1e-5);
    assert_eq!(separation.y, 0.0);
    assert_eq!(separation.z, 0.0);
    assert_eq!(
        cube.separation(&overlapping),
        point3(-separation.x, 0.0, 0.0)
    );

    let separated = Hitbox3D::sphere(1.0e19, point3(1.2e20, 0.0, 0.0));
    assert!(!separated.intersects(&cube));
}

#[test]
fn three_dimensional_objects_apply_origins_and_reject_separated_spheres() {
    let tetrahedron = Hitbox3D::Object(
        StableVec::from([
            point3(0.0, 0.0, 0.0),
            point3(1.0, 0.0, 0.0),
            point3(0.0, 1.0, 0.0),
            point3(0.0, 0.0, 1.0),
        ]),
        point3(10.0, 0.0, 0.0),
    );
    assert!(tetrahedron.intersects(&Hitbox3D::Sphere(0.05, point3(10.2, 0.2, 0.2))));
    assert!(!tetrahedron.intersects(&Hitbox3D::Sphere(0.05, point3(10.9, 0.2, 0.2))));
    assert_eq!(
        tetrahedron.separation(&Hitbox3D::Sphere(0.05, point3(12.0, 0.0, 0.0))),
        point3(0.0, 0.0, 0.0)
    );

    let flat_object = Hitbox3D::Object(
        StableVec::from([
            point3(0.0, 0.0, 0.0),
            point3(1.0, 0.0, 0.0),
            point3(0.0, 1.0, 0.0),
            point3(1.0, 1.0, 0.0),
        ]),
        point3(0.0, 0.0, 0.0),
    );
    assert!(!flat_object.intersects(&Hitbox3D::Sphere(1.0, point3(0.5, 0.5, 0.0))));

    let tiny_cube = cube_at(0.0, 0.0, 0.0, 0.001);
    assert!(!tiny_cube.intersects(&Hitbox3D::Sphere(0.0, point3(0.001005, 0.0, 0.0))));
}

#[test]
fn three_dimensional_hitbox_constructors_and_defaults_are_usable() {
    let sphere = Hitbox3D::sphere(0.5, point3(5.5, 0.5, 0.5));
    let quad = Hitbox3D::quad_at(
        point3(0.0, 0.0, 0.0),
        point3(1.0, 0.0, 0.0),
        point3(1.0, 1.0, 0.0),
        point3(0.0, 1.0, 0.0),
        point3(0.0, 0.0, 1.0),
        point3(1.0, 0.0, 1.0),
        point3(1.0, 1.0, 1.0),
        point3(0.0, 1.0, 1.0),
        point3(5.0, 0.0, 0.0),
    );
    assert!(quad.intersects(&sphere));

    let object = Hitbox3D::object_at(
        StableVec::from([
            point3(0.0, 0.0, 0.0),
            point3(1.0, 0.0, 0.0),
            point3(0.0, 1.0, 0.0),
            point3(0.0, 0.0, 1.0),
        ]),
        point3(5.0, 0.0, 0.0),
    );
    assert!(object.intersects(&sphere));
    assert!(Hitbox3D::default().intersects(&Hitbox3D::sphere(0.1, point3(0.0, 0.0, 0.0))));
}

#[test]
fn physics_objects_integrate_dimension_correct_velocity() {
    let mut object_2d =
        PhysicsObject2D::new(box_at(0.0, 0.0, 1.0, 1.0), 2.0, 0.5, 0.0, point(2.0, -1.0));
    object_2d.integrate(0.5);
    assert!(
        object_2d
            .hitbox
            .intersects(&Hitbox2D::circle(0.1, point(1.5, 0.0)))
    );

    let mut object_3d = PhysicsObject3D::new(
        cube_at(0.0, 0.0, 0.0, 1.0),
        3.0,
        0.5,
        0.0,
        point3(1.0, 2.0, 3.0),
    );
    object_3d.integrate(0.5);
    assert!(
        object_3d
            .hitbox
            .intersects(&Hitbox3D::sphere(0.1, point3(0.5, 1.0, 1.5)))
    );

    let before = object_3d.hitbox.clone();
    object_3d.integrate(f32::NAN);
    assert_eq!(object_3d.hitbox, before);
}

#[test]
fn physics_world_replaces_bodies_and_returns_input_for_missing_rows() {
    let original = PhysicsObject2D::new(
        Hitbox2D::circle(0.5, point(0.0, 0.0)),
        1.0,
        0.5,
        0.0,
        point(1.0, 0.0),
    );
    let missing = PhysicsObject2D::new(
        Hitbox2D::circle(0.25, point(-3.0, -2.0)),
        3.0,
        0.25,
        0.2,
        point(0.0, 1.0),
    );
    let mut world = PhysicsWorld2D::with_growable_capacity(1).unwrap();
    assert_eq!(world.add(original.clone()).unwrap(), 0);

    let mut replacement = world.get_cloned(0).unwrap();
    assert!(replacement.apply_impulse(point(-2.0, 2.0)));
    assert_eq!(replacement.velocity, point(-1.0, 2.0));
    assert_eq!(world.try_replace(0, replacement.clone()), Ok(original));
    assert_eq!(world.get_cloned(0), Some(replacement.clone()));
    assert_eq!(world.try_replace(1, missing.clone()), Err(missing));
    assert_eq!(world.get_cloned(0), Some(replacement));
    assert_eq!(world.len(), 1);
}

#[test]
fn two_dimensional_hitbox_and_physics_object_report_world_origin() {
    let circle_center = point(4.0, -5.0);
    let circle = PhysicsObject2D::new(
        Hitbox2D::circle(0.5, circle_center),
        1.0,
        0.5,
        0.0,
        point(0.0, 0.0),
    );
    assert_eq!(circle.hitbox.origin(), circle_center);
    assert_eq!(circle.position(), circle_center);

    let origin = point(-2.0, 3.0);
    let rectangle = PhysicsObject2D::new(
        Hitbox2D::rectangular_at(
            point(-1.0, -1.0),
            point(1.0, -1.0),
            point(1.0, 1.0),
            point(-1.0, 1.0),
            origin,
        ),
        1.0,
        0.5,
        0.0,
        point(0.0, 0.0),
    );
    assert_eq!(rectangle.hitbox.origin(), origin);
    assert_eq!(rectangle.position(), origin);
}

#[test]
fn three_dimensional_hitbox_and_physics_object_report_world_origin() {
    let sphere_center = point3(4.0, -5.0, 6.0);
    let sphere = PhysicsObject3D::new(
        Hitbox3D::sphere(0.5, sphere_center),
        1.0,
        0.5,
        0.0,
        point3(0.0, 0.0, 0.0),
    );
    assert_eq!(sphere.hitbox.origin(), sphere_center);
    assert_eq!(sphere.position(), sphere_center);

    let origin = point3(-2.0, 3.0, -4.0);
    let cube = PhysicsObject3D::new(
        cube_at(origin.x, origin.y, origin.z, 1.0),
        1.0,
        0.5,
        0.0,
        point3(0.0, 0.0, 0.0),
    );
    assert_eq!(cube.hitbox.origin(), origin);
    assert_eq!(cube.position(), origin);
}

#[test]
fn negative_physics_time_steps_do_not_reverse_or_resolve_motion() {
    let mut object_2d = PhysicsObject2D::new(
        Hitbox2D::circle(1.0, point(0.0, 0.0)),
        1.0,
        1.0,
        0.0,
        point(2.0, 0.0),
    );
    let before_2d = object_2d.clone();
    object_2d.integrate(-0.5);
    assert_eq!(object_2d, before_2d);

    let mut object_3d = PhysicsObject3D::new(
        Hitbox3D::sphere(1.0, point3(0.0, 0.0, 0.0)),
        1.0,
        1.0,
        0.0,
        point3(2.0, 0.0, 0.0),
    );
    let before_3d = object_3d.clone();
    object_3d.integrate(-0.5);
    assert_eq!(object_3d, before_3d);

    let mut world_2d = PhysicsWorld2D::with_growable_capacity(2).unwrap();
    world_2d
        .add(PhysicsObject2D::new(
            Hitbox2D::circle(1.0, point(-0.9, 0.0)),
            1.0,
            1.0,
            0.0,
            point(0.0, 0.0),
        ))
        .unwrap();
    world_2d
        .add(PhysicsObject2D::new(
            Hitbox2D::circle(1.0, point(0.9, 0.0)),
            1.0,
            1.0,
            0.0,
            point(0.0, 0.0),
        ))
        .unwrap();
    let before_world_2d = [
        world_2d.get_cloned(0).unwrap(),
        world_2d.get_cloned(1).unwrap(),
    ];
    assert_eq!(world_2d.step(-0.1, 0.0).unwrap(), 0);
    assert_eq!(
        [
            world_2d.get_cloned(0).unwrap(),
            world_2d.get_cloned(1).unwrap()
        ],
        before_world_2d
    );

    let mut world_3d = PhysicsWorld3D::with_growable_capacity(2).unwrap();
    world_3d
        .add(PhysicsObject3D::new(
            Hitbox3D::sphere(1.0, point3(-0.9, 0.0, 0.0)),
            1.0,
            1.0,
            0.0,
            point3(0.0, 0.0, 0.0),
        ))
        .unwrap();
    world_3d
        .add(PhysicsObject3D::new(
            Hitbox3D::sphere(1.0, point3(0.9, 0.0, 0.0)),
            1.0,
            1.0,
            0.0,
            point3(0.0, 0.0, 0.0),
        ))
        .unwrap();
    let before_world_3d = [
        world_3d.get_cloned(0).unwrap(),
        world_3d.get_cloned(1).unwrap(),
    ];
    assert_eq!(world_3d.step(-0.1, 0.0).unwrap(), 0);
    assert_eq!(
        [
            world_3d.get_cloned(0).unwrap(),
            world_3d.get_cloned(1).unwrap()
        ],
        before_world_3d
    );
}

#[test]
fn physics_world_integrate_all_ignores_non_finite_steps_without_mutation() {
    let mut world_2d = PhysicsWorld2D::with_growable_capacity(1).unwrap();
    world_2d
        .add(PhysicsObject2D::new(
            Hitbox2D::circle(1.0, point(0.0, 0.0)),
            1.0,
            1.0,
            0.0,
            point(2.0, -3.0),
        ))
        .unwrap();
    let before_2d = world_2d.get_cloned(0).unwrap();

    let mut world_3d = PhysicsWorld3D::with_growable_capacity(1).unwrap();
    world_3d
        .add(PhysicsObject3D::new(
            Hitbox3D::sphere(1.0, point3(0.0, 0.0, 0.0)),
            1.0,
            1.0,
            0.0,
            point3(2.0, -3.0, 4.0),
        ))
        .unwrap();
    let before_3d = world_3d.get_cloned(0).unwrap();

    for delta_time in [f32::NAN, f32::INFINITY] {
        world_2d.integrate_all(delta_time).unwrap();
        world_3d.integrate_all(delta_time).unwrap();
        assert_eq!(world_2d.get_cloned(0).unwrap(), before_2d);
        assert_eq!(world_3d.get_cloned(0).unwrap(), before_3d);
    }
}

#[test]
fn ecs_backed_physics_world_grows_removes_and_integrates_2d_bodies() {
    let mut world = PhysicsWorld2D::with_growable_capacity(1).unwrap();
    assert!(world.is_empty());

    let moving = world
        .add(PhysicsObject2D::new(
            Hitbox2D::circle(0.25, point(0.0, 0.0)),
            2.0,
            0.5,
            0.0,
            point(2.0, -1.0),
        ))
        .unwrap();
    let stationary = world
        .add(PhysicsObject2D::new(
            Hitbox2D::circle(0.25, point(5.0, 0.0)),
            0.0,
            0.5,
            0.0,
            point(3.0, 4.0),
        ))
        .unwrap();
    assert_eq!((moving, stationary), (0, 1));
    assert!(world.capacity() > 1);

    world.integrate_all(0.5).unwrap();
    let moved = world.get_cloned(moving).unwrap();
    assert!(
        moved
            .hitbox
            .intersects(&Hitbox2D::circle(0.01, point(1.0, -0.5)))
    );

    let removed = world.remove(moving).unwrap();
    assert_eq!(removed.mass, 2.0);
    assert_eq!(world.len(), 1);
    let stationary = world.get_cloned(0).unwrap();
    assert!(
        stationary
            .hitbox
            .intersects(&Hitbox2D::circle(0.01, point(5.0, 0.0)))
    );
    assert_eq!(stationary.velocity, point(0.0, 0.0));
}

#[test]
fn ecs_backed_physics_world_integrates_3d_bodies() {
    let mut world = PhysicsWorld3D::new();
    world
        .add(PhysicsObject3D::new(
            Hitbox3D::sphere(0.25, point3(0.0, 0.0, 0.0)),
            3.0,
            0.5,
            0.0,
            point3(1.0, 2.0, 3.0),
        ))
        .unwrap();

    world.integrate_all(0.5).unwrap();
    let moved = world.get_cloned(0).unwrap();
    assert!(
        moved
            .hitbox
            .intersects(&Hitbox3D::sphere(0.01, point3(0.5, 1.0, 1.5)))
    );
}

#[test]
fn ecs_backed_physics_world_step_resolves_2d_collisions() {
    let mut world = PhysicsWorld2D::with_growable_capacity(2).unwrap();
    world
        .add(PhysicsObject2D::new(
            Hitbox2D::circle(1.0, point(-1.1, 0.0)),
            1.0,
            1.0,
            0.0,
            point(1.0, 0.0),
        ))
        .unwrap();
    world
        .add(PhysicsObject2D::new(
            Hitbox2D::circle(1.0, point(1.1, 0.0)),
            1.0,
            1.0,
            0.0,
            point(-1.0, 0.0),
        ))
        .unwrap();

    assert_eq!(world.step(0.2, 0.0).unwrap(), 1);
    let left = world.get_cloned(0).unwrap();
    let right = world.get_cloned(1).unwrap();
    assert_eq!(left.velocity, point(-1.0, 0.0));
    assert_eq!(right.velocity, point(1.0, 0.0));
    assert_eq!(left.hitbox.separation(&right.hitbox), point(0.0, 0.0));
}

#[test]
fn ecs_backed_physics_world_prevents_fast_2d_circle_tunneling() {
    let mut world = PhysicsWorld2D::with_growable_capacity(2).unwrap();
    world
        .add(PhysicsObject2D::new(
            Hitbox2D::circle(1.0, point(0.0, 0.0)),
            0.0,
            1.0,
            0.0,
            point(0.0, 0.0),
        ))
        .unwrap();
    world
        .add(PhysicsObject2D::new(
            Hitbox2D::circle(0.5, point(-5.0, 0.0)),
            1.0,
            1.0,
            0.0,
            point(10.0, 0.0),
        ))
        .unwrap();

    assert_eq!(world.step(1.0, 0.0).unwrap(), 1);
    let moving = world.get_cloned(1).unwrap();
    assert!(moving.velocity.x < 0.0);
    assert!(moving.hitbox.origin().x < 0.0);
}

#[test]
fn ecs_backed_physics_world_rejects_tunneling_point_body_atomically() {
    let mut world = PhysicsWorld2D::with_growable_capacity(2).unwrap();
    world
        .add(PhysicsObject2D::new(
            box_at(-0.05, -1.0, 0.1, 2.0),
            0.0,
            1.0,
            0.0,
            point(0.0, 0.0),
        ))
        .unwrap();
    let wall_before = world.get_cloned(0).unwrap();
    world
        .add(PhysicsObject2D::new(
            Hitbox2D::circle(0.0, point(-5.0, 0.0)),
            1.0,
            1.0,
            0.0,
            point(10.0, 0.0),
        ))
        .unwrap();
    let before = world.get_cloned(1).unwrap();

    let error = world.step(1.0, 0.0).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(world.get_cloned(0).unwrap(), wall_before);
    assert_eq!(world.get_cloned(1).unwrap(), before);
}

#[test]
fn ecs_backed_physics_world_prevents_fast_3d_sphere_tunneling() {
    let mut world = PhysicsWorld3D::with_growable_capacity(2).unwrap();
    world
        .add(PhysicsObject3D::new(
            Hitbox3D::sphere(1.0, point3(0.0, 0.0, 0.0)),
            0.0,
            1.0,
            0.0,
            point3(0.0, 0.0, 0.0),
        ))
        .unwrap();
    world
        .add(PhysicsObject3D::new(
            Hitbox3D::sphere(0.5, point3(-5.0, 0.0, 0.0)),
            1.0,
            1.0,
            0.0,
            point3(10.0, 0.0, 0.0),
        ))
        .unwrap();

    assert!(world.step(1.0, 0.0).unwrap() >= 1);
    let moving = world.get_cloned(1).unwrap();
    assert!(moving.velocity.x < 0.0);
    assert!(moving.hitbox.origin().x < 0.0);
}

#[test]
fn ecs_backed_physics_world_stops_fast_sphere_at_3d_static_box() {
    let mut world = PhysicsWorld3D::with_growable_capacity(2).unwrap();
    world
        .add(PhysicsObject3D::new(
            cube_at(0.0, 0.0, 0.0, 1.0),
            0.0,
            1.0,
            0.0,
            point3(0.0, 0.0, 0.0),
        ))
        .unwrap();
    world
        .add(PhysicsObject3D::new(
            Hitbox3D::sphere(0.5, point3(-5.0, 0.0, 0.0)),
            1.0,
            1.0,
            0.0,
            point3(10.0, 0.0, 0.0),
        ))
        .unwrap();

    assert_eq!(world.step(1.0, 0.0).unwrap(), 1);
    let moving = world.get_cloned(1).unwrap();
    assert!(moving.velocity.x < 0.0);
    assert!(moving.hitbox.origin().x < -1.0);
}

#[test]
fn ecs_backed_physics_world_step_resolves_3d_collisions() {
    let mut world = PhysicsWorld3D::with_growable_capacity(2).unwrap();
    world
        .add(PhysicsObject3D::new(
            Hitbox3D::sphere(1.0, point3(-1.1, 0.0, 0.0)),
            1.0,
            1.0,
            0.0,
            point3(1.0, 0.0, 0.0),
        ))
        .unwrap();
    world
        .add(PhysicsObject3D::new(
            Hitbox3D::sphere(1.0, point3(1.1, 0.0, 0.0)),
            1.0,
            1.0,
            0.0,
            point3(-1.0, 0.0, 0.0),
        ))
        .unwrap();

    assert_eq!(world.step(0.2, 0.0).unwrap(), 1);
    let left = world.get_cloned(0).unwrap();
    let right = world.get_cloned(1).unwrap();
    assert_eq!(left.velocity, point3(-1.0, 0.0, 0.0));
    assert_eq!(right.velocity, point3(1.0, 0.0, 0.0));
    assert_eq!(left.hitbox.separation(&right.hitbox), point3(0.0, 0.0, 0.0));
}

#[test]
fn ecs_backed_physics_world_step_rejects_invalid_friction_atomically() {
    let mut world = PhysicsWorld2D::new();
    world
        .add(PhysicsObject2D::new(
            Hitbox2D::circle(1.0, point(0.0, 0.0)),
            1.0,
            1.0,
            0.0,
            point(1.0, 0.0),
        ))
        .unwrap();
    let before = world.get_cloned(0).unwrap();

    assert!(world.step(0.5, f32::NAN).is_err());
    assert_eq!(world.get_cloned(0).unwrap(), before);
}

#[test]
fn ecs_backed_physics_world_rejects_excessive_substeps_atomically() {
    let mut world = PhysicsWorld2D::with_growable_capacity(1).unwrap();
    world
        .add(PhysicsObject2D::new(
            Hitbox2D::circle(0.5, point(0.0, 0.0)),
            1.0,
            1.0,
            0.0,
            point(10_000.0, 0.0),
        ))
        .unwrap();
    let before = world.get_cloned(0).unwrap();

    let error = world.step(1.0, 0.0).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(world.get_cloned(0).unwrap(), before);
}

#[test]
fn physics_objects_with_zero_mass_remain_stationary_during_integration() {
    let initial_2d = box_at(4.0, -3.0, 1.0, 1.0);
    let mut static_2d = PhysicsObject2D::new(initial_2d.clone(), 0.0, 0.5, 0.0, point(3.0, -2.0));
    static_2d.integrate(2.0);
    assert_eq!(static_2d.hitbox, initial_2d);
    assert_eq!(static_2d.velocity, point(0.0, 0.0));

    let initial_3d = cube_at(4.0, -3.0, 2.0, 1.0);
    let mut static_3d =
        PhysicsObject3D::new(initial_3d.clone(), 0.0, 0.5, 0.0, point3(3.0, -2.0, 1.0));
    static_3d.integrate(2.0);
    assert_eq!(static_3d.hitbox, initial_3d);
    assert_eq!(static_3d.velocity, point3(0.0, 0.0, 0.0));
}

#[test]
fn physics_integration_ignores_invalid_masses_without_mutation() {
    for mass in [-1.0, f64::NAN, f64::INFINITY] {
        let mut object_2d =
            PhysicsObject2D::new(box_at(0.0, 0.0, 1.0, 1.0), mass, 0.5, 0.0, point(2.0, -1.0));
        let before_hitbox_2d = object_2d.hitbox.clone();
        let before_velocity_2d = object_2d.velocity;
        object_2d.integrate(0.5);
        assert_eq!(object_2d.hitbox, before_hitbox_2d);
        assert_eq!(object_2d.velocity, before_velocity_2d);

        let mut object_3d = PhysicsObject3D::new(
            cube_at(0.0, 0.0, 0.0, 1.0),
            mass,
            0.5,
            0.0,
            point3(1.0, 2.0, 3.0),
        );
        let before_hitbox_3d = object_3d.hitbox.clone();
        let before_velocity_3d = object_3d.velocity;
        object_3d.integrate(0.5);
        assert_eq!(object_3d.hitbox, before_hitbox_3d);
        assert_eq!(object_3d.velocity, before_velocity_3d);
    }
}

#[test]
fn physics_object_2d_applies_impulses_using_mass_atomically() {
    let mut object =
        PhysicsObject2D::new(box_at(0.0, 0.0, 1.0, 1.0), 2.0, 0.5, 0.0, point(1.0, -2.0));
    assert!(object.apply_impulse(point(4.0, 2.0)));
    assert_eq!(object.velocity, point(3.0, -1.0));

    let before = object.velocity;
    assert!(!object.apply_impulse(point(f32::NAN, 1.0)));
    assert_eq!(object.velocity, before);

    let mut invalid_mass =
        PhysicsObject2D::new(box_at(0.0, 0.0, 1.0, 1.0), 0.0, 0.5, 0.0, point(0.0, 0.0));
    assert!(!invalid_mass.apply_impulse(point(1.0, 0.0)));
    assert_eq!(invalid_mass.velocity, point(0.0, 0.0));

    let mut overflowing = PhysicsObject2D::new(
        box_at(0.0, 0.0, 1.0, 1.0),
        1.0,
        0.5,
        0.0,
        point(f32::MAX, 0.0),
    );
    assert!(!overflowing.apply_impulse(point(f32::MAX, 0.0)));
    assert_eq!(overflowing.velocity, point(f32::MAX, 0.0));
}

#[test]
fn physics_object_2d_applies_force_over_a_timestep_using_mass() {
    let mut object =
        PhysicsObject2D::new(box_at(0.0, 0.0, 1.0, 1.0), 2.0, 0.5, 0.0, point(1.0, -2.0));
    assert!(object.apply_force(point(4.0, 2.0), 0.5));
    assert_eq!(object.velocity, point(2.0, -1.5));

    let before = object.velocity;
    assert!(!object.apply_force(point(1.0, f32::NAN), 0.5));
    assert_eq!(object.velocity, before);
    assert!(!object.apply_force(point(1.0, 1.0), -0.1));
    assert_eq!(object.velocity, before);
}

#[test]
fn physics_object_2d_resolves_circle_collision_by_mass_and_restitution() {
    let mut left = PhysicsObject2D::new(
        Hitbox2D::circle(1.0, point(-0.5, 0.0)),
        1.0,
        1.0,
        0.0,
        point(1.0, 0.0),
    );
    let mut right = PhysicsObject2D::new(
        Hitbox2D::circle(1.0, point(0.5, 0.0)),
        3.0,
        1.0,
        0.0,
        point(-1.0, 0.0),
    );

    assert!(left.resolve_collision(&mut right, 0.0));
    assert_eq!(left.velocity, point(-2.0, 0.0));
    assert_eq!(right.velocity, point(0.0, 0.0));
    assert_eq!(left.hitbox.separation(&right.hitbox), point(0.0, 0.0));
}

#[test]
fn physics_object_3d_applies_impulses_using_mass_atomically() {
    let mut object = PhysicsObject3D::new(
        cube_at(0.0, 0.0, 0.0, 1.0),
        2.0,
        0.5,
        0.0,
        point3(1.0, -2.0, 3.0),
    );
    assert!(object.apply_impulse(point3(4.0, 2.0, -6.0)));
    assert_eq!(object.velocity, point3(3.0, -1.0, 0.0));

    let before = object.velocity;
    assert!(!object.apply_impulse(point3(1.0, f32::NAN, 1.0)));
    assert_eq!(object.velocity, before);

    let mut overflowing = PhysicsObject3D::new(
        cube_at(0.0, 0.0, 0.0, 1.0),
        1.0,
        0.5,
        0.0,
        point3(1.0, 2.0, f32::MAX),
    );
    assert!(!overflowing.apply_impulse(point3(1.0, 1.0, f32::MAX)));
    assert_eq!(overflowing.velocity, point3(1.0, 2.0, f32::MAX));
}

#[test]
fn physics_object_3d_applies_force_over_a_timestep_using_mass() {
    let mut object = PhysicsObject3D::new(
        cube_at(0.0, 0.0, 0.0, 1.0),
        4.0,
        0.5,
        0.0,
        point3(-1.0, 2.0, 0.0),
    );
    assert!(object.apply_force(point3(8.0, -4.0, 12.0), 0.5));
    assert_eq!(object.velocity, point3(0.0, 1.5, 1.5));

    let before = object.velocity;
    assert!(!object.apply_force(point3(1.0, 1.0, 1.0), f32::INFINITY));
    assert_eq!(object.velocity, before);
}

#[test]
fn physics_object_3d_resolves_sphere_collision_by_mass_and_restitution() {
    let mut left = PhysicsObject3D::new(
        Hitbox3D::sphere(1.0, point3(0.0, 0.0, -0.5)),
        1.0,
        1.0,
        0.0,
        point3(0.0, 0.0, 1.0),
    );
    let mut right = PhysicsObject3D::new(
        Hitbox3D::sphere(1.0, point3(0.0, 0.0, 0.5)),
        3.0,
        1.0,
        0.0,
        point3(0.0, 0.0, -1.0),
    );

    assert!(left.resolve_collision(&mut right, 0.0));
    assert_eq!(left.velocity, point3(0.0, 0.0, -2.0));
    assert_eq!(right.velocity, point3(0.0, 0.0, 0.0));
    assert_eq!(left.hitbox.separation(&right.hitbox), point3(0.0, 0.0, 0.0));
}

#[test]
fn physics_objects_resolve_dynamic_collisions_against_immovable_bodies() {
    let mut terrain_2d =
        PhysicsObject2D::new(box_at(0.0, 0.0, 1.0, 1.0), 0.0, 1.0, 0.0, point(0.0, 0.0));
    let terrain_2d_before = terrain_2d.clone();
    let mut body_2d =
        PhysicsObject2D::new(box_at(0.75, 0.0, 1.0, 1.0), 1.0, 1.0, 0.0, point(-1.0, 0.0));
    assert!(terrain_2d.resolve_collision(&mut body_2d, 0.0));
    assert_eq!(terrain_2d, terrain_2d_before);
    assert_eq!(body_2d.velocity, point(1.0, 0.0));
    assert_eq!(
        terrain_2d.hitbox.separation(&body_2d.hitbox),
        point(0.0, 0.0)
    );

    let mut terrain_3d = PhysicsObject3D::new(
        cube_at(0.0, 0.0, 0.0, 1.0),
        0.0,
        1.0,
        0.0,
        point3(0.0, 0.0, 0.0),
    );
    let terrain_3d_before = terrain_3d.clone();
    let mut body_3d = PhysicsObject3D::new(
        cube_at(1.5, 0.0, 0.0, 1.0),
        1.0,
        1.0,
        0.0,
        point3(-1.0, 0.0, 0.0),
    );
    assert!(terrain_3d.resolve_collision(&mut body_3d, 0.0));
    assert_eq!(terrain_3d, terrain_3d_before);
    assert_eq!(body_3d.velocity, point3(1.0, 0.0, 0.0));
    assert_eq!(
        terrain_3d.hitbox.separation(&body_3d.hitbox),
        point3(0.0, 0.0, 0.0)
    );
}

#[test]
fn physics_objects_ignore_velocity_on_immovable_bodies() {
    let mut terrain_2d =
        PhysicsObject2D::new(box_at(0.0, 0.0, 1.0, 1.0), 0.0, 1.0, 0.0, point(10.0, 0.0));
    let mut body_2d =
        PhysicsObject2D::new(box_at(0.75, 0.0, 1.0, 1.0), 1.0, 1.0, 0.0, point(0.0, 0.0));
    assert!(terrain_2d.resolve_collision(&mut body_2d, 0.0));
    assert_eq!(terrain_2d.velocity, point(0.0, 0.0));
    assert_eq!(body_2d.velocity, point(0.0, 0.0));

    let mut terrain_3d = PhysicsObject3D::new(
        cube_at(0.0, 0.0, 0.0, 1.0),
        0.0,
        1.0,
        0.0,
        point3(10.0, 0.0, 0.0),
    );
    let mut body_3d = PhysicsObject3D::new(
        cube_at(1.5, 0.0, 0.0, 1.0),
        1.0,
        1.0,
        0.0,
        point3(0.0, 0.0, 0.0),
    );
    assert!(terrain_3d.resolve_collision(&mut body_3d, 0.0));
    assert_eq!(terrain_3d.velocity, point3(0.0, 0.0, 0.0));
    assert_eq!(body_3d.velocity, point3(0.0, 0.0, 0.0));
}

#[test]
fn physics_objects_apply_friction_during_collisions() {
    let mut terrain_2d =
        PhysicsObject2D::new(box_at(0.0, 0.0, 1.0, 1.0), 0.0, 1.0, 0.0, point(0.0, 0.0));
    let mut body_2d =
        PhysicsObject2D::new(box_at(0.75, 0.0, 1.0, 1.0), 1.0, 1.0, 0.0, point(-1.0, 4.0));
    let terrain_before = terrain_2d.clone();
    let body_before = body_2d.clone();
    assert!(!terrain_2d.resolve_collision(&mut body_2d, f32::NAN));
    assert_eq!(terrain_2d, terrain_before);
    assert_eq!(body_2d, body_before);
    assert!(!terrain_2d.resolve_collision(&mut body_2d, -0.1));
    assert_eq!(terrain_2d, terrain_before);
    assert_eq!(body_2d, body_before);
    assert!(terrain_2d.resolve_collision(&mut body_2d, 0.5));
    assert_eq!(body_2d.velocity, point(1.0, 3.0));

    let mut terrain_3d = PhysicsObject3D::new(
        cube_at(0.0, 0.0, 0.0, 1.0),
        0.0,
        1.0,
        0.0,
        point3(0.0, 0.0, 0.0),
    );
    let mut body_3d = PhysicsObject3D::new(
        cube_at(1.5, 0.0, 0.0, 1.0),
        1.0,
        1.0,
        0.0,
        point3(-1.0, 4.0, 0.0),
    );
    assert!(terrain_3d.resolve_collision(&mut body_3d, 0.5));
    assert_eq!(body_3d.velocity, point3(1.0, 3.0, 0.0));
}

#[test]
fn two_immovable_bodies_do_not_resolve_a_collision() {
    let mut first =
        PhysicsObject2D::new(box_at(0.0, 0.0, 1.0, 1.0), 0.0, 1.0, 0.0, point(0.0, 0.0));
    let mut second =
        PhysicsObject2D::new(box_at(0.75, 0.0, 1.0, 1.0), 0.0, 1.0, 0.0, point(0.0, 0.0));
    let first_before = first.clone();
    let second_before = second.clone();

    assert!(!first.resolve_collision(&mut second, 0.0));
    assert_eq!(first, first_before);
    assert_eq!(second, second_before);
}

#[test]
fn physics_objects_resolve_exact_circle_and_sphere_contacts() {
    let mut left_2d = PhysicsObject2D::new(
        Hitbox2D::circle(1.0, point(-1.0, 0.0)),
        1.0,
        1.0,
        0.0,
        point(1.0, 0.0),
    );
    let mut right_2d = PhysicsObject2D::new(
        Hitbox2D::circle(1.0, point(1.0, 0.0)),
        1.0,
        1.0,
        0.0,
        point(-1.0, 0.0),
    );
    assert!(left_2d.resolve_collision(&mut right_2d, 0.0));
    assert_eq!(left_2d.velocity, point(-1.0, 0.0));
    assert_eq!(right_2d.velocity, point(1.0, 0.0));

    let mut left_3d = PhysicsObject3D::new(
        Hitbox3D::sphere(1.0, point3(0.0, 0.0, -1.0)),
        1.0,
        1.0,
        0.0,
        point3(0.0, 0.0, 1.0),
    );
    let mut right_3d = PhysicsObject3D::new(
        Hitbox3D::sphere(1.0, point3(0.0, 0.0, 1.0)),
        1.0,
        1.0,
        0.0,
        point3(0.0, 0.0, -1.0),
    );
    assert!(left_3d.resolve_collision(&mut right_3d, 0.0));
    assert_eq!(left_3d.velocity, point3(0.0, 0.0, -1.0));
    assert_eq!(right_3d.velocity, point3(0.0, 0.0, 1.0));
}

#[test]
fn physics_objects_resolve_exact_circle_and_sphere_polygon_contacts() {
    let mut circle = PhysicsObject2D::new(
        Hitbox2D::circle(0.5, point(1.5, 0.5)),
        1.0,
        1.0,
        0.0,
        point(-1.0, 0.0),
    );
    let mut rectangle =
        PhysicsObject2D::new(box_at(0.0, 0.0, 1.0, 1.0), 0.0, 1.0, 0.0, point(0.0, 0.0));
    assert!(circle.resolve_collision(&mut rectangle, 0.0));
    assert_eq!(circle.velocity, point(1.0, 0.0));
    assert_eq!(rectangle.velocity, point(0.0, 0.0));
    assert_eq!(circle.hitbox.separation(&rectangle.hitbox), point(0.0, 0.0));

    let mut sphere = PhysicsObject3D::new(
        Hitbox3D::sphere(0.5, point3(1.5, 0.0, 0.0)),
        1.0,
        1.0,
        0.0,
        point3(-1.0, 0.0, 0.0),
    );
    let mut cube = PhysicsObject3D::new(
        cube_at(0.0, 0.0, 0.0, 1.0),
        0.0,
        1.0,
        0.0,
        point3(0.0, 0.0, 0.0),
    );
    assert!(sphere.resolve_collision(&mut cube, 0.0));
    assert_eq!(sphere.velocity, point3(1.0, 0.0, 0.0));
    assert_eq!(cube.velocity, point3(0.0, 0.0, 0.0));
    assert_eq!(
        sphere.hitbox.separation(&cube.hitbox),
        point3(0.0, 0.0, 0.0)
    );
}

#[test]
fn physics_objects_resolve_exact_convex_polygon_contacts() {
    let mut left_2d =
        PhysicsObject2D::new(box_at(0.0, 0.0, 1.0, 1.0), 1.0, 1.0, 0.0, point(1.0, 0.0));
    let mut right_2d =
        PhysicsObject2D::new(box_at(1.0, 0.0, 1.0, 1.0), 1.0, 1.0, 0.0, point(-1.0, 0.0));
    assert!(left_2d.resolve_collision(&mut right_2d, 0.0));
    assert_eq!(left_2d.velocity, point(-1.0, 0.0));
    assert_eq!(right_2d.velocity, point(1.0, 0.0));
    assert_eq!(left_2d.hitbox.separation(&right_2d.hitbox), point(0.0, 0.0));

    let mut left_3d = PhysicsObject3D::new(
        cube_at(-1.0, 0.0, 0.0, 1.0),
        1.0,
        1.0,
        0.0,
        point3(1.0, 0.0, 0.0),
    );
    let mut right_3d = PhysicsObject3D::new(
        cube_at(1.0, 0.0, 0.0, 1.0),
        1.0,
        1.0,
        0.0,
        point3(-1.0, 0.0, 0.0),
    );
    assert!(left_3d.resolve_collision(&mut right_3d, 0.0));
    assert_eq!(left_3d.velocity, point3(-1.0, 0.0, 0.0));
    assert_eq!(right_3d.velocity, point3(1.0, 0.0, 0.0));
    assert_eq!(
        left_3d.hitbox.separation(&right_3d.hitbox),
        point3(0.0, 0.0, 0.0)
    );
}

#[test]
fn physics_object_3d_rejects_unrepresentable_collision_atomically() {
    let mut left = PhysicsObject3D::new(
        Hitbox3D::sphere(1.0, point3(0.0, 0.0, -0.5)),
        1.0,
        1.0,
        0.0,
        point3(0.0, 0.0, 2.0e38),
    );
    let mut right = PhysicsObject3D::new(
        Hitbox3D::sphere(1.0, point3(0.0, 0.0, 0.5)),
        1.0e-30,
        1.0,
        0.0,
        point3(0.0, 0.0, 0.0),
    );
    let left_before = left.clone();
    let right_before = right.clone();

    assert!(!left.resolve_collision(&mut right, 0.0));
    assert_eq!(left, left_before);
    assert_eq!(right, right_before);
}

#[test]
fn hitbox_translation_rejects_coordinate_overflow_atomically() {
    let mut hitbox_2d = Hitbox2D::circle(1.0, point(0.0, 0.0));
    hitbox_2d.translate_by(point(f32::MAX, f32::MAX));
    let valid_2d = hitbox_2d.clone();
    hitbox_2d.translate_by(point(f32::MAX, 0.0));
    assert_eq!(hitbox_2d, valid_2d);

    let mut hitbox_3d = Hitbox3D::sphere(1.0, point3(0.0, 0.0, 0.0));
    hitbox_3d.translate_by(point3(f32::MAX, f32::MAX, f32::MAX));
    let valid_3d = hitbox_3d.clone();
    hitbox_3d.translate_by(point3(f32::MAX, 0.0, 0.0));
    assert_eq!(hitbox_3d, valid_3d);
}

#[test]
fn large_circle_math_handles_overflowing_squares_and_translations() {
    let first = Hitbox2D::circle(1.0e20, point(0.0, 0.0));
    let distant = Hitbox2D::circle(1.0e20, point(3.0e20, 0.0));

    assert!(!first.intersects(&distant));
    assert_eq!(first.separation(&distant), point(0.0, 0.0));

    let huge = Hitbox2D::circle(f32::MAX, point(0.0, 0.0));
    assert!(huge.intersects(&huge));
    assert_eq!(huge.separation(&huge), point(0.0, 0.0));
}
