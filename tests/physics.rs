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
use vagus::physics::{PhysicsObject2D, PhysicsObject3D};

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
