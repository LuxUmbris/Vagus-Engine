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

pub mod hitbox {
    use crate::essential::collections::StableVec;
    use crate::math::{MathVec, Vec2, Vec3};

    pub trait Hitbox<V>
    where
        V: MathVec,
    {
        fn intersects(&self, other: &Self) -> bool;
        fn separation(&self, other: &Self) -> V;
    }

    #[derive(Debug, Clone, PartialEq)]
    pub enum Hitbox2D {
        Rectangular(Vec2, Vec2, Vec2, Vec2, /* Origin for rotation */ Vec2),
        Triangular(Vec2, Vec2, Vec2, /* Origin */ Vec2),
        /// radius, origin
        Circle(f32, Vec2),
        /// edges, origin
        Polygon(StableVec<Vec2>, Vec2),
    }

    #[derive(Debug, Clone, PartialEq)]
    pub enum Hitbox3D {
        Quad(
            Vec3,
            Vec3,
            Vec3,
            Vec3,
            Vec3,
            Vec3,
            Vec3,
            Vec3,
            /// Origin
            Vec3,
        ),
        /// radius, origin
        Sphere(f32, Vec3),
        Object(StableVec<Vec3>, /* Origin */ Vec3),
    }

    impl Hitbox3D {
        pub fn new() -> Self {
            Self::quad(
                Vec3 {
                    x: -10.0,
                    y: -10.0,
                    z: -10.0,
                },
                Vec3 {
                    x: 10.0,
                    y: -10.0,
                    z: -10.0,
                },
                Vec3 {
                    x: 10.0,
                    y: 10.0,
                    z: -10.0,
                },
                Vec3 {
                    x: -10.0,
                    y: 10.0,
                    z: -10.0,
                },
                Vec3 {
                    x: -10.0,
                    y: -10.0,
                    z: 10.0,
                },
                Vec3 {
                    x: 10.0,
                    y: -10.0,
                    z: 10.0,
                },
                Vec3 {
                    x: 10.0,
                    y: 10.0,
                    z: 10.0,
                },
                Vec3 {
                    x: -10.0,
                    y: 10.0,
                    z: 10.0,
                },
            )
        }

        pub fn quad(
            a: Vec3,
            b: Vec3,
            c: Vec3,
            d: Vec3,
            e: Vec3,
            f: Vec3,
            g: Vec3,
            h: Vec3,
        ) -> Self {
            Self::quad_at(a, b, c, d, e, f, g, h, zero3())
        }

        pub fn quad_at(
            a: Vec3,
            b: Vec3,
            c: Vec3,
            d: Vec3,
            e: Vec3,
            f: Vec3,
            g: Vec3,
            h: Vec3,
            origin: Vec3,
        ) -> Self {
            Self::Quad(a, b, c, d, e, f, g, h, origin)
        }

        pub fn sphere(radius: f32, center: Vec3) -> Self {
            Self::Sphere(radius, center)
        }

        pub fn object(points: StableVec<Vec3>) -> Self {
            Self::object_at(points, zero3())
        }

        pub fn object_at(points: StableVec<Vec3>, origin: Vec3) -> Self {
            Self::Object(points, origin)
        }

        pub fn translate_by(&mut self, offset: Vec3) {
            if !offset.x.is_finite() || !offset.y.is_finite() || !offset.z.is_finite() {
                return;
            }
            match self {
                Self::Quad(_, _, _, _, _, _, _, _, origin) | Self::Object(_, origin) => {
                    *origin = add3(*origin, offset)
                }
                Self::Sphere(_, center) => *center = add3(*center, offset),
            }
        }
    }

    impl Default for Hitbox3D {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Hitbox<Vec3> for Hitbox3D {
        /// Returns whether these convex hitboxes overlap or touch.
        fn intersects(&self, rhs: &Hitbox3D) -> bool {
            minimum_translation_3d(self, rhs).is_some()
        }

        /// Returns the minimum translation vector to move `self` out of `rhs`.
        /// Returns zero when the shapes do not overlap or either shape is invalid.
        fn separation(&self, rhs: &Hitbox3D) -> Vec3 {
            minimum_translation_3d(self, rhs).unwrap_or(zero3())
        }
    }

    impl Hitbox2D {
        pub fn new() -> Self {
            Self::Rectangular(
                Vec2 { x: 0.0, y: 0.0 },
                Vec2 { x: 0.0, y: 0.0 },
                Vec2 { x: 0.0, y: 0.0 },
                Vec2 { x: 0.0, y: 0.0 },
                Vec2 { x: 0.0, y: 0.0 },
            )
        }

        pub fn rectangular(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> Self {
            Self::rectangular_at(a, b, c, d, Vec2 { x: 0.0, y: 0.0 })
        }

        pub fn rectangular_at(a: Vec2, b: Vec2, c: Vec2, d: Vec2, origin: Vec2) -> Self {
            Self::Rectangular(a, b, c, d, origin)
        }

        pub fn triangular(a: Vec2, b: Vec2, c: Vec2) -> Self {
            Self::triangular_at(a, b, c, Vec2 { x: 0.0, y: 0.0 })
        }

        pub fn triangular_at(a: Vec2, b: Vec2, c: Vec2, origin: Vec2) -> Self {
            Self::Triangular(a, b, c, origin)
        }

        pub fn circle(radius: f32, center: Vec2) -> Self {
            Self::Circle(radius, center)
        }

        pub fn polygon(edges: StableVec<Vec2>) -> Self {
            Self::polygon_at(edges, Vec2 { x: 0.0, y: 0.0 })
        }

        pub fn polygon_at(edges: StableVec<Vec2>, origin: Vec2) -> Self {
            Self::Polygon(edges, origin)
        }

        pub fn translate_by(&mut self, offset: Vec2) {
            if !offset.x.is_finite() || !offset.y.is_finite() {
                return;
            }
            match self {
                Self::Rectangular(_, _, _, _, origin)
                | Self::Triangular(_, _, _, origin)
                | Self::Polygon(_, origin) => {
                    origin.x += offset.x;
                    origin.y += offset.y;
                }
                Self::Circle(_, center) => {
                    center.x += offset.x;
                    center.y += offset.y;
                }
            }
        }
    }

    impl Hitbox<Vec2> for Hitbox2D {
        /// Returns whether these convex hitboxes overlap or touch.
        fn intersects(&self, rhs: &Hitbox2D) -> bool {
            minimum_translation(self, rhs).is_some()
        }

        /// Returns the minimum translation vector to move `self` out of `rhs`.
        /// Returns zero when they do not overlap; touching shapes also need no motion.
        fn separation(&self, rhs: &Hitbox2D) -> Vec2 {
            minimum_translation(self, rhs).unwrap_or(Vec2 { x: 0.0, y: 0.0 })
        }
    }

    impl Default for Hitbox2D {
        fn default() -> Self {
            Self::new()
        }
    }

    fn vertices(shape: &Hitbox2D) -> Option<Vec<Vec2>> {
        let points: Vec<Vec2> = match shape {
            Hitbox2D::Rectangular(a, b, c, d, origin) => [*a, *b, *c, *d]
                .into_iter()
                .map(|point| Vec2 {
                    x: point.x + origin.x,
                    y: point.y + origin.y,
                })
                .collect(),
            Hitbox2D::Triangular(a, b, c, origin) => [*a, *b, *c]
                .into_iter()
                .map(|point| Vec2 {
                    x: point.x + origin.x,
                    y: point.y + origin.y,
                })
                .collect(),
            Hitbox2D::Polygon(points, origin) => points
                .iter()
                .map(|point| Vec2 {
                    x: point.x + origin.x,
                    y: point.y + origin.y,
                })
                .collect(),
            Hitbox2D::Circle(..) => return None,
        };
        if points.len() < 3 || points.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
            return None;
        }
        Some(points)
    }

    fn circle_radius(shape: &Hitbox2D) -> Option<f32> {
        match shape {
            Hitbox2D::Circle(radius, center)
                if radius.is_finite()
                    && *radius >= 0.0
                    && center.x.is_finite()
                    && center.y.is_finite() =>
            {
                Some(*radius)
            }
            _ => None,
        }
    }

    fn circle_center(shape: &Hitbox2D) -> Option<Vec2> {
        match shape {
            Hitbox2D::Circle(_, center) if circle_radius(shape).is_some() => Some(*center),
            _ => None,
        }
    }

    fn dot(a: Vec2, b: Vec2) -> f32 {
        a.x * b.x + a.y * b.y
    }

    fn sub(a: Vec2, b: Vec2) -> Vec2 {
        Vec2 {
            x: a.x - b.x,
            y: a.y - b.y,
        }
    }

    fn normalized(v: Vec2) -> Option<Vec2> {
        let length = dot(v, v).sqrt();
        (length > 0.0 && length.is_finite()).then(|| Vec2 {
            x: v.x / length,
            y: v.y / length,
        })
    }

    fn project(points: &[Vec2], axis: Vec2) -> (f32, f32) {
        let mut min = dot(points[0], axis);
        let mut max = min;
        for point in &points[1..] {
            let value = dot(*point, axis);
            min = min.min(value);
            max = max.max(value);
        }
        (min, max)
    }

    fn polygon_axes(points: &[Vec2], axes: &mut Vec<Vec2>) {
        for index in 0..points.len() {
            let edge = sub(points[(index + 1) % points.len()], points[index]);
            if let Some(axis) = normalized(Vec2 {
                x: -edge.y,
                y: edge.x,
            }) {
                axes.push(axis);
            }
        }
    }

    fn minimum_translation(a: &Hitbox2D, b: &Hitbox2D) -> Option<Vec2> {
        if let (Some(ra), Some(rb)) = (circle_radius(a), circle_radius(b)) {
            let delta = sub(circle_center(a)?, circle_center(b)?);
            let distance_squared = dot(delta, delta);
            let radius_sum = ra + rb;
            if distance_squared > radius_sum * radius_sum {
                return None;
            }
            let direction = normalized(delta).unwrap_or(Vec2 { x: 1.0, y: 0.0 });
            return Some(Vec2 {
                x: direction.x * (radius_sum - distance_squared.sqrt()),
                y: direction.y * (radius_sum - distance_squared.sqrt()),
            });
        }

        let a_points = vertices(a);
        let b_points = vertices(b);
        if a_points.is_none() && circle_radius(a).is_none()
            || b_points.is_none() && circle_radius(b).is_none()
        {
            return None;
        }

        let mut axes = Vec::new();
        if let Some(points) = &a_points {
            polygon_axes(points, &mut axes);
        }
        if let Some(points) = &b_points {
            polygon_axes(points, &mut axes);
        }

        for (circle, points) in [(a, &b_points), (b, &a_points)] {
            if let (Some(radius), Some(points)) = (circle_radius(circle), points) {
                let center = circle_center(circle)?;
                let closest = points.iter().copied().min_by(|left, right| {
                    dot(sub(*left, center), sub(*left, center))
                        .total_cmp(&dot(sub(*right, center), sub(*right, center)))
                })?;
                if radius.is_finite() {
                    if let Some(axis) = normalized(sub(closest, center)) {
                        axes.push(axis);
                    }
                }
            }
        }

        if axes.is_empty() {
            return None;
        }
        let mut best: Option<Vec2> = None;

        for axis in axes {
            let (a_min, a_max) = if let Some(points) = &a_points {
                project(points, axis)
            } else {
                let radius = circle_radius(a)?;
                let center = circle_center(a)?;
                let projected = dot(center, axis);
                (projected - radius, projected + radius)
            };
            let (b_min, b_max) = if let Some(points) = &b_points {
                project(points, axis)
            } else {
                let radius = circle_radius(b)?;
                let center = circle_center(b)?;
                let projected = dot(center, axis);
                (projected - radius, projected + radius)
            };
            if a_max < b_min || b_max < a_min {
                return None;
            }
            let move_positive = b_max - a_min;
            let move_negative = a_max - b_min;
            let translation = if move_positive < move_negative {
                move_positive
            } else {
                -move_negative
            };
            let candidate = Vec2 {
                x: axis.x * translation,
                y: axis.y * translation,
            };
            if best.map_or(true, |current| {
                dot(candidate, candidate) < dot(current, current)
            }) {
                best = Some(candidate);
            }
        }
        best
    }

    fn zero3() -> Vec3 {
        Vec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }

    fn add3(a: Vec3, b: Vec3) -> Vec3 {
        Vec3 {
            x: a.x + b.x,
            y: a.y + b.y,
            z: a.z + b.z,
        }
    }

    fn sub3(a: Vec3, b: Vec3) -> Vec3 {
        Vec3 {
            x: a.x - b.x,
            y: a.y - b.y,
            z: a.z - b.z,
        }
    }

    fn scale3(v: Vec3, scale: f32) -> Vec3 {
        Vec3 {
            x: v.x * scale,
            y: v.y * scale,
            z: v.z * scale,
        }
    }

    fn dot3(a: Vec3, b: Vec3) -> f32 {
        a.x * b.x + a.y * b.y + a.z * b.z
    }

    fn cross3(a: Vec3, b: Vec3) -> Vec3 {
        Vec3 {
            x: a.y * b.z - a.z * b.y,
            y: a.z * b.x - a.x * b.z,
            z: a.x * b.y - a.y * b.x,
        }
    }

    fn length3(v: Vec3) -> f32 {
        dot3(v, v).sqrt()
    }

    fn normalized3(v: Vec3) -> Option<Vec3> {
        let length = length3(v);
        (length > 0.0 && length.is_finite()).then(|| scale3(v, 1.0 / length))
    }

    fn sphere3(shape: &Hitbox3D) -> Option<(f32, Vec3)> {
        match shape {
            Hitbox3D::Sphere(radius, center)
                if radius.is_finite()
                    && *radius >= 0.0
                    && center.x.is_finite()
                    && center.y.is_finite()
                    && center.z.is_finite() =>
            {
                Some((*radius, *center))
            }
            _ => None,
        }
    }

    fn vertices3(shape: &Hitbox3D) -> Option<Vec<Vec3>> {
        let (mut points, origin): (Vec<Vec3>, Vec3) = match shape {
            Hitbox3D::Quad(a, b, c, d, e, f, g, h, origin) => {
                (vec![*a, *b, *c, *d, *e, *f, *g, *h], *origin)
            }
            Hitbox3D::Object(points, origin) => (points.iter().copied().collect(), *origin),
            Hitbox3D::Sphere(..) => return None,
        };
        if points.len() < 4
            || !origin.x.is_finite()
            || !origin.y.is_finite()
            || !origin.z.is_finite()
            || points
                .iter()
                .any(|p| !p.x.is_finite() || !p.y.is_finite() || !p.z.is_finite())
        {
            return None;
        }
        for point in &mut points {
            *point = add3(*point, origin);
        }
        if points
            .iter()
            .any(|p| !p.x.is_finite() || !p.y.is_finite() || !p.z.is_finite())
        {
            return None;
        }
        Some(points)
    }

    fn point_tolerance(points: &[Vec3]) -> f32 {
        let mut minimum = [f32::INFINITY; 3];
        let mut maximum = [f32::NEG_INFINITY; 3];
        let mut coordinate_scale = 0.0_f32;
        for point in points {
            for (index, value) in [point.x, point.y, point.z].into_iter().enumerate() {
                minimum[index] = minimum[index].min(value);
                maximum[index] = maximum[index].max(value);
                coordinate_scale = coordinate_scale.max(value.abs());
            }
        }
        let extent = (maximum[0] - minimum[0])
            .max(maximum[1] - minimum[1])
            .max(maximum[2] - minimum[2]);
        extent * 1.0e-5 + coordinate_scale * f32::EPSILON * 8.0
    }

    // Enumerating supporting triples gives a safe triangulation of each convex hull face.
    fn hull_faces(points: &[Vec3]) -> Option<Vec<[Vec3; 3]>> {
        let (minimum, maximum) = points.iter().fold(
            ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]),
            |(mut min, mut max), point| {
                for (index, value) in [point.x, point.y, point.z].into_iter().enumerate() {
                    min[index] = min[index].min(value);
                    max[index] = max[index].max(value);
                }
                (min, max)
            },
        );
        let extent = (maximum[0] - minimum[0])
            .max(maximum[1] - minimum[1])
            .max(maximum[2] - minimum[2]);
        if !extent.is_finite() || extent <= 0.0 {
            return None;
        }
        let tolerance = point_tolerance(points);
        let minimum_normal = extent * extent * 1.0e-7;
        let mut faces = Vec::new();
        for i in 0..points.len() - 2 {
            for j in i + 1..points.len() - 1 {
                for k in j + 1..points.len() {
                    let raw_normal = cross3(sub3(points[j], points[i]), sub3(points[k], points[i]));
                    if length3(raw_normal) <= minimum_normal {
                        continue;
                    }
                    let normal = normalized3(raw_normal)?;
                    let mut positive = false;
                    let mut negative = false;
                    for point in points {
                        let distance = dot3(normal, sub3(*point, points[i]));
                        positive |= distance > tolerance;
                        negative |= distance < -tolerance;
                    }
                    if !(positive && negative) && (positive || negative) {
                        faces.push([points[i], points[j], points[k]]);
                    }
                }
            }
        }
        (!faces.is_empty()).then_some(faces)
    }

    fn push_axis(axes: &mut Vec<Vec3>, candidate: Vec3) {
        let Some(axis) = normalized3(candidate) else {
            return;
        };
        if axes
            .iter()
            .any(|existing| dot3(*existing, axis).abs() > 1.0 - 1.0e-5)
        {
            return;
        }
        axes.push(axis);
    }

    fn poly_axes(a_faces: &[[Vec3; 3]], b_faces: &[[Vec3; 3]]) -> Vec<Vec3> {
        let mut axes = Vec::new();
        let face_normal = |face: &[Vec3; 3]| cross3(sub3(face[1], face[0]), sub3(face[2], face[0]));
        for face in a_faces.iter().chain(b_faces) {
            push_axis(&mut axes, face_normal(face));
        }
        let edges = |faces: &[[Vec3; 3]]| {
            faces
                .iter()
                .flat_map(|face| {
                    [
                        sub3(face[1], face[0]),
                        sub3(face[2], face[1]),
                        sub3(face[0], face[2]),
                    ]
                })
                .collect::<Vec<_>>()
        };
        let a_edges = edges(a_faces);
        let b_edges = edges(b_faces);
        for edge_a in &a_edges {
            for edge_b in &b_edges {
                push_axis(&mut axes, cross3(*edge_a, *edge_b));
            }
        }
        axes
    }

    fn project3(points: &[Vec3], axis: Vec3) -> (f32, f32) {
        let first = dot3(points[0], axis);
        points
            .iter()
            .skip(1)
            .fold((first, first), |(min, max), point| {
                let value = dot3(*point, axis);
                (min.min(value), max.max(value))
            })
    }

    fn closest_on_triangle(point: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
        let ab = sub3(b, a);
        let ac = sub3(c, a);
        let ap = sub3(point, a);
        let d1 = dot3(ab, ap);
        let d2 = dot3(ac, ap);
        if d1 <= 0.0 && d2 <= 0.0 {
            return a;
        }
        let bp = sub3(point, b);
        let d3 = dot3(ab, bp);
        let d4 = dot3(ac, bp);
        if d3 >= 0.0 && d4 <= d3 {
            return b;
        }
        let vc = d1 * d4 - d3 * d2;
        if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
            return add3(a, scale3(ab, d1 / (d1 - d3)));
        }
        let cp = sub3(point, c);
        let d5 = dot3(ab, cp);
        let d6 = dot3(ac, cp);
        if d6 >= 0.0 && d5 <= d6 {
            return c;
        }
        let vb = d5 * d2 - d1 * d6;
        if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
            return add3(a, scale3(ac, d2 / (d2 - d6)));
        }
        let va = d3 * d6 - d5 * d4;
        if va <= 0.0 && d4 - d3 >= 0.0 && d5 - d6 >= 0.0 {
            return add3(b, scale3(sub3(c, b), (d4 - d3) / ((d4 - d3) + (d5 - d6))));
        }
        let denominator = 1.0 / (va + vb + vc);
        add3(
            a,
            add3(scale3(ab, vb * denominator), scale3(ac, vc * denominator)),
        )
    }

    fn sphere_poly_translation(sphere: &Hitbox3D, poly: &Hitbox3D) -> Option<Vec3> {
        let (radius, center) = sphere3(sphere)?;
        let points = vertices3(poly)?;
        let faces = hull_faces(&points)?;
        let mut inside = true;
        let mut nearest = center;
        let mut nearest_distance = f32::INFINITY;
        let mut nearest_outward = zero3();
        let tolerance = point_tolerance(&points);
        let centroid = scale3(
            points.iter().copied().fold(zero3(), add3),
            1.0 / points.len() as f32,
        );
        for face in &faces {
            let raw_normal = cross3(sub3(face[1], face[0]), sub3(face[2], face[0]));
            let normal = normalized3(raw_normal)?;
            let outward = if dot3(normal, sub3(face[0], centroid)) < 0.0 {
                scale3(normal, -1.0)
            } else {
                normal
            };
            if dot3(sub3(center, face[0]), outward) > tolerance {
                inside = false;
            }
            let closest = closest_on_triangle(center, face[0], face[1], face[2]);
            let distance = length3(sub3(closest, center));
            if distance < nearest_distance {
                nearest_distance = distance;
                nearest = closest;
                nearest_outward = outward;
            }
        }
        if !nearest_distance.is_finite() || (!inside && nearest_distance > radius) {
            return None;
        }
        if inside {
            let direction = normalized3(sub3(nearest, center)).unwrap_or(nearest_outward);
            Some(scale3(direction, nearest_distance + radius))
        } else {
            let direction = normalized3(sub3(center, nearest)).unwrap_or(nearest_outward);
            Some(scale3(direction, radius - nearest_distance))
        }
    }

    fn minimum_translation_3d(a: &Hitbox3D, b: &Hitbox3D) -> Option<Vec3> {
        if let (Some((ra, ca)), Some((rb, cb))) = (sphere3(a), sphere3(b)) {
            let delta = sub3(ca, cb);
            let distance = length3(delta);
            let overlap = ra + rb - distance;
            if overlap < 0.0 {
                return None;
            }
            return Some(scale3(
                normalized3(delta).unwrap_or(Vec3 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                }),
                overlap,
            ));
        }
        if sphere3(a).is_some() {
            return sphere_poly_translation(a, b);
        }
        if sphere3(b).is_some() {
            return sphere_poly_translation(b, a).map(|translation| scale3(translation, -1.0));
        }

        let a_points = vertices3(a)?;
        let b_points = vertices3(b)?;
        let a_faces = hull_faces(&a_points)?;
        let b_faces = hull_faces(&b_points)?;
        let axes = poly_axes(&a_faces, &b_faces);
        let mut best = None;
        for axis in axes {
            let (a_min, a_max) = project3(&a_points, axis);
            let (b_min, b_max) = project3(&b_points, axis);
            if a_max < b_min || b_max < a_min {
                return None;
            }
            let positive = b_max - a_min;
            let negative = a_max - b_min;
            let translation = if positive < negative {
                positive
            } else {
                -negative
            };
            let candidate = scale3(axis, translation);
            if best.map_or(true, |current: Vec3| {
                dot3(candidate, candidate) < dot3(current, current)
            }) {
                best = Some(candidate);
            }
        }
        best
    }
}

use super::math::{Vec2, Vec3};
use crate::physics::hitbox::{Hitbox2D, Hitbox3D};

#[derive(Debug, Clone, PartialEq)]
pub struct PhysicsObject2D {
    pub hitbox: hitbox::Hitbox2D,
    pub mass: f64,
    pub elasticity: f64,
    pub plasticity: f64,
    pub velocity: Vec2,
}

impl PhysicsObject2D {
    pub fn new(
        hitbox: Hitbox2D,
        mass: f64,
        elasticity: f64,
        plasticity: f64,
        velocity: Vec2,
    ) -> Self {
        Self {
            hitbox,
            mass,
            elasticity,
            plasticity,
            velocity,
        }
    }

    /// Advances the hitbox by its constant velocity for finite `delta_time` seconds.
    pub fn integrate(&mut self, delta_time: f32) {
        if !delta_time.is_finite() {
            return;
        }
        self.hitbox.translate_by(Vec2 {
            x: self.velocity.x * delta_time,
            y: self.velocity.y * delta_time,
        });
    }
}

impl Default for PhysicsObject2D {
    fn default() -> Self {
        Self {
            hitbox: hitbox::Hitbox2D::rectangular(
                Vec2 { x: 10.0, y: 10.0 },
                Vec2 { x: 10.0, y: -10.0 },
                Vec2 { x: -10.0, y: -10.0 },
                Vec2 { x: -10.0, y: 10.0 },
            ),
            mass: 10.0,
            elasticity: 1.5,
            plasticity: 0.0,
            velocity: Vec2 { x: 0.0, y: 0.0 },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PhysicsObject3D {
    pub hitbox: hitbox::Hitbox3D,
    pub mass: f64,
    pub elasticity: f64,
    pub plasticity: f64,
    pub velocity: Vec3,
}

impl PhysicsObject3D {
    pub fn new(
        hitbox: Hitbox3D,
        mass: f64,
        elasticity: f64,
        plasticity: f64,
        velocity: Vec3,
    ) -> Self {
        Self {
            hitbox,
            mass,
            elasticity,
            plasticity,
            velocity,
        }
    }

    /// Advances the hitbox by its constant velocity for finite `delta_time` seconds.
    pub fn integrate(&mut self, delta_time: f32) {
        if !delta_time.is_finite() {
            return;
        }
        self.hitbox.translate_by(Vec3 {
            x: self.velocity.x * delta_time,
            y: self.velocity.y * delta_time,
            z: self.velocity.z * delta_time,
        });
    }
}

impl Default for PhysicsObject3D {
    fn default() -> Self {
        Self {
            hitbox: hitbox::Hitbox3D::new(),
            mass: 10.0,
            elasticity: 1.5,
            plasticity: 0.0,
            velocity: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
        }
    }
}
