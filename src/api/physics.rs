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

use crate::{
    api::ecs::{Soa, SoaClone},
    ecs::Ecs,
};
use hitbox::Hitbox;
use std::io;

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
                    let translated = add3(*origin, offset);
                    if translated.x.is_finite()
                        && translated.y.is_finite()
                        && translated.z.is_finite()
                    {
                        *origin = translated;
                    }
                }
                Self::Sphere(_, center) => {
                    let translated = add3(*center, offset);
                    if translated.x.is_finite()
                        && translated.y.is_finite()
                        && translated.z.is_finite()
                    {
                        *center = translated;
                    }
                }
            }
        }

        /// Returns the world-space origin used to translate this hitbox.
        pub fn origin(&self) -> Vec3 {
            match self {
                Self::Quad(_, _, _, _, _, _, _, _, origin) | Self::Object(_, origin) => *origin,
                Self::Sphere(_, center) => *center,
            }
        }

        pub(super) fn broadphase_bounds(&self) -> Option<[(f64, f64); 3]> {
            match self {
                Self::Quad(a, b, c, d, e, f, g, h, origin) => {
                    bounds3([*a, *b, *c, *d, *e, *f, *g, *h], *origin)
                }
                Self::Object(points, origin) => bounds3(points.iter().copied(), *origin),
                Self::Sphere(radius, center)
                    if radius.is_finite()
                        && *radius >= 0.0
                        && center.x.is_finite()
                        && center.y.is_finite()
                        && center.z.is_finite() =>
                {
                    let radius = *radius as f64;
                    let center = [center.x as f64, center.y as f64, center.z as f64];
                    Some([
                        (center[0] - radius, center[0] + radius),
                        (center[1] - radius, center[1] + radius),
                        (center[2] - radius, center[2] + radius),
                    ])
                }
                Self::Sphere(..) => None,
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
                    let translated = Vec2 {
                        x: origin.x + offset.x,
                        y: origin.y + offset.y,
                    };
                    if translated.x.is_finite() && translated.y.is_finite() {
                        *origin = translated;
                    }
                }
                Self::Circle(_, center) => {
                    let translated = Vec2 {
                        x: center.x + offset.x,
                        y: center.y + offset.y,
                    };
                    if translated.x.is_finite() && translated.y.is_finite() {
                        *center = translated;
                    }
                }
            }
        }

        /// Returns the world-space origin used to translate this hitbox.
        pub fn origin(&self) -> Vec2 {
            match self {
                Self::Rectangular(_, _, _, _, origin)
                | Self::Triangular(_, _, _, origin)
                | Self::Polygon(_, origin) => *origin,
                Self::Circle(_, center) => *center,
            }
        }

        pub(super) fn broadphase_bounds(&self) -> Option<[(f64, f64); 3]> {
            match self {
                Self::Rectangular(a, b, c, d, origin) => bounds2([*a, *b, *c, *d], *origin),
                Self::Triangular(a, b, c, origin) => bounds2([*a, *b, *c], *origin),
                Self::Polygon(points, origin) => bounds2(points.iter().copied(), *origin),
                Self::Circle(radius, center)
                    if radius.is_finite()
                        && *radius >= 0.0
                        && center.x.is_finite()
                        && center.y.is_finite() =>
                {
                    let radius = *radius as f64;
                    let center = [center.x as f64, center.y as f64];
                    Some([
                        (center[0] - radius, center[0] + radius),
                        (center[1] - radius, center[1] + radius),
                        (0.0, 0.0),
                    ])
                }
                Self::Circle(..) => None,
            }
        }
    }

    impl Hitbox<Vec2> for Hitbox2D {
        /// Returns whether these convex hitboxes overlap or touch.
        fn intersects(&self, rhs: &Hitbox2D) -> bool {
            if let Some((intersects, _)) = circle_circle_result(self, rhs) {
                return intersects;
            }
            minimum_translation(self, rhs).is_some()
        }

        /// Returns the minimum translation vector to move `self` out of `rhs`.
        /// Returns zero for separated, touching, or invalid shapes and unrepresentable translations.
        fn separation(&self, rhs: &Hitbox2D) -> Vec2 {
            minimum_translation(self, rhs).unwrap_or(Vec2 { x: 0.0, y: 0.0 })
        }
    }

    impl Default for Hitbox2D {
        fn default() -> Self {
            Self::new()
        }
    }

    fn bounds2(points: impl IntoIterator<Item = Vec2>, origin: Vec2) -> Option<[(f64, f64); 3]> {
        if !origin.x.is_finite() || !origin.y.is_finite() {
            return None;
        }
        let mut min = [f64::INFINITY; 2];
        let mut max = [f64::NEG_INFINITY; 2];
        let mut any = false;
        for point in points {
            if !point.x.is_finite() || !point.y.is_finite() {
                return None;
            }
            for (axis, coordinate) in [(point.x + origin.x) as f64, (point.y + origin.y) as f64]
                .into_iter()
                .enumerate()
            {
                min[axis] = min[axis].min(coordinate);
                max[axis] = max[axis].max(coordinate);
            }
            any = true;
        }
        any.then_some([(min[0], max[0]), (min[1], max[1]), (0.0, 0.0)])
    }

    fn bounds3(points: impl IntoIterator<Item = Vec3>, origin: Vec3) -> Option<[(f64, f64); 3]> {
        if !origin.x.is_finite() || !origin.y.is_finite() || !origin.z.is_finite() {
            return None;
        }
        let mut min = [f64::INFINITY; 3];
        let mut max = [f64::NEG_INFINITY; 3];
        let mut any = false;
        for point in points {
            if !point.x.is_finite() || !point.y.is_finite() || !point.z.is_finite() {
                return None;
            }
            for (axis, coordinate) in [
                (point.x + origin.x) as f64,
                (point.y + origin.y) as f64,
                (point.z + origin.z) as f64,
            ]
            .into_iter()
            .enumerate()
            {
                min[axis] = min[axis].min(coordinate);
                max[axis] = max[axis].max(coordinate);
            }
            any = true;
        }
        any.then_some([(min[0], max[0]), (min[1], max[1]), (min[2], max[2])])
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

    fn circle_circle_result(a: &Hitbox2D, b: &Hitbox2D) -> Option<(bool, Vec2)> {
        let radius_a = f64::from(circle_radius(a)?);
        let radius_b = f64::from(circle_radius(b)?);
        let center_a = circle_center(a)?;
        let center_b = circle_center(b)?;
        let delta_x = f64::from(center_a.x) - f64::from(center_b.x);
        let delta_y = f64::from(center_a.y) - f64::from(center_b.y);
        let distance = delta_x.hypot(delta_y);
        let overlap = radius_a + radius_b - distance;
        if overlap < 0.0 {
            return Some((false, Vec2 { x: 0.0, y: 0.0 }));
        }

        let (direction_x, direction_y) = if distance == 0.0 {
            (1.0, 0.0)
        } else {
            (delta_x / distance, delta_y / distance)
        };
        let translation_x = direction_x * overlap;
        let translation_y = direction_y * overlap;
        let max = f64::from(f32::MAX);
        let translation = if translation_x.abs() <= max && translation_y.abs() <= max {
            Vec2 {
                x: translation_x as f32,
                y: translation_y as f32,
            }
        } else {
            Vec2 { x: 0.0, y: 0.0 }
        };
        Some((true, translation))
    }

    fn dot2_wide(a: Vec2, b: Vec2) -> f64 {
        f64::from(a.x) * f64::from(b.x) + f64::from(a.y) * f64::from(b.y)
    }

    fn distance_squared_wide(a: Vec2, b: Vec2) -> f64 {
        let x = f64::from(a.x) - f64::from(b.x);
        let y = f64::from(a.y) - f64::from(b.y);
        x * x + y * y
    }

    fn normalized_axis(x: f64, y: f64) -> Option<Vec2> {
        let length = x.hypot(y);
        (length > 0.0 && length.is_finite()).then(|| Vec2 {
            x: (x / length) as f32,
            y: (y / length) as f32,
        })
    }

    fn project(points: &[Vec2], axis: Vec2) -> (f64, f64) {
        let mut min = dot2_wide(points[0], axis);
        let mut max = min;
        for point in &points[1..] {
            let value = dot2_wide(*point, axis);
            min = min.min(value);
            max = max.max(value);
        }
        (min, max)
    }

    fn polygon_axes(points: &[Vec2], axes: &mut Vec<Vec2>) {
        for index in 0..points.len() {
            let next = points[(index + 1) % points.len()];
            let edge_x = f64::from(next.x) - f64::from(points[index].x);
            let edge_y = f64::from(next.y) - f64::from(points[index].y);
            if let Some(axis) = normalized_axis(-edge_y, edge_x) {
                axes.push(axis);
            }
        }
    }

    fn minimum_translation(a: &Hitbox2D, b: &Hitbox2D) -> Option<Vec2> {
        if let Some((intersects, translation)) = circle_circle_result(a, b) {
            return intersects.then_some(translation);
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
                    distance_squared_wide(*left, center)
                        .total_cmp(&distance_squared_wide(*right, center))
                })?;
                if radius.is_finite() {
                    let x = f64::from(closest.x) - f64::from(center.x);
                    let y = f64::from(closest.y) - f64::from(center.y);
                    if let Some(axis) = normalized_axis(x, y) {
                        axes.push(axis);
                    }
                }
            }
        }

        if axes.is_empty() {
            return None;
        }
        let mut best: Option<[f64; 2]> = None;

        for axis in axes {
            let (a_min, a_max) = if let Some(points) = &a_points {
                project(points, axis)
            } else {
                let radius = circle_radius(a)?;
                let center = circle_center(a)?;
                let projected = dot2_wide(center, axis);
                (projected - f64::from(radius), projected + f64::from(radius))
            };
            let (b_min, b_max) = if let Some(points) = &b_points {
                project(points, axis)
            } else {
                let radius = circle_radius(b)?;
                let center = circle_center(b)?;
                let projected = dot2_wide(center, axis);
                (projected - f64::from(radius), projected + f64::from(radius))
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
            let candidate = [
                f64::from(axis.x) * translation,
                f64::from(axis.y) * translation,
            ];
            if best.map_or(true, |current| {
                candidate[0].hypot(candidate[1]) < current[0].hypot(current[1])
            }) {
                best = Some(candidate);
            }
        }
        let translation = best?;
        let max = f64::from(f32::MAX);
        Some(
            if translation
                .into_iter()
                .all(|component| component.is_finite() && component.abs() <= max)
            {
                Vec2 {
                    x: translation[0] as f32,
                    y: translation[1] as f32,
                }
            } else {
                Vec2 { x: 0.0, y: 0.0 }
            },
        )
    }

    pub(super) fn touching_polygon_normal(a: &Hitbox2D, b: &Hitbox2D) -> Option<Vec2> {
        let a_points = vertices(a)?;
        let b_points = vertices(b)?;
        let mut axes = Vec::new();
        polygon_axes(&a_points, &mut axes);
        polygon_axes(&b_points, &mut axes);
        for axis in axes {
            let (a_min, a_max) = project(&a_points, axis);
            let (b_min, b_max) = project(&b_points, axis);
            if a_max == b_min {
                return Some(Vec2 {
                    x: -axis.x,
                    y: -axis.y,
                });
            }
            if b_max == a_min {
                return Some(axis);
            }
        }
        None
    }

    pub(super) fn touching_circle_poly_normal(a: &Hitbox2D, b: &Hitbox2D) -> Option<Vec2> {
        let (circle, polygon, circle_is_a) = match (a, b) {
            (Hitbox2D::Circle(..), _) => (a, b, true),
            (_, Hitbox2D::Circle(..)) => (b, a, false),
            _ => return None,
        };
        let center = circle_center(circle)?;
        let radius = f64::from(circle_radius(circle)?);
        let points = vertices(polygon)?;
        let mut closest = None;
        let mut nearest_distance = f64::INFINITY;
        for index in 0..points.len() {
            let start = points[index];
            let end = points[(index + 1) % points.len()];
            let dx = f64::from(end.x) - f64::from(start.x);
            let dy = f64::from(end.y) - f64::from(start.y);
            let length_squared = dx * dx + dy * dy;
            let offset_x = f64::from(center.x) - f64::from(start.x);
            let offset_y = f64::from(center.y) - f64::from(start.y);
            let amount = if length_squared == 0.0 {
                0.0
            } else {
                ((offset_x * dx + offset_y * dy) / length_squared).clamp(0.0, 1.0)
            };
            let point = Vec2 {
                x: (f64::from(start.x) + amount * dx) as f32,
                y: (f64::from(start.y) + amount * dy) as f32,
            };
            let distance = (f64::from(center.x) - f64::from(point.x))
                .hypot(f64::from(center.y) - f64::from(point.y));
            if distance < nearest_distance {
                nearest_distance = distance;
                closest = Some(point);
            }
        }
        let closest = closest?;
        if (nearest_distance - radius).abs() > radius.max(nearest_distance).max(1.0) * 1.0e-6 {
            return None;
        }
        let mut has_positive = false;
        let mut has_negative = false;
        for index in 0..points.len() {
            let start = points[index];
            let end = points[(index + 1) % points.len()];
            let cross = (f64::from(end.x) - f64::from(start.x))
                * (f64::from(center.y) - f64::from(start.y))
                - (f64::from(end.y) - f64::from(start.y))
                    * (f64::from(center.x) - f64::from(start.x));
            has_positive |= cross > 0.0;
            has_negative |= cross < 0.0;
        }
        let inside = !(has_positive && has_negative);
        let sign = if inside == circle_is_a { -1.0 } else { 1.0 };
        let dx = (f64::from(center.x) - f64::from(closest.x)) * sign;
        let dy = (f64::from(center.y) - f64::from(closest.y)) * sign;
        let length = dx.hypot(dy);
        (length > 0.0 && length.is_finite()).then(|| Vec2 {
            x: (dx / length) as f32,
            y: (dy / length) as f32,
        })
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

    fn delta3_wide(a: Vec3, b: Vec3) -> [f64; 3] {
        [
            f64::from(a.x) - f64::from(b.x),
            f64::from(a.y) - f64::from(b.y),
            f64::from(a.z) - f64::from(b.z),
        ]
    }

    fn cross3_wide(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    }

    fn dot3_wide(axis: Vec3, point: [f64; 3]) -> f64 {
        f64::from(axis.x) * point[0] + f64::from(axis.y) * point[1] + f64::from(axis.z) * point[2]
    }

    fn length3_wide(v: [f64; 3]) -> f64 {
        v[0].hypot(v[1]).hypot(v[2])
    }

    fn normalized3_wide(v: [f64; 3], minimum_length: f64) -> Option<Vec3> {
        let length = length3_wide(v);
        (length > minimum_length && length.is_finite()).then(|| Vec3 {
            x: (v[0] / length) as f32,
            y: (v[1] / length) as f32,
            z: (v[2] / length) as f32,
        })
    }

    fn face_normal3(face: &[Vec3; 3]) -> Option<Vec3> {
        normalized3_wide(
            cross3_wide(delta3_wide(face[1], face[0]), delta3_wide(face[2], face[0])),
            0.0,
        )
    }

    fn normalized3(v: Vec3) -> Option<Vec3> {
        normalized3_wide([f64::from(v.x), f64::from(v.y), f64::from(v.z)], 0.0)
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

    fn point_tolerance(points: &[Vec3]) -> f64 {
        let mut minimum = [f64::INFINITY; 3];
        let mut maximum = [f64::NEG_INFINITY; 3];
        let mut coordinate_scale = 0.0_f64;
        for point in points {
            for (index, value) in [point.x, point.y, point.z].into_iter().enumerate() {
                let value = f64::from(value);
                minimum[index] = minimum[index].min(value);
                maximum[index] = maximum[index].max(value);
                coordinate_scale = coordinate_scale.max(value.abs());
            }
        }
        let extent = (maximum[0] - minimum[0])
            .max(maximum[1] - minimum[1])
            .max(maximum[2] - minimum[2]);
        extent * 1.0e-5 + coordinate_scale * f64::from(f32::EPSILON) * 8.0
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
        let extent = (f64::from(maximum[0]) - f64::from(minimum[0]))
            .max(f64::from(maximum[1]) - f64::from(minimum[1]))
            .max(f64::from(maximum[2]) - f64::from(minimum[2]));
        if !extent.is_finite() || extent <= 0.0 {
            return None;
        }
        let tolerance = point_tolerance(points);
        let minimum_normal = extent * extent * 1.0e-7;
        let mut faces = Vec::new();
        for i in 0..points.len() - 2 {
            for j in i + 1..points.len() - 1 {
                for k in j + 1..points.len() {
                    let raw_normal = cross3_wide(
                        delta3_wide(points[j], points[i]),
                        delta3_wide(points[k], points[i]),
                    );
                    if length3_wide(raw_normal) <= minimum_normal {
                        continue;
                    }
                    let Some(normal) = normalized3_wide(raw_normal, minimum_normal) else {
                        continue;
                    };
                    let mut positive = false;
                    let mut negative = false;
                    for point in points {
                        let delta = delta3_wide(*point, points[i]);
                        let distance = f64::from(normal.x) * delta[0]
                            + f64::from(normal.y) * delta[1]
                            + f64::from(normal.z) * delta[2];
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

    fn push_cross_axis(axes: &mut Vec<Vec3>, edge_a: [f64; 3], edge_b: [f64; 3]) {
        let Some(axis) = normalized3_wide(cross3_wide(edge_a, edge_b), 0.0) else {
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
        for face in a_faces.iter().chain(b_faces) {
            if let Some(normal) = face_normal3(face) {
                push_axis(&mut axes, normal);
            }
        }
        let edges = |faces: &[[Vec3; 3]]| {
            faces
                .iter()
                .flat_map(|face| {
                    [
                        delta3_wide(face[1], face[0]),
                        delta3_wide(face[2], face[1]),
                        delta3_wide(face[0], face[2]),
                    ]
                })
                .collect::<Vec<_>>()
        };
        let a_edges = edges(a_faces);
        let b_edges = edges(b_faces);
        for edge_a in &a_edges {
            for edge_b in &b_edges {
                push_cross_axis(&mut axes, *edge_a, *edge_b);
            }
        }
        axes
    }

    fn project3(points: &[Vec3], axis: Vec3) -> (f64, f64) {
        let project = |point: Vec3| {
            dot3_wide(
                axis,
                [f64::from(point.x), f64::from(point.y), f64::from(point.z)],
            )
        };
        let first = project(points[0]);
        points
            .iter()
            .skip(1)
            .fold((first, first), |(min, max), point| {
                let value = project(*point);
                (min.min(value), max.max(value))
            })
    }

    fn point3_wide(point: Vec3) -> [f64; 3] {
        [f64::from(point.x), f64::from(point.y), f64::from(point.z)]
    }

    fn dot_wide(a: [f64; 3], b: [f64; 3]) -> f64 {
        a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    }

    fn add_scaled3_wide(origin: [f64; 3], a: [f64; 3], sa: f64, b: [f64; 3], sb: f64) -> [f64; 3] {
        [
            origin[0] + a[0] * sa + b[0] * sb,
            origin[1] + a[1] * sa + b[1] * sb,
            origin[2] + a[2] * sa + b[2] * sb,
        ]
    }

    fn offset3_wide(origin: [f64; 3], delta: [f64; 3], scale: f64) -> [f64; 3] {
        [
            origin[0] + delta[0] * scale,
            origin[1] + delta[1] * scale,
            origin[2] + delta[2] * scale,
        ]
    }

    fn centroid3_wide(points: &[Vec3]) -> [f64; 3] {
        let sum = points.iter().fold([0.0; 3], |mut sum, point| {
            sum[0] += f64::from(point.x);
            sum[1] += f64::from(point.y);
            sum[2] += f64::from(point.z);
            sum
        });
        let count = points.len() as f64;
        [sum[0] / count, sum[1] / count, sum[2] / count]
    }

    fn closest_on_triangle(point: Vec3, a: Vec3, b: Vec3, c: Vec3) -> [f64; 3] {
        let point = point3_wide(point);
        let a = point3_wide(a);
        let b = point3_wide(b);
        let c = point3_wide(c);
        let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let ap = [point[0] - a[0], point[1] - a[1], point[2] - a[2]];
        let d1 = dot_wide(ab, ap);
        let d2 = dot_wide(ac, ap);
        if d1 <= 0.0 && d2 <= 0.0 {
            return a;
        }
        let bp = [point[0] - b[0], point[1] - b[1], point[2] - b[2]];
        let d3 = dot_wide(ab, bp);
        let d4 = dot_wide(ac, bp);
        if d3 >= 0.0 && d4 <= d3 {
            return b;
        }
        let vc = d1 * d4 - d3 * d2;
        if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
            return offset3_wide(a, ab, d1 / (d1 - d3));
        }
        let cp = [point[0] - c[0], point[1] - c[1], point[2] - c[2]];
        let d5 = dot_wide(ab, cp);
        let d6 = dot_wide(ac, cp);
        if d6 >= 0.0 && d5 <= d6 {
            return c;
        }
        let vb = d5 * d2 - d1 * d6;
        if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
            return offset3_wide(a, ac, d2 / (d2 - d6));
        }
        let va = d3 * d6 - d5 * d4;
        if va <= 0.0 && d4 - d3 >= 0.0 && d5 - d6 >= 0.0 {
            let bc = [c[0] - b[0], c[1] - b[1], c[2] - b[2]];
            return offset3_wide(b, bc, (d4 - d3) / ((d4 - d3) + (d5 - d6)));
        }
        let denominator = 1.0 / (va + vb + vc);
        add_scaled3_wide(a, ab, vb * denominator, ac, vc * denominator)
    }

    fn translation3_wide(direction: [f64; 3], magnitude: f64) -> Vec3 {
        let translation = [
            direction[0] * magnitude,
            direction[1] * magnitude,
            direction[2] * magnitude,
        ];
        let max = f64::from(f32::MAX);
        if translation
            .into_iter()
            .all(|component| component.is_finite() && component.abs() <= max)
        {
            Vec3 {
                x: translation[0] as f32,
                y: translation[1] as f32,
                z: translation[2] as f32,
            }
        } else {
            zero3()
        }
    }

    fn sphere_poly_translation(sphere: &Hitbox3D, poly: &Hitbox3D) -> Option<Vec3> {
        let (radius, center) = sphere3(sphere)?;
        let points = vertices3(poly)?;
        let faces = hull_faces(&points)?;
        let mut inside = true;
        let center_wide = point3_wide(center);
        let mut nearest = center_wide;
        let mut nearest_distance = f64::INFINITY;
        let mut nearest_outward = zero3();
        let tolerance = point_tolerance(&points);
        let centroid = centroid3_wide(&points);
        for face in &faces {
            let normal = face_normal3(face)?;
            let face_origin = point3_wide(face[0]);
            let outward_delta = [
                face_origin[0] - centroid[0],
                face_origin[1] - centroid[1],
                face_origin[2] - centroid[2],
            ];
            let outward = if dot3_wide(normal, outward_delta) < 0.0 {
                scale3(normal, -1.0)
            } else {
                normal
            };
            if dot3_wide(outward, delta3_wide(center, face[0])) > tolerance {
                inside = false;
            }
            let closest = closest_on_triangle(center, face[0], face[1], face[2]);
            let distance = length3_wide([
                closest[0] - center_wide[0],
                closest[1] - center_wide[1],
                closest[2] - center_wide[2],
            ]);
            if distance < nearest_distance {
                nearest_distance = distance;
                nearest = closest;
                nearest_outward = outward;
            }
        }
        if !nearest_distance.is_finite() || (!inside && nearest_distance > f64::from(radius)) {
            return None;
        }
        let direction = if inside {
            [
                nearest[0] - center_wide[0],
                nearest[1] - center_wide[1],
                nearest[2] - center_wide[2],
            ]
        } else {
            [
                center_wide[0] - nearest[0],
                center_wide[1] - nearest[1],
                center_wide[2] - nearest[2],
            ]
        };
        let direction_length = length3_wide(direction);
        let direction = if direction_length > 0.0 && direction_length.is_finite() {
            [
                direction[0] / direction_length,
                direction[1] / direction_length,
                direction[2] / direction_length,
            ]
        } else {
            [
                f64::from(nearest_outward.x),
                f64::from(nearest_outward.y),
                f64::from(nearest_outward.z),
            ]
        };
        let magnitude = if inside {
            nearest_distance + f64::from(radius)
        } else {
            f64::from(radius) - nearest_distance
        };
        Some(translation3_wide(direction, magnitude))
    }

    pub(super) fn touching_sphere_poly_normal(a: &Hitbox3D, b: &Hitbox3D) -> Option<Vec3> {
        let (sphere, poly, sphere_is_a) = match (sphere3(a), sphere3(b)) {
            (Some(_), _) => (a, b, true),
            (_, Some(_)) => (b, a, false),
            _ => return None,
        };
        let (radius, center) = sphere3(sphere)?;
        let points = vertices3(poly)?;
        let faces = hull_faces(&points)?;
        let tolerance = point_tolerance(&points);
        let centroid = centroid3_wide(&points);
        let center_wide = point3_wide(center);
        let mut inside = true;
        let mut closest = None;
        let mut nearest_distance = f64::INFINITY;
        for face in &faces {
            let normal = face_normal3(face)?;
            let face_origin = point3_wide(face[0]);
            let outward_delta = [
                face_origin[0] - centroid[0],
                face_origin[1] - centroid[1],
                face_origin[2] - centroid[2],
            ];
            let outward = if dot3_wide(normal, outward_delta) < 0.0 {
                scale3(normal, -1.0)
            } else {
                normal
            };
            if dot3_wide(outward, delta3_wide(center, face[0])) > tolerance {
                inside = false;
            }
            let point = closest_on_triangle(center, face[0], face[1], face[2]);
            let distance = length3_wide([
                point[0] - center_wide[0],
                point[1] - center_wide[1],
                point[2] - center_wide[2],
            ]);
            if distance < nearest_distance {
                nearest_distance = distance;
                closest = Some(point);
            }
        }
        if (nearest_distance - f64::from(radius)).abs() > tolerance.max(f64::from(radius) * 1.0e-6)
        {
            return None;
        }
        let closest = closest?;
        let direction = if inside {
            [
                closest[0] - center_wide[0],
                closest[1] - center_wide[1],
                closest[2] - center_wide[2],
            ]
        } else {
            [
                center_wide[0] - closest[0],
                center_wide[1] - closest[1],
                center_wide[2] - closest[2],
            ]
        };
        let direction = normalized3_wide(direction, 0.0)?;
        Some(if sphere_is_a {
            direction
        } else {
            scale3(direction, -1.0)
        })
    }

    fn minimum_translation_3d(a: &Hitbox3D, b: &Hitbox3D) -> Option<Vec3> {
        if let (Some((ra, ca)), Some((rb, cb))) = (sphere3(a), sphere3(b)) {
            let delta_x = f64::from(ca.x) - f64::from(cb.x);
            let delta_y = f64::from(ca.y) - f64::from(cb.y);
            let delta_z = f64::from(ca.z) - f64::from(cb.z);
            let distance = delta_x.hypot(delta_y).hypot(delta_z);
            let overlap = f64::from(ra) + f64::from(rb) - distance;
            if overlap < 0.0 {
                return None;
            }
            let (direction_x, direction_y, direction_z) = if distance == 0.0 {
                (1.0, 0.0, 0.0)
            } else {
                (delta_x / distance, delta_y / distance, delta_z / distance)
            };
            let translation_x = direction_x * overlap;
            let translation_y = direction_y * overlap;
            let translation_z = direction_z * overlap;
            let max = f64::from(f32::MAX);
            let translation = if [translation_x, translation_y, translation_z]
                .into_iter()
                .all(|component| component.is_finite() && component.abs() <= max)
            {
                Vec3 {
                    x: translation_x as f32,
                    y: translation_y as f32,
                    z: translation_z as f32,
                }
            } else {
                zero3()
            };
            return Some(translation);
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
        let mut best: Option<([f64; 3], f64)> = None;
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
            let candidate = [
                f64::from(axis.x) * translation,
                f64::from(axis.y) * translation,
                f64::from(axis.z) * translation,
            ];
            let length_squared = candidate[0] * candidate[0]
                + candidate[1] * candidate[1]
                + candidate[2] * candidate[2];
            if best.map_or(true, |(_, current_length_squared)| {
                length_squared < current_length_squared
            }) {
                best = Some((candidate, length_squared));
            }
        }
        let (translation, _) = best?;
        let max = f64::from(f32::MAX);
        if !translation
            .into_iter()
            .all(|component| component.is_finite() && component.abs() <= max)
        {
            return Some(zero3());
        }
        Some(Vec3 {
            x: translation[0] as f32,
            y: translation[1] as f32,
            z: translation[2] as f32,
        })
    }

    pub(super) fn touching_poly_normal_3d(a: &Hitbox3D, b: &Hitbox3D) -> Option<Vec3> {
        let a_points = vertices3(a)?;
        let b_points = vertices3(b)?;
        let a_faces = hull_faces(&a_points)?;
        let b_faces = hull_faces(&b_points)?;
        for axis in poly_axes(&a_faces, &b_faces) {
            let (a_min, a_max) = project3(&a_points, axis);
            let (b_min, b_max) = project3(&b_points, axis);
            if a_max == b_min {
                return Some(scale3(axis, -1.0));
            }
            if b_max == a_min {
                return Some(axis);
            }
        }
        None
    }
}

use super::math::{Vec2, Vec3};
use crate::physics::hitbox::{Hitbox2D, Hitbox3D};

fn velocity_after_impulse_component(current: f32, impulse: f32, mass: f64) -> Option<f32> {
    if !current.is_finite() || !impulse.is_finite() || !mass.is_finite() || mass <= 0.0 {
        return None;
    }
    let next = f64::from(current) + f64::from(impulse) / mass;
    (next.is_finite() && next.abs() <= f64::from(f32::MAX)).then_some(next as f32)
}

fn checked_f32(value: f64) -> Option<f32> {
    (value.is_finite() && value.abs() <= f64::from(f32::MAX)).then_some(value as f32)
}

fn velocity_after_delta(current: f32, delta: f64) -> Option<f32> {
    if !current.is_finite() {
        return None;
    }
    checked_f32(f64::from(current) + delta)
}

fn velocity_after_force_component(
    current: f32,
    force: f32,
    mass: f64,
    delta_time: f32,
) -> Option<f32> {
    if !force.is_finite()
        || !mass.is_finite()
        || mass <= 0.0
        || !delta_time.is_finite()
        || delta_time < 0.0
    {
        return None;
    }
    velocity_after_delta(current, f64::from(force) * f64::from(delta_time) / mass)
}

fn touching_circle_normal(a: &Hitbox2D, b: &Hitbox2D) -> Option<Vec2> {
    let (Hitbox2D::Circle(radius_a, center_a), Hitbox2D::Circle(radius_b, center_b)) = (a, b)
    else {
        return None;
    };
    if !radius_a.is_finite()
        || *radius_a < 0.0
        || !radius_b.is_finite()
        || *radius_b < 0.0
        || !center_a.x.is_finite()
        || !center_a.y.is_finite()
        || !center_b.x.is_finite()
        || !center_b.y.is_finite()
    {
        return None;
    }
    let dx = f64::from(center_a.x) - f64::from(center_b.x);
    let dy = f64::from(center_a.y) - f64::from(center_b.y);
    let length = dx.hypot(dy);
    if length == 0.0 {
        Some(Vec2 { x: 1.0, y: 0.0 })
    } else {
        Some(Vec2 {
            x: (dx / length) as f32,
            y: (dy / length) as f32,
        })
    }
}

fn touching_sphere_normal(a: &Hitbox3D, b: &Hitbox3D) -> Option<Vec3> {
    let (Hitbox3D::Sphere(radius_a, center_a), Hitbox3D::Sphere(radius_b, center_b)) = (a, b)
    else {
        return None;
    };
    if !radius_a.is_finite()
        || *radius_a < 0.0
        || !radius_b.is_finite()
        || *radius_b < 0.0
        || !center_a.x.is_finite()
        || !center_a.y.is_finite()
        || !center_a.z.is_finite()
        || !center_b.x.is_finite()
        || !center_b.y.is_finite()
        || !center_b.z.is_finite()
    {
        return None;
    }
    let dx = f64::from(center_a.x) - f64::from(center_b.x);
    let dy = f64::from(center_a.y) - f64::from(center_b.y);
    let dz = f64::from(center_a.z) - f64::from(center_b.z);
    let length = dx.hypot(dy).hypot(dz);
    if length == 0.0 {
        Some(Vec3 {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        })
    } else {
        Some(Vec3 {
            x: (dx / length) as f32,
            y: (dy / length) as f32,
            z: (dz / length) as f32,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Soa)]
pub struct PhysicsObject2D {
    pub hitbox: hitbox::Hitbox2D,
    /// Collision mass in kilograms; zero marks this body static.
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

    /// Returns the world-space origin of this body's hitbox.
    pub fn position(&self) -> Vec2 {
        self.hitbox.origin()
    }

    /// Applies an instantaneous impulse in newton-seconds, using `mass` to change velocity.
    /// Returns false without changing velocity for invalid mass, input, or an unrepresentable result.
    pub fn apply_impulse(&mut self, impulse: Vec2) -> bool {
        let Some(x) = velocity_after_impulse_component(self.velocity.x, impulse.x, self.mass)
        else {
            return false;
        };
        let Some(y) = velocity_after_impulse_component(self.velocity.y, impulse.y, self.mass)
        else {
            return false;
        };
        self.velocity = Vec2 { x, y };
        true
    }

    /// Applies a constant force in newtons over `delta_time` seconds using `mass` in kilograms.
    /// Returns false without changing velocity for invalid inputs or an unrepresentable result.
    pub fn apply_force(&mut self, force: Vec2, delta_time: f32) -> bool {
        let Some(x) =
            velocity_after_force_component(self.velocity.x, force.x, self.mass, delta_time)
        else {
            return false;
        };
        let Some(y) =
            velocity_after_force_component(self.velocity.y, force.y, self.mass, delta_time)
        else {
            return false;
        };
        self.velocity = Vec2 { x, y };
        true
    }

    /// Separates overlapping hitboxes and applies mass-weighted normal and friction impulses.
    /// `friction` is a finite, nonnegative Coulomb coefficient; angular motion and plasticity
    /// are not modeled. Invalid or unrepresentable collisions return false
    /// without changing either object. Exact circle and convex polygon contacts use geometric
    /// normals; other touching shapes need a contact manifold.
    pub fn resolve_collision(&mut self, other: &mut Self, friction: f32) -> bool {
        let separation = self.hitbox.separation(&other.hitbox);
        let zero_separation = separation.x == 0.0 && separation.y == 0.0;
        let contact_normal = (zero_separation && self.hitbox.intersects(&other.hitbox))
            .then(|| {
                touching_circle_normal(&self.hitbox, &other.hitbox)
                    .or_else(|| hitbox::touching_circle_poly_normal(&self.hitbox, &other.hitbox))
                    .or_else(|| hitbox::touching_polygon_normal(&self.hitbox, &other.hitbox))
            })
            .flatten();
        if !separation.x.is_finite()
            || !separation.y.is_finite()
            || (zero_separation && contact_normal.is_none())
            || !self.mass.is_finite()
            || self.mass < 0.0
            || !other.mass.is_finite()
            || other.mass < 0.0
            || !self.elasticity.is_finite()
            || self.elasticity < 0.0
            || !other.elasticity.is_finite()
            || other.elasticity < 0.0
            || !friction.is_finite()
            || friction < 0.0
        {
            return false;
        }

        let inverse_self = if self.mass == 0.0 {
            0.0
        } else {
            1.0 / self.mass
        };
        let inverse_other = if other.mass == 0.0 {
            0.0
        } else {
            1.0 / other.mass
        };
        let inverse_total = inverse_self + inverse_other;
        if !inverse_total.is_finite() || inverse_total <= 0.0 {
            return false;
        }
        let self_share = inverse_self / inverse_total;
        let other_share = inverse_other / inverse_total;
        let self_velocity = if self.mass == 0.0 {
            Vec2 { x: 0.0, y: 0.0 }
        } else {
            self.velocity
        };
        let other_velocity = if other.mass == 0.0 {
            Vec2 { x: 0.0, y: 0.0 }
        } else {
            other.velocity
        };

        let length = f64::from(separation.x).hypot(f64::from(separation.y));
        if !length.is_finite() || (length <= 0.0 && contact_normal.is_none()) {
            return false;
        }
        let (normal_x, normal_y) = if length > 0.0 {
            (
                f64::from(separation.x) / length,
                f64::from(separation.y) / length,
            )
        } else {
            let Some(normal) = contact_normal else {
                return false;
            };
            (f64::from(normal.x), f64::from(normal.y))
        };
        let Some(self_dx) = checked_f32(f64::from(separation.x) * self_share) else {
            return false;
        };
        let Some(self_dy) = checked_f32(f64::from(separation.y) * self_share) else {
            return false;
        };
        let Some(other_dx) = checked_f32(-f64::from(separation.x) * other_share) else {
            return false;
        };
        let Some(other_dy) = checked_f32(-f64::from(separation.y) * other_share) else {
            return false;
        };

        let mut self_hitbox = self.hitbox.clone();
        let mut other_hitbox = other.hitbox.clone();
        self_hitbox.translate_by(Vec2 {
            x: self_dx,
            y: self_dy,
        });
        other_hitbox.translate_by(Vec2 {
            x: other_dx,
            y: other_dy,
        });
        let residual = self_hitbox.separation(&other_hitbox);
        if residual.x != 0.0 || residual.y != 0.0 {
            return false;
        }

        let relative_x = f64::from(self_velocity.x) - f64::from(other_velocity.x);
        let relative_y = f64::from(self_velocity.y) - f64::from(other_velocity.y);
        let relative_normal_velocity = relative_x * normal_x + relative_y * normal_y;
        if !relative_normal_velocity.is_finite() {
            return false;
        }
        let tangent_x = relative_x - relative_normal_velocity * normal_x;
        let tangent_y = relative_y - relative_normal_velocity * normal_y;
        let tangent_speed = tangent_x.hypot(tangent_y);
        if !tangent_speed.is_finite() {
            return false;
        }
        let restitution = self.elasticity.min(other.elasticity).min(1.0);
        let impact_speed = if relative_normal_velocity < 0.0 {
            -(1.0 + restitution) * relative_normal_velocity
        } else {
            0.0
        };
        let max_friction_impulse = f64::from(friction) * impact_speed / inverse_total;
        if !max_friction_impulse.is_finite() {
            return false;
        }
        let friction_impulse = if tangent_speed > 0.0 {
            (-tangent_speed / inverse_total).clamp(-max_friction_impulse, max_friction_impulse)
        } else {
            0.0
        };
        let tangent_unit_x = if tangent_speed > 0.0 {
            tangent_x / tangent_speed
        } else {
            0.0
        };
        let tangent_unit_y = if tangent_speed > 0.0 {
            tangent_y / tangent_speed
        } else {
            0.0
        };
        let Some(self_vx) = velocity_after_delta(
            self_velocity.x,
            impact_speed * self_share * normal_x + friction_impulse * inverse_self * tangent_unit_x,
        ) else {
            return false;
        };
        let Some(self_vy) = velocity_after_delta(
            self_velocity.y,
            impact_speed * self_share * normal_y + friction_impulse * inverse_self * tangent_unit_y,
        ) else {
            return false;
        };
        let Some(other_vx) = velocity_after_delta(
            other_velocity.x,
            -impact_speed * other_share * normal_x
                - friction_impulse * inverse_other * tangent_unit_x,
        ) else {
            return false;
        };
        let Some(other_vy) = velocity_after_delta(
            other_velocity.y,
            -impact_speed * other_share * normal_y
                - friction_impulse * inverse_other * tangent_unit_y,
        ) else {
            return false;
        };

        self.hitbox = self_hitbox;
        other.hitbox = other_hitbox;
        self.velocity = Vec2 {
            x: self_vx,
            y: self_vy,
        };
        other.velocity = Vec2 {
            x: other_vx,
            y: other_vy,
        };
        true
    }

    /// Advances dynamic hitboxes; static bodies keep their position and have velocity cleared.
    /// Negative or non-finite time steps and invalid masses are ignored.
    pub fn integrate(&mut self, delta_time: f32) {
        if !delta_time.is_finite() || delta_time < 0.0 || !self.mass.is_finite() || self.mass < 0.0
        {
            return;
        }
        if self.mass == 0.0 {
            self.velocity = Vec2 { x: 0.0, y: 0.0 };
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

#[derive(Debug, Clone, PartialEq, Soa)]
pub struct PhysicsObject3D {
    pub hitbox: hitbox::Hitbox3D,
    /// Collision mass in kilograms; zero marks this body static.
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

    /// Returns the world-space origin of this body's hitbox.
    pub fn position(&self) -> Vec3 {
        self.hitbox.origin()
    }

    /// Applies an instantaneous impulse in newton-seconds, using `mass` to change velocity.
    /// Returns false without changing velocity for invalid mass, input, or an unrepresentable result.
    pub fn apply_impulse(&mut self, impulse: Vec3) -> bool {
        let Some(x) = velocity_after_impulse_component(self.velocity.x, impulse.x, self.mass)
        else {
            return false;
        };
        let Some(y) = velocity_after_impulse_component(self.velocity.y, impulse.y, self.mass)
        else {
            return false;
        };
        let Some(z) = velocity_after_impulse_component(self.velocity.z, impulse.z, self.mass)
        else {
            return false;
        };
        self.velocity = Vec3 { x, y, z };
        true
    }

    /// Applies a constant force in newtons over `delta_time` seconds using `mass` in kilograms.
    /// Returns false without changing velocity for invalid inputs or an unrepresentable result.
    pub fn apply_force(&mut self, force: Vec3, delta_time: f32) -> bool {
        let Some(x) =
            velocity_after_force_component(self.velocity.x, force.x, self.mass, delta_time)
        else {
            return false;
        };
        let Some(y) =
            velocity_after_force_component(self.velocity.y, force.y, self.mass, delta_time)
        else {
            return false;
        };
        let Some(z) =
            velocity_after_force_component(self.velocity.z, force.z, self.mass, delta_time)
        else {
            return false;
        };
        self.velocity = Vec3 { x, y, z };
        true
    }

    /// Separates overlapping hitboxes and applies mass-weighted normal and friction impulses.
    /// `friction` is a finite, nonnegative Coulomb coefficient; angular motion and plasticity
    /// are not modeled. Invalid or unrepresentable collisions return false
    /// without changing either object. Exact sphere and convex polygon contacts use geometric
    /// normals; other touching shapes need a contact manifold.
    pub fn resolve_collision(&mut self, other: &mut Self, friction: f32) -> bool {
        let separation = self.hitbox.separation(&other.hitbox);
        let zero_separation = separation.x == 0.0 && separation.y == 0.0 && separation.z == 0.0;
        let contact_normal = (zero_separation && self.hitbox.intersects(&other.hitbox))
            .then(|| {
                touching_sphere_normal(&self.hitbox, &other.hitbox)
                    .or_else(|| hitbox::touching_sphere_poly_normal(&self.hitbox, &other.hitbox))
                    .or_else(|| hitbox::touching_poly_normal_3d(&self.hitbox, &other.hitbox))
            })
            .flatten();
        if !separation.x.is_finite()
            || !separation.y.is_finite()
            || !separation.z.is_finite()
            || (zero_separation && contact_normal.is_none())
            || !self.mass.is_finite()
            || self.mass < 0.0
            || !other.mass.is_finite()
            || other.mass < 0.0
            || !self.elasticity.is_finite()
            || self.elasticity < 0.0
            || !other.elasticity.is_finite()
            || other.elasticity < 0.0
            || !friction.is_finite()
            || friction < 0.0
        {
            return false;
        }

        let inverse_self = if self.mass == 0.0 {
            0.0
        } else {
            1.0 / self.mass
        };
        let inverse_other = if other.mass == 0.0 {
            0.0
        } else {
            1.0 / other.mass
        };
        let inverse_total = inverse_self + inverse_other;
        if !inverse_total.is_finite() || inverse_total <= 0.0 {
            return false;
        }
        let self_share = inverse_self / inverse_total;
        let other_share = inverse_other / inverse_total;
        let self_velocity = if self.mass == 0.0 {
            Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            }
        } else {
            self.velocity
        };
        let other_velocity = if other.mass == 0.0 {
            Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            }
        } else {
            other.velocity
        };

        let length = f64::from(separation.x)
            .hypot(f64::from(separation.y))
            .hypot(f64::from(separation.z));
        if !length.is_finite() || (length <= 0.0 && contact_normal.is_none()) {
            return false;
        }
        let (normal_x, normal_y, normal_z) = if length > 0.0 {
            (
                f64::from(separation.x) / length,
                f64::from(separation.y) / length,
                f64::from(separation.z) / length,
            )
        } else {
            let Some(normal) = contact_normal else {
                return false;
            };
            (
                f64::from(normal.x),
                f64::from(normal.y),
                f64::from(normal.z),
            )
        };
        let Some(self_dx) = checked_f32(f64::from(separation.x) * self_share) else {
            return false;
        };
        let Some(self_dy) = checked_f32(f64::from(separation.y) * self_share) else {
            return false;
        };
        let Some(self_dz) = checked_f32(f64::from(separation.z) * self_share) else {
            return false;
        };
        let Some(other_dx) = checked_f32(-f64::from(separation.x) * other_share) else {
            return false;
        };
        let Some(other_dy) = checked_f32(-f64::from(separation.y) * other_share) else {
            return false;
        };
        let Some(other_dz) = checked_f32(-f64::from(separation.z) * other_share) else {
            return false;
        };

        let mut self_hitbox = self.hitbox.clone();
        let mut other_hitbox = other.hitbox.clone();
        self_hitbox.translate_by(Vec3 {
            x: self_dx,
            y: self_dy,
            z: self_dz,
        });
        other_hitbox.translate_by(Vec3 {
            x: other_dx,
            y: other_dy,
            z: other_dz,
        });
        let residual = self_hitbox.separation(&other_hitbox);
        if residual.x != 0.0 || residual.y != 0.0 || residual.z != 0.0 {
            return false;
        }

        let relative_x = f64::from(self_velocity.x) - f64::from(other_velocity.x);
        let relative_y = f64::from(self_velocity.y) - f64::from(other_velocity.y);
        let relative_z = f64::from(self_velocity.z) - f64::from(other_velocity.z);
        let relative_normal_velocity =
            relative_x * normal_x + relative_y * normal_y + relative_z * normal_z;
        if !relative_normal_velocity.is_finite() {
            return false;
        }
        let tangent_x = relative_x - relative_normal_velocity * normal_x;
        let tangent_y = relative_y - relative_normal_velocity * normal_y;
        let tangent_z = relative_z - relative_normal_velocity * normal_z;
        let tangent_speed = tangent_x.hypot(tangent_y).hypot(tangent_z);
        if !tangent_speed.is_finite() {
            return false;
        }
        let restitution = self.elasticity.min(other.elasticity).min(1.0);
        let impact_speed = if relative_normal_velocity < 0.0 {
            -(1.0 + restitution) * relative_normal_velocity
        } else {
            0.0
        };
        let max_friction_impulse = f64::from(friction) * impact_speed / inverse_total;
        if !max_friction_impulse.is_finite() {
            return false;
        }
        let friction_impulse = if tangent_speed > 0.0 {
            (-tangent_speed / inverse_total).clamp(-max_friction_impulse, max_friction_impulse)
        } else {
            0.0
        };
        let tangent_unit_x = if tangent_speed > 0.0 {
            tangent_x / tangent_speed
        } else {
            0.0
        };
        let tangent_unit_y = if tangent_speed > 0.0 {
            tangent_y / tangent_speed
        } else {
            0.0
        };
        let tangent_unit_z = if tangent_speed > 0.0 {
            tangent_z / tangent_speed
        } else {
            0.0
        };
        let Some(self_vx) = velocity_after_delta(
            self_velocity.x,
            impact_speed * self_share * normal_x + friction_impulse * inverse_self * tangent_unit_x,
        ) else {
            return false;
        };
        let Some(self_vy) = velocity_after_delta(
            self_velocity.y,
            impact_speed * self_share * normal_y + friction_impulse * inverse_self * tangent_unit_y,
        ) else {
            return false;
        };
        let Some(self_vz) = velocity_after_delta(
            self_velocity.z,
            impact_speed * self_share * normal_z + friction_impulse * inverse_self * tangent_unit_z,
        ) else {
            return false;
        };
        let Some(other_vx) = velocity_after_delta(
            other_velocity.x,
            -impact_speed * other_share * normal_x
                - friction_impulse * inverse_other * tangent_unit_x,
        ) else {
            return false;
        };
        let Some(other_vy) = velocity_after_delta(
            other_velocity.y,
            -impact_speed * other_share * normal_y
                - friction_impulse * inverse_other * tangent_unit_y,
        ) else {
            return false;
        };
        let Some(other_vz) = velocity_after_delta(
            other_velocity.z,
            -impact_speed * other_share * normal_z
                - friction_impulse * inverse_other * tangent_unit_z,
        ) else {
            return false;
        };

        self.hitbox = self_hitbox;
        other.hitbox = other_hitbox;
        self.velocity = Vec3 {
            x: self_vx,
            y: self_vy,
            z: self_vz,
        };
        other.velocity = Vec3 {
            x: other_vx,
            y: other_vy,
            z: other_vz,
        };
        true
    }

    /// Advances dynamic hitboxes; static bodies keep their position and have velocity cleared.
    /// Negative or non-finite time steps and invalid masses are ignored.
    pub fn integrate(&mut self, delta_time: f32) {
        if !delta_time.is_finite() || delta_time < 0.0 || !self.mass.is_finite() || self.mass < 0.0
        {
            return;
        }
        if self.mass == 0.0 {
            self.velocity = Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            };
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

/// A SoA-compatible body that can be advanced by a physics world.
pub trait PhysicsBody: Soa + SoaClone {
    fn integrate_body(&mut self, delta_time: f32);
    fn resolve_body_collision(&mut self, other: &mut Self, friction: f32) -> bool;

    /// Recommends bounded subdivisions for linear motion before collision checks.
    /// Custom bodies default to one; override when their bounds can tunnel between steps.
    fn recommended_substeps(&self, _delta_time: f32) -> usize {
        1
    }

    /// Returns conservative world-space bounds, or `None` when unknown.
    fn broadphase_bounds(&self) -> Option<[(f64, f64); 3]> {
        None
    }
}

impl PhysicsBody for PhysicsObject2D {
    fn integrate_body(&mut self, delta_time: f32) {
        self.integrate(delta_time);
    }
    fn resolve_body_collision(&mut self, other: &mut Self, friction: f32) -> bool {
        self.resolve_collision(other, friction)
    }
    fn recommended_substeps(&self, delta_time: f32) -> usize {
        if !self.mass.is_finite() || self.mass <= 0.0 {
            return 1;
        }
        let speed = f64::from(self.velocity.x).hypot(f64::from(self.velocity.y));
        required_collision_substeps(self.hitbox.broadphase_bounds(), speed, delta_time, 2)
    }
    fn broadphase_bounds(&self) -> Option<[(f64, f64); 3]> {
        self.hitbox.broadphase_bounds()
    }
}

impl PhysicsBody for PhysicsObject3D {
    fn integrate_body(&mut self, delta_time: f32) {
        self.integrate(delta_time);
    }
    fn resolve_body_collision(&mut self, other: &mut Self, friction: f32) -> bool {
        self.resolve_collision(other, friction)
    }
    fn recommended_substeps(&self, delta_time: f32) -> usize {
        if !self.mass.is_finite() || self.mass <= 0.0 {
            return 1;
        }
        let speed = f64::from(self.velocity.x)
            .hypot(f64::from(self.velocity.y))
            .hypot(f64::from(self.velocity.z));
        required_collision_substeps(self.hitbox.broadphase_bounds(), speed, delta_time, 3)
    }
    fn broadphase_bounds(&self) -> Option<[(f64, f64); 3]> {
        self.hitbox.broadphase_bounds()
    }
}

const BOUNDS_TREE_MIN_BODY_COUNT: usize = 512;
const BOUNDS_TREE_DENSITY_SAMPLE_ROWS: usize = 32;
const BOUNDS_TREE_MAX_SAMPLED_PAIR_DENSITY_PERCENT: usize = 1;
const MAX_PHYSICS_SUBSTEPS: usize = 1024;

fn required_collision_substeps(
    bounds: Option<[(f64, f64); 3]>,
    speed: f64,
    delta_time: f32,
    dimensions: usize,
) -> usize {
    let Some(bounds) = bounds else {
        return 1;
    };
    if !speed.is_finite() || speed <= 0.0 || !delta_time.is_finite() || delta_time <= 0.0 {
        return 1;
    }
    let minimum_extent = bounds[..dimensions]
        .iter()
        .map(|(minimum, maximum)| maximum - minimum)
        .fold(f64::INFINITY, f64::min);
    if !minimum_extent.is_finite() {
        return 1;
    }
    if minimum_extent <= 0.0 {
        // No finite discrete stride can guarantee contact for a degenerate moving bound.
        return MAX_PHYSICS_SUBSTEPS + 1;
    }
    let required = (speed * f64::from(delta_time) / (minimum_extent * 0.5)).ceil();
    if !required.is_finite() || required > MAX_PHYSICS_SUBSTEPS as f64 {
        MAX_PHYSICS_SUBSTEPS + 1
    } else {
        (required as usize).max(1)
    }
}

#[derive(Default)]
struct BoundsTree {
    nodes: Vec<BoundsTreeNode>,
    leaves: Vec<usize>,
    order: Vec<usize>,
    root: Option<usize>,
}

struct BoundsTreeNode {
    bounds: [(f64, f64); 3],
    left: Option<usize>,
    right: Option<usize>,
    parent: Option<usize>,
    body_index: Option<usize>,
    max_body_index: usize,
}

impl BoundsTree {
    fn clear(&mut self) {
        self.nodes.clear();
        self.leaves.clear();
        self.order.clear();
        self.root = None;
    }

    fn prepare_capacity(&mut self, body_count: usize) -> io::Result<()> {
        let node_count = if body_count == 0 {
            0
        } else {
            body_count
                .checked_mul(2)
                .and_then(|count| count.checked_sub(1))
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "too many physics bodies")
                })?
        };

        self.clear();
        self.nodes
            .try_reserve(node_count)
            .map_err(|error| io::Error::other(error.to_string()))?;
        self.leaves
            .try_reserve(body_count)
            .map_err(|error| io::Error::other(error.to_string()))?;
        self.order
            .try_reserve(body_count)
            .map_err(|error| io::Error::other(error.to_string()))?;
        Ok(())
    }

    fn rebuild(&mut self, bounds: &[[(f64, f64); 3]]) {
        self.leaves.resize(bounds.len(), 0);
        self.order.extend(0..bounds.len());
        self.order.sort_unstable_by(|left, right| {
            bounds[*left][0]
                .0
                .total_cmp(&bounds[*right][0].0)
                .then_with(|| left.cmp(right))
        });
        if !self.order.is_empty() {
            let order = std::mem::take(&mut self.order);
            self.root = Some(self.build_range(&order, bounds));
            self.order = order;
        }
    }

    fn build_range(&mut self, order: &[usize], bounds: &[[(f64, f64); 3]]) -> usize {
        if order.len() == 1 {
            let body_index = order[0];
            let node_index = self.nodes.len();
            self.nodes.push(BoundsTreeNode {
                bounds: bounds[body_index],
                left: None,
                right: None,
                parent: None,
                body_index: Some(body_index),
                max_body_index: body_index,
            });
            self.leaves[body_index] = node_index;
            return node_index;
        }

        let middle = order.len() / 2;
        let left = self.build_range(&order[..middle], bounds);
        let right = self.build_range(&order[middle..], bounds);
        let node_index = self.nodes.len();
        self.nodes.push(BoundsTreeNode {
            bounds: bounds_union(self.nodes[left].bounds, self.nodes[right].bounds),
            left: Some(left),
            right: Some(right),
            parent: None,
            body_index: None,
            max_body_index: self.nodes[left]
                .max_body_index
                .max(self.nodes[right].max_body_index),
        });
        self.nodes[left].parent = Some(node_index);
        self.nodes[right].parent = Some(node_index);
        node_index
    }

    fn query(
        &self,
        bounds: [(f64, f64); 3],
        after_body_index: usize,
        candidates: &mut Vec<usize>,
    ) -> usize {
        fn visit(
            tree: &BoundsTree,
            node_index: usize,
            bounds: [(f64, f64); 3],
            after_body_index: usize,
            candidates: &mut Vec<usize>,
            visited: &mut usize,
        ) {
            *visited += 1;
            let node = &tree.nodes[node_index];
            if node.max_body_index <= after_body_index || !bounds_overlap(node.bounds, bounds) {
                return;
            }
            if let Some(body_index) = node.body_index {
                candidates.push(body_index);
                return;
            }
            if let Some(left) = node.left {
                visit(tree, left, bounds, after_body_index, candidates, visited);
            }
            if let Some(right) = node.right {
                visit(tree, right, bounds, after_body_index, candidates, visited);
            }
        }

        let mut visited = 0;
        if let Some(root) = self.root {
            visit(
                self,
                root,
                bounds,
                after_body_index,
                candidates,
                &mut visited,
            );
        }
        visited
    }

    fn update(&mut self, body_index: usize, bounds: [(f64, f64); 3]) {
        let leaf = self.leaves[body_index];
        self.nodes[leaf].bounds = bounds;
        let mut parent = self.nodes[leaf].parent;
        while let Some(node_index) = parent {
            let node = &self.nodes[node_index];
            let left = node.left.expect("internal tree nodes have a left child");
            let right = node.right.expect("internal tree nodes have a right child");
            let next_parent = node.parent;
            let merged = bounds_union(self.nodes[left].bounds, self.nodes[right].bounds);
            self.nodes[node_index].bounds = merged;
            parent = next_parent;
        }
    }
}

fn bounds_union(left: [(f64, f64); 3], right: [(f64, f64); 3]) -> [(f64, f64); 3] {
    std::array::from_fn(|axis| {
        (
            left[axis].0.min(right[axis].0),
            left[axis].1.max(right[axis].1),
        )
    })
}

struct PhysicsRowsRestore<'a, T: PhysicsBody> {
    bodies: &'a mut Ecs<T>,
    scratch: &'a mut Vec<T>,
    scratch_is_ordered: bool,
}

impl<T: PhysicsBody> PhysicsRowsRestore<'_, T> {
    fn restore(&mut self) {
        if self.scratch_is_ordered {
            self.scratch.reverse();
            self.scratch_is_ordered = false;
        }
        while let Some(body) = self.scratch.pop() {
            self.bodies
                .push(body)
                .expect("restoring a row must fit the body's existing ECS capacity");
        }
    }
}

impl<T: PhysicsBody> Drop for PhysicsRowsRestore<'_, T> {
    fn drop(&mut self) {
        self.restore();
    }
}

/// Reusable integration and collision scratch for caller-owned ECS bodies.
pub struct PhysicsStepper<T: PhysicsBody> {
    scratch: Vec<T>,
    bounds_scratch: Vec<[(f64, f64); 3]>,
    bounds_tree: BoundsTree,
    candidate_scratch: Vec<usize>,
}

impl<T: PhysicsBody> PhysicsStepper<T> {
    pub fn new() -> Self {
        Self {
            scratch: Vec::new(),
            bounds_scratch: Vec::new(),
            bounds_tree: BoundsTree::default(),
            candidate_scratch: Vec::new(),
        }
    }
}

impl<T: PhysicsBody> Default for PhysicsStepper<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// ECS-backed bodies with reusable scratch for stable-order simulation.
pub struct PhysicsWorld<T: PhysicsBody> {
    bodies: Ecs<T>,
    stepper: PhysicsStepper<T>,
}

pub type PhysicsWorld2D = PhysicsWorld<PhysicsObject2D>;
pub type PhysicsWorld3D = PhysicsWorld<PhysicsObject3D>;

impl<T: PhysicsBody> PhysicsWorld<T> {
    pub fn new() -> Self {
        Self {
            bodies: Ecs::new(),
            stepper: PhysicsStepper::new(),
        }
    }

    /// Creates a growable world with storage initially sized for `capacity` bodies.
    pub fn with_growable_capacity(capacity: usize) -> io::Result<Self> {
        Ok(Self {
            bodies: Ecs::with_growable_capacity(capacity)?,
            stepper: PhysicsStepper::new(),
        })
    }

    /// Adds a body and returns its current row index.
    pub fn add(&mut self, body: T) -> io::Result<usize> {
        let index = self.bodies.len();
        self.bodies.push(body)?;
        Ok(index)
    }

    /// Returns a cloned body at `index`.
    pub fn get_cloned(&self, index: usize) -> Option<T> {
        self.bodies.get_cloned(index)
    }

    /// Replaces a body at `index`, returning the old body or the input if replacement fails.
    pub fn try_replace(&mut self, index: usize, body: T) -> Result<T, T> {
        self.bodies.try_replace(index, body)
    }

    /// Removes a body; later row indices shift down by one.
    pub fn remove(&mut self, index: usize) -> Option<T> {
        self.bodies.remove(index)
    }

    pub fn len(&self) -> usize {
        self.bodies.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bodies.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.bodies.capacity()
    }

    /// Integrates every body once while preserving insertion order.
    /// Scratch allocation failure is reported before any body is removed.
    pub fn integrate_all(&mut self, delta_time: f32) -> io::Result<()> {
        self.stepper.integrate_all(&mut self.bodies, delta_time)
    }

    /// Advances bodies and resolves pairs in lexicographic row order using adaptive substeps.
    /// Larger sparse worlds use a rebuilt AABB tree; dense worlds use ordered pair checks.
    /// Dense or unknown-bound scenes may remain O(n²).
    /// Built-in bodies cap motion at 1,024 substeps; exceeding that limit, scratch allocation
    /// failure, or invalid friction is rejected before body mutation. Invalid time steps are ignored.
    pub fn step(&mut self, delta_time: f32, friction: f32) -> io::Result<usize> {
        self.stepper.step(&mut self.bodies, delta_time, friction)
    }

    #[cfg(test)]
    fn step_with_tree(
        &mut self,
        delta_time: f32,
        friction: f32,
        use_tree: bool,
    ) -> io::Result<usize> {
        self.stepper
            .step_with_broadphase(&mut self.bodies, delta_time, friction, Some(use_tree))
    }
}

impl<T: PhysicsBody> PhysicsStepper<T> {
    /// Integrates every row in `bodies` once, preserving row order.
    /// Negative or non-finite time steps are ignored. Scratch allocation failure is reported
    /// before any ECS row is removed.
    pub fn integrate_all(&mut self, bodies: &mut Ecs<T>, delta_time: f32) -> io::Result<()> {
        if !delta_time.is_finite() || delta_time < 0.0 {
            return Ok(());
        }
        self.scratch.clear();
        self.scratch
            .try_reserve(bodies.len())
            .map_err(|error| io::Error::other(error.to_string()))?;

        let mut rows = PhysicsRowsRestore {
            bodies,
            scratch: &mut self.scratch,
            scratch_is_ordered: false,
        };
        while let Some(body) = rows.bodies.pop() {
            rows.scratch.push(body);
            rows.scratch
                .last_mut()
                .expect("a body was just added to physics scratch")
                .integrate_body(delta_time);
        }
        rows.restore();
        Ok(())
    }

    /// Integrates `bodies`, then resolves candidate row pairs in lexicographic order.
    /// Negative or non-finite time steps are ignored. Scratch allocation failure and invalid
    /// friction are rejected before mutation.
    pub fn step(
        &mut self,
        bodies: &mut Ecs<T>,
        delta_time: f32,
        friction: f32,
    ) -> io::Result<usize> {
        self.step_with_broadphase(bodies, delta_time, friction, None)
    }

    fn step_with_broadphase(
        &mut self,
        bodies: &mut Ecs<T>,
        delta_time: f32,
        friction: f32,
        forced_tree: Option<bool>,
    ) -> io::Result<usize> {
        if !friction.is_finite() || friction < 0.0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "physics friction must be finite and nonnegative",
            ));
        }
        if !delta_time.is_finite() || delta_time < 0.0 {
            return Ok(0);
        }

        let use_tree = forced_tree.unwrap_or_else(|| {
            bodies.len() >= BOUNDS_TREE_MIN_BODY_COUNT && sampled_body_bounds_are_sparse(bodies)
        });
        self.scratch.clear();
        self.scratch
            .try_reserve(bodies.len())
            .map_err(|error| io::Error::other(error.to_string()))?;
        self.bounds_scratch.clear();
        self.bounds_scratch
            .try_reserve(bodies.len())
            .map_err(|error| io::Error::other(error.to_string()))?;
        if use_tree {
            self.bounds_tree.prepare_capacity(bodies.len())?;
            self.candidate_scratch.clear();
            self.candidate_scratch
                .try_reserve(bodies.len())
                .map_err(|error| io::Error::other(error.to_string()))?;
        } else {
            self.bounds_tree.clear();
        }

        let mut rows = PhysicsRowsRestore {
            bodies,
            scratch: &mut self.scratch,
            scratch_is_ordered: false,
        };
        while let Some(body) = rows.bodies.pop() {
            rows.scratch.push(body);
        }
        rows.scratch.reverse();
        rows.scratch_is_ordered = true;
        let substep_count = rows
            .scratch
            .iter()
            .map(|body| body.recommended_substeps(delta_time))
            .max()
            .unwrap_or(1);
        if substep_count > MAX_PHYSICS_SUBSTEPS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "physics step requires more than the supported 1024 collision substeps",
            ));
        }
        let substep_delta_time = delta_time / substep_count as f32;
        let mut resolved_pairs = 0usize;
        for _ in 0..substep_count {
            for body in rows.scratch.iter_mut() {
                body.integrate_body(substep_delta_time);
            }
            self.bounds_scratch.clear();
            self.bounds_scratch
                .extend(rows.scratch.iter().map(conservative_bounds));
            if use_tree {
                self.bounds_tree.rebuild(&self.bounds_scratch);
            }
            for first_index in 0..rows.scratch.len() {
                let mut after_second_index = first_index;
                let mut refresh_candidates = true;
                let mut candidate_offset = 0;
                while after_second_index + 1 < rows.scratch.len() {
                    let second_index = if use_tree {
                        if refresh_candidates {
                            self.candidate_scratch.clear();
                            self.bounds_tree.query(
                                self.bounds_scratch[first_index],
                                after_second_index,
                                &mut self.candidate_scratch,
                            );
                            self.candidate_scratch.sort_unstable();
                            candidate_offset = 0;
                        }
                        let Some(second_index) =
                            self.candidate_scratch.get(candidate_offset).copied()
                        else {
                            break;
                        };
                        candidate_offset += 1;
                        second_index
                    } else {
                        after_second_index + 1
                    };
                    if !use_tree
                        && !bounds_overlap(
                            self.bounds_scratch[first_index],
                            self.bounds_scratch[second_index],
                        )
                    {
                        after_second_index = second_index;
                        continue;
                    }
                    let first_previous_bounds = self.bounds_scratch[first_index];
                    let (through_first, after_first) = rows.scratch.split_at_mut(first_index + 1);
                    let first = &mut through_first[first_index];
                    let second = &mut after_first[second_index - first_index - 1];
                    let resolved = first.resolve_body_collision(second, friction);
                    self.bounds_scratch[first_index] = conservative_bounds(first);
                    self.bounds_scratch[second_index] = conservative_bounds(second);
                    if use_tree {
                        self.bounds_tree
                            .update(first_index, self.bounds_scratch[first_index]);
                        self.bounds_tree
                            .update(second_index, self.bounds_scratch[second_index]);
                    }
                    after_second_index = second_index;
                    refresh_candidates =
                        use_tree && self.bounds_scratch[first_index] != first_previous_bounds;
                    if resolved {
                        resolved_pairs = resolved_pairs.saturating_add(1);
                    }
                }
            }
        }

        rows.restore();
        Ok(resolved_pairs)
    }
}

fn conservative_bounds<T: PhysicsBody>(body: &T) -> [(f64, f64); 3] {
    match body.broadphase_bounds() {
        Some(bounds)
            if bounds
                .iter()
                .all(|(min, max)| min.is_finite() && max.is_finite() && min <= max) =>
        {
            bounds
        }
        _ => [(f64::NEG_INFINITY, f64::INFINITY); 3],
    }
}

fn bounds_overlap(left: [(f64, f64); 3], right: [(f64, f64); 3]) -> bool {
    left.into_iter()
        .zip(right)
        .all(|((left_min, left_max), (right_min, right_max))| {
            left_max >= right_min && right_max >= left_min
        })
}

fn sampled_body_bounds_are_sparse<T: PhysicsBody>(bodies: &Ecs<T>) -> bool {
    let body_count = bodies.len();
    let sample_count = BOUNDS_TREE_DENSITY_SAMPLE_ROWS.min(body_count);
    if sample_count < 2 {
        return false;
    }

    let sampled_pair_count = sample_count * (sample_count - 1) / 2;
    let maximum_overlap_count =
        sampled_pair_count * BOUNDS_TREE_MAX_SAMPLED_PAIR_DENSITY_PERCENT / 100;
    let mut overlapping_sample_pairs = 0;
    let mut sample_indices = [0; BOUNDS_TREE_DENSITY_SAMPLE_ROWS];
    let mut sample_bounds =
        [[(f64::NEG_INFINITY, f64::INFINITY); 3]; BOUNDS_TREE_DENSITY_SAMPLE_ROWS];
    for sample_index in 0..sample_count {
        // Scramble row indexes so regular cluster layouts do not alias a fixed stride.
        let mut hash = (sample_index as u64).wrapping_add(0x9E37_79B9_7F4A_7C15);
        hash = (hash ^ (hash >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        hash = (hash ^ (hash >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        hash ^= hash >> 31;
        let mut body_index = (hash % body_count as u64) as usize;
        while sample_indices[..sample_index].contains(&body_index) {
            body_index = (body_index + 1) % body_count;
        }
        sample_indices[sample_index] = body_index;
        let Some(body) = bodies.get_cloned(body_index) else {
            return false;
        };
        sample_bounds[sample_index] = conservative_bounds(&body);
    }

    for first_index in 0..sample_count {
        for second_index in first_index + 1..sample_count {
            if bounds_overlap(sample_bounds[first_index], sample_bounds[second_index]) {
                overlapping_sample_pairs += 1;
                if overlapping_sample_pairs > maximum_overlap_count {
                    return false;
                }
            }
        }
    }

    true
}

impl<T: PhysicsBody> Default for PhysicsWorld<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod broadphase_tests {
    use super::hitbox::{Hitbox, Hitbox2D, Hitbox3D};
    use super::{
        BoundsTree, PhysicsBody, PhysicsObject2D, PhysicsObject3D, bounds_overlap,
        conservative_bounds,
    };
    use super::{PhysicsWorld, PhysicsWorld2D, PhysicsWorld3D};
    use crate::api::ecs::Soa;
    use crate::math::{Vec2, Vec3};
    use std::cell::{Cell, RefCell};
    use std::hint::black_box;
    use std::rc::Rc;
    use std::time::{Duration, Instant};

    #[derive(Clone, PartialEq, Soa)]
    struct CountingBody {
        id: usize,
        bounds: Option<[(f64, f64); 3]>,
        resolver_calls: Rc<Cell<usize>>,
        resolver_trace: Rc<Cell<u64>>,
        move_other_to: Option<[(f64, f64); 3]>,
    }

    impl PhysicsBody for CountingBody {
        fn integrate_body(&mut self, _delta_time: f32) {}

        fn resolve_body_collision(&mut self, other: &mut Self, _friction: f32) -> bool {
            self.resolver_calls
                .set(self.resolver_calls.get().saturating_add(1));
            self.resolver_trace.set(
                self.resolver_trace
                    .get()
                    .wrapping_mul(100)
                    .wrapping_add((self.id * 10 + other.id) as u64),
            );
            if let Some(bounds) = self.move_other_to {
                other.bounds = Some(bounds);
            }
            false
        }

        fn broadphase_bounds(&self) -> Option<[(f64, f64); 3]> {
            self.bounds
        }
    }

    #[derive(Clone, PartialEq, Soa)]
    struct OrderedPairBody {
        id: usize,
        bounds: Option<[(f64, f64); 3]>,
        resolver_trace: Rc<RefCell<Vec<(usize, usize)>>>,
    }

    impl PhysicsBody for OrderedPairBody {
        fn integrate_body(&mut self, _delta_time: f32) {}

        fn resolve_body_collision(&mut self, other: &mut Self, _friction: f32) -> bool {
            self.resolver_trace.borrow_mut().push((self.id, other.id));
            false
        }

        fn broadphase_bounds(&self) -> Option<[(f64, f64); 3]> {
            self.bounds
        }
    }

    #[derive(Clone, PartialEq, Soa)]
    struct PanickingBody {
        id: usize,
        panic_on_integrate: bool,
        panic_on_resolve: bool,
    }

    impl PhysicsBody for PanickingBody {
        fn integrate_body(&mut self, _delta_time: f32) {
            assert!(!self.panic_on_integrate, "intentional integration panic");
        }

        fn resolve_body_collision(&mut self, _other: &mut Self, _friction: f32) -> bool {
            assert!(!self.panic_on_resolve, "intentional collision panic");
            false
        }

        fn broadphase_bounds(&self) -> Option<[(f64, f64); 3]> {
            Some([(0.0, 1.0); 3])
        }
    }

    fn fixture_position(scene: &str, body_index: usize, is_3d: bool) -> [f32; 3] {
        let group_size = match scene {
            "clustered" | "cluster64" => Some(64),
            "cluster8" => Some(8),
            "cluster16" => Some(16),
            "cluster32" => Some(32),
            "cluster128" => Some(128),
            _ => None,
        };
        if let Some(group_size) = group_size {
            let cluster = body_index / group_size;
            let local = body_index % group_size;
            return if is_3d {
                [
                    cluster as f32 * 4.0 + (local % 4) as f32 * 0.05,
                    ((local / 4) % 4) as f32 * 0.05,
                    (local / 16) as f32 * 0.05,
                ]
            } else {
                [
                    cluster as f32 * 4.0 + (local % 8) as f32 * 0.05,
                    (local / 8) as f32 * 0.05,
                    0.0,
                ]
            };
        }

        match scene {
            "sparse" => [body_index as f32 * 4.0, 0.0, 0.0],
            "dense" if is_3d => [
                (body_index % 8) as f32 * 0.02,
                ((body_index / 8) % 8) as f32 * 0.02,
                (body_index / 64) as f32 * 0.02,
            ],
            "dense" => [
                (body_index % 32) as f32 * 0.01,
                (body_index / 32) as f32 * 0.01,
                0.0,
            ],
            _ => panic!("unknown physics benchmark scene: {scene}"),
        }
    }

    fn build_2d_fixture(scene: &str, body_count: usize) -> PhysicsWorld2D {
        let mut world = PhysicsWorld2D::with_growable_capacity(body_count).unwrap();
        for body_index in 0..body_count {
            let [x, y, _] = fixture_position(scene, body_index, false);
            world
                .add(PhysicsObject2D::new(
                    Hitbox2D::circle(0.5, Vec2 { x, y }),
                    0.0,
                    0.0,
                    0.0,
                    Vec2 { x: 0.0, y: 0.0 },
                ))
                .unwrap();
        }
        world
    }

    fn build_3d_fixture(scene: &str, body_count: usize) -> PhysicsWorld3D {
        let mut world = PhysicsWorld3D::with_growable_capacity(body_count).unwrap();
        for body_index in 0..body_count {
            let [x, y, z] = fixture_position(scene, body_index, true);
            world
                .add(PhysicsObject3D::new(
                    Hitbox3D::sphere(0.5, Vec3 { x, y, z }),
                    0.0,
                    0.0,
                    0.0,
                    Vec3 {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                ))
                .unwrap();
        }
        world
    }

    fn build_dynamic_2d_fixture(body_count: usize) -> PhysicsWorld2D {
        assert_eq!(body_count % 2, 0);
        let mut world = PhysicsWorld2D::with_growable_capacity(body_count).unwrap();
        for body_index in 0..body_count {
            let pair_index = body_index / 2;
            let moving = body_index % 2 == 0;
            let x = pair_index as f32 * 4.0 + if moving { 0.0 } else { 0.8 };
            world
                .add(PhysicsObject2D::new(
                    Hitbox2D::circle(0.5, Vec2 { x, y: 0.0 }),
                    if moving { 1.0 } else { 0.0 },
                    0.5,
                    0.0,
                    Vec2 {
                        x: if moving { 1.0 } else { 0.0 },
                        y: 0.0,
                    },
                ))
                .unwrap();
        }
        world
    }

    fn build_dynamic_3d_fixture(body_count: usize) -> PhysicsWorld3D {
        assert_eq!(body_count % 2, 0);
        let mut world = PhysicsWorld3D::with_growable_capacity(body_count).unwrap();
        for body_index in 0..body_count {
            let pair_index = body_index / 2;
            let moving = body_index % 2 == 0;
            let x = pair_index as f32 * 4.0 + if moving { 0.0 } else { 0.8 };
            world
                .add(PhysicsObject3D::new(
                    Hitbox3D::sphere(0.5, Vec3 { x, y: 0.0, z: 0.0 }),
                    if moving { 1.0 } else { 0.0 },
                    0.5,
                    0.0,
                    Vec3 {
                        x: if moving { 1.0 } else { 0.0 },
                        y: 0.0,
                        z: 0.0,
                    },
                ))
                .unwrap();
        }
        world
    }

    fn assert_dynamic_strategies_match<T>(
        mut naive: PhysicsWorld<T>,
        mut tree: PhysicsWorld<T>,
        mut automatic: PhysicsWorld<T>,
    ) where
        T: PhysicsBody + std::fmt::Debug + PartialEq,
    {
        let naive_resolved = naive.step_with_tree(0.01, 0.1, false).unwrap();
        let tree_resolved = tree.step_with_tree(0.01, 0.1, true).unwrap();
        let automatic_resolved = automatic.step(0.01, 0.1).unwrap();
        assert!(
            automatic.stepper.bounds_tree.root.is_some(),
            "sparse moving contacts should select the tree"
        );
        assert!(
            naive_resolved > 0,
            "fixture must exercise active resolution"
        );
        assert_eq!(tree_resolved, naive_resolved);
        assert_eq!(automatic_resolved, naive_resolved);
        for body_index in 0..naive.len() {
            let expected = naive.get_cloned(body_index);
            assert_eq!(tree.get_cloned(body_index), expected);
            assert_eq!(automatic.get_cloned(body_index), expected);
        }
    }

    fn assert_forced_strategies_match<T>(mut naive: PhysicsWorld<T>, mut tree: PhysicsWorld<T>)
    where
        T: PhysicsBody + std::fmt::Debug + PartialEq,
    {
        let naive_resolved = naive.step_with_tree(0.0, 0.0, false).unwrap();
        let tree_resolved = tree.step_with_tree(0.0, 0.0, true).unwrap();
        assert_eq!(tree_resolved, naive_resolved);
        for body_index in 0..naive.len() {
            assert_eq!(
                tree.get_cloned(body_index),
                naive.get_cloned(body_index),
                "body {body_index} differs between broad-phase strategies"
            );
        }
    }

    #[test]
    fn builtin_2d_and_3d_bodies_match_across_broadphase_strategies() {
        for scene in [
            "sparse",
            "cluster8",
            "cluster16",
            "cluster32",
            "cluster64",
            "cluster128",
            "dense",
        ] {
            assert_forced_strategies_match(
                build_2d_fixture(scene, 512),
                build_2d_fixture(scene, 512),
            );
            assert_forced_strategies_match(
                build_3d_fixture(scene, 512),
                build_3d_fixture(scene, 512),
            );
            let expected_tree = scene == "sparse";
            let mut automatic_2d = build_2d_fixture(scene, 512);
            automatic_2d.step(0.0, 0.0).unwrap();
            assert_eq!(
                automatic_2d.stepper.bounds_tree.root.is_some(),
                expected_tree
            );
            let mut automatic_3d = build_3d_fixture(scene, 512);
            automatic_3d.step(0.0, 0.0).unwrap();
            assert_eq!(
                automatic_3d.stepper.bounds_tree.root.is_some(),
                expected_tree
            );
        }
    }

    #[test]
    fn moving_builtin_collisions_match_across_broadphase_strategies() {
        const BODY_COUNT: usize = 512;

        assert_dynamic_strategies_match(
            build_dynamic_2d_fixture(BODY_COUNT),
            build_dynamic_2d_fixture(BODY_COUNT),
            build_dynamic_2d_fixture(BODY_COUNT),
        );
        assert_dynamic_strategies_match(
            build_dynamic_3d_fixture(BODY_COUNT),
            build_dynamic_3d_fixture(BODY_COUNT),
            build_dynamic_3d_fixture(BODY_COUNT),
        );
    }

    #[test]
    fn automatic_step_checks_all_pairs_when_custom_body_bounds_are_unknown() {
        const BODY_COUNT: usize = 512;
        let calls = Rc::new(Cell::new(0));
        let trace = Rc::new(Cell::new(0));
        let mut world = PhysicsWorld::<CountingBody>::with_growable_capacity(BODY_COUNT).unwrap();
        for id in 0..BODY_COUNT {
            world
                .add(CountingBody {
                    id,
                    bounds: None,
                    resolver_calls: Rc::clone(&calls),
                    resolver_trace: Rc::clone(&trace),
                    move_other_to: None,
                })
                .unwrap();
        }

        assert_eq!(world.step(0.0, 0.0).unwrap(), 0);
        assert!(world.stepper.bounds_tree.root.is_none());
        assert_eq!(calls.get(), BODY_COUNT * (BODY_COUNT - 1) / 2);
    }

    #[test]
    fn automatic_unknown_bounds_resolve_pairs_in_lexicographic_order() {
        const BODY_COUNT: usize = 512;
        let trace = Rc::new(RefCell::new(Vec::new()));
        let mut world =
            PhysicsWorld::<OrderedPairBody>::with_growable_capacity(BODY_COUNT).unwrap();
        for id in 0..BODY_COUNT {
            world
                .add(OrderedPairBody {
                    id,
                    bounds: None,
                    resolver_trace: Rc::clone(&trace),
                })
                .unwrap();
        }

        assert_eq!(world.step(0.0, 0.0).unwrap(), 0);
        assert!(world.stepper.bounds_tree.root.is_none());
        let expected: Vec<_> = (0..BODY_COUNT)
            .flat_map(|first| (first + 1..BODY_COUNT).map(move |second| (first, second)))
            .collect();
        assert_eq!(*trace.borrow(), expected);
    }

    #[test]
    fn automatic_step_visits_ordered_pairs_for_invalid_bounds_at_tree_threshold() {
        const BODY_COUNT: usize = super::BOUNDS_TREE_MIN_BODY_COUNT;
        let expected: Vec<_> = (0..BODY_COUNT)
            .flat_map(|first| (first + 1..BODY_COUNT).map(move |second| (first, second)))
            .collect();

        for bounds in [
            [(0.0, 1.0), (2.0, 1.0), (0.0, 0.0)],
            [(0.0, f64::NAN), (0.0, 1.0), (0.0, 0.0)],
        ] {
            let trace = Rc::new(RefCell::new(Vec::new()));
            let mut world =
                PhysicsWorld::<OrderedPairBody>::with_growable_capacity(BODY_COUNT).unwrap();
            for id in 0..BODY_COUNT {
                world
                    .add(OrderedPairBody {
                        id,
                        bounds: Some(bounds),
                        resolver_trace: Rc::clone(&trace),
                    })
                    .unwrap();
            }

            assert_eq!(world.step(0.0, 0.0).unwrap(), 0);
            assert!(world.stepper.bounds_tree.root.is_none());
            assert_eq!(*trace.borrow(), expected);
        }
    }

    #[test]
    fn broad_phase_skips_separated_axes_but_keeps_touching_pairs() {
        let calls = Rc::new(Cell::new(0));
        for second_bounds in [
            [(0.0, 1.0), (2.0, 3.0), (0.0, 0.0)],
            [(0.0, 1.0), (0.0, 1.0), (2.0, 3.0)],
        ] {
            let mut separated = PhysicsWorld::<CountingBody>::new();
            for (id, bounds) in [
                Some([(0.0, 1.0), (0.0, 1.0), (0.0, 0.0)]),
                Some(second_bounds),
            ]
            .into_iter()
            .enumerate()
            {
                separated
                    .add(CountingBody {
                        id,
                        bounds,
                        resolver_calls: Rc::clone(&calls),
                        resolver_trace: Rc::new(Cell::new(0)),
                        move_other_to: None,
                    })
                    .unwrap();
            }
            assert_eq!(separated.step(0.0, 0.0).unwrap(), 0);
            assert_eq!(calls.get(), 0);
        }

        let mut touching = PhysicsWorld::<CountingBody>::new();
        for (id, bounds) in [
            Some([(0.0, 1.0), (0.0, 1.0), (0.0, 0.0)]),
            Some([(0.0, 1.0), (1.0, 2.0), (0.0, 0.0)]),
        ]
        .into_iter()
        .enumerate()
        {
            touching
                .add(CountingBody {
                    id,
                    bounds,
                    resolver_calls: Rc::clone(&calls),
                    resolver_trace: Rc::new(Cell::new(0)),
                    move_other_to: None,
                })
                .unwrap();
        }
        assert_eq!(touching.step(0.0, 0.0).unwrap(), 0);
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn physics_stepper_restores_ecs_rows_when_body_callbacks_panic() {
        let build_world = |panic_on_integrate, panic_on_resolve| {
            let mut world = PhysicsWorld::<PanickingBody>::new();
            for id in 0..2 {
                world
                    .add(PanickingBody {
                        id,
                        panic_on_integrate: panic_on_integrate == Some(id),
                        panic_on_resolve: panic_on_resolve == Some(id),
                    })
                    .unwrap();
            }
            world
        };
        let assert_rows_restored = |world: &PhysicsWorld<PanickingBody>| {
            assert_eq!(world.len(), 2);
            assert_eq!(world.get_cloned(0).unwrap().id, 0);
            assert_eq!(world.get_cloned(1).unwrap().id, 1);
        };

        let mut integrate_all_world = build_world(Some(1), None);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            integrate_all_world.integrate_all(1.0).unwrap();
        }));
        assert!(result.is_err());
        assert_rows_restored(&integrate_all_world);

        let mut step_integration_world = build_world(Some(1), None);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            step_integration_world.step(1.0, 0.0).unwrap();
        }));
        assert!(result.is_err());
        assert_rows_restored(&step_integration_world);

        let mut collision_world = build_world(None, Some(0));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            collision_world.step(0.0, 0.0).unwrap();
        }));
        assert!(result.is_err());
        assert_rows_restored(&collision_world);
    }

    #[test]
    fn broad_phase_refreshes_bounds_after_mutating_resolver() {
        let calls = Rc::new(Cell::new(0));
        let trace = Rc::new(Cell::new(0));
        let initial = [(0.0, 1.0), (0.0, 1.0), (0.0, 0.0)];
        let destination = [(3.0, 4.0), (0.0, 1.0), (0.0, 0.0)];
        let mut world = PhysicsWorld::<CountingBody>::new();

        for (id, (bounds, move_other_to)) in [
            (Some(initial), Some(destination)),
            (Some(initial), None),
            (Some(destination), None),
        ]
        .into_iter()
        .enumerate()
        {
            world
                .add(CountingBody {
                    id,
                    bounds,
                    resolver_calls: Rc::clone(&calls),
                    resolver_trace: Rc::clone(&trace),
                    move_other_to,
                })
                .unwrap();
        }

        assert_eq!(world.step(0.0, 0.0).unwrap(), 0);
        assert_eq!(calls.get(), 2);
        assert_eq!(trace.get(), 112);
    }

    #[test]
    fn builtin_dynamic_collision_refreshes_tree_for_a_later_contact() {
        let build_world = || {
            let mut world = PhysicsWorld2D::new();
            for center in [0.0, 0.9, -1.05] {
                world
                    .add(PhysicsObject2D::new(
                        Hitbox2D::circle(0.5, Vec2 { x: center, y: 0.0 }),
                        if center == 0.0 { 1.0 } else { 0.0 },
                        0.0,
                        0.0,
                        Vec2 { x: 0.0, y: 0.0 },
                    ))
                    .unwrap();
            }
            world
        };
        let mut naive = build_world();
        let mut tree = build_world();
        assert!(
            !naive
                .get_cloned(0)
                .unwrap()
                .hitbox
                .intersects(&naive.get_cloned(2).unwrap().hitbox)
        );

        assert_eq!(naive.step_with_tree(0.0, 0.0, false).unwrap(), 2);
        assert_eq!(tree.step_with_tree(0.0, 0.0, true).unwrap(), 2);
        for body_index in 0..naive.len() {
            assert_eq!(tree.get_cloned(body_index), naive.get_cloned(body_index));
        }
    }

    #[test]
    fn builtin_dynamic_3d_collision_refreshes_tree_for_a_later_contact() {
        let build_world = || {
            let mut world = PhysicsWorld3D::new();
            for center in [0.0, 0.9, -1.05] {
                world
                    .add(PhysicsObject3D::new(
                        Hitbox3D::sphere(
                            0.5,
                            Vec3 {
                                x: center,
                                y: 0.0,
                                z: 0.0,
                            },
                        ),
                        if center == 0.0 { 1.0 } else { 0.0 },
                        0.0,
                        0.0,
                        Vec3 {
                            x: 0.0,
                            y: 0.0,
                            z: 0.0,
                        },
                    ))
                    .unwrap();
            }
            world
        };
        let mut naive = build_world();
        let mut tree = build_world();
        assert!(
            !naive
                .get_cloned(0)
                .unwrap()
                .hitbox
                .intersects(&naive.get_cloned(2).unwrap().hitbox)
        );

        assert_eq!(naive.step_with_tree(0.0, 0.0, false).unwrap(), 2);
        assert_eq!(tree.step_with_tree(0.0, 0.0, true).unwrap(), 2);
        for body_index in 0..naive.len() {
            assert_eq!(tree.get_cloned(body_index), naive.get_cloned(body_index));
        }
    }

    #[test]
    fn builtin_dynamic_3d_quad_collision_refreshes_tree_for_a_later_contact() {
        let build_world = || {
            let mut world = PhysicsWorld3D::new();
            for center in [0.0, 0.9, -1.05] {
                world
                    .add(PhysicsObject3D::new(
                        Hitbox3D::quad_at(
                            Vec3 {
                                x: -0.5,
                                y: -0.5,
                                z: -0.5,
                            },
                            Vec3 {
                                x: 0.5,
                                y: -0.5,
                                z: -0.5,
                            },
                            Vec3 {
                                x: 0.5,
                                y: 0.5,
                                z: -0.5,
                            },
                            Vec3 {
                                x: -0.5,
                                y: 0.5,
                                z: -0.5,
                            },
                            Vec3 {
                                x: -0.5,
                                y: -0.5,
                                z: 0.5,
                            },
                            Vec3 {
                                x: 0.5,
                                y: -0.5,
                                z: 0.5,
                            },
                            Vec3 {
                                x: 0.5,
                                y: 0.5,
                                z: 0.5,
                            },
                            Vec3 {
                                x: -0.5,
                                y: 0.5,
                                z: 0.5,
                            },
                            Vec3 {
                                x: center,
                                y: 0.0,
                                z: 0.0,
                            },
                        ),
                        if center == 0.0 { 1.0 } else { 0.0 },
                        0.0,
                        0.0,
                        Vec3 {
                            x: 0.0,
                            y: 0.0,
                            z: 0.0,
                        },
                    ))
                    .unwrap();
            }
            world
        };
        let mut naive = build_world();
        let mut tree = build_world();
        assert!(
            !naive
                .get_cloned(0)
                .unwrap()
                .hitbox
                .intersects(&naive.get_cloned(2).unwrap().hitbox)
        );

        assert_eq!(naive.step_with_tree(0.0, 0.0, false).unwrap(), 2);
        assert_eq!(tree.step_with_tree(0.0, 0.0, true).unwrap(), 2);
        for body_index in 0..naive.len() {
            assert_eq!(tree.get_cloned(body_index), naive.get_cloned(body_index));
        }
    }

    #[test]
    fn tree_broad_phase_refreshes_after_mutation_at_large_world_size() {
        let calls = Rc::new(Cell::new(0));
        let trace = Rc::new(Cell::new(0));
        let initial = [(0.0, 1.0), (0.0, 1.0), (0.0, 0.0)];
        let destination = [(3.0, 4.0), (0.0, 1.0), (0.0, 0.0)];
        let mut world = PhysicsWorld::<CountingBody>::new();

        for (id, (bounds, move_other_to)) in [
            (Some(initial), Some(destination)),
            (Some(initial), None),
            (Some(destination), None),
        ]
        .into_iter()
        .enumerate()
        {
            world
                .add(CountingBody {
                    id,
                    bounds,
                    resolver_calls: Rc::clone(&calls),
                    resolver_trace: Rc::clone(&trace),
                    move_other_to,
                })
                .unwrap();
        }
        for id in 3..256 {
            let min_x = 100.0 + id as f64 * 4.0;
            world
                .add(CountingBody {
                    id,
                    bounds: Some([(min_x, min_x + 1.0), (0.0, 1.0), (0.0, 0.0)]),
                    resolver_calls: Rc::clone(&calls),
                    resolver_trace: Rc::new(Cell::new(0)),
                    move_other_to: None,
                })
                .unwrap();
        }

        assert_eq!(world.step_with_tree(0.0, 0.0, true).unwrap(), 0);
        assert!(world.stepper.bounds_tree.root.is_some());
        assert_eq!(calls.get(), 2);
        assert_eq!(trace.get(), 112);
    }

    #[test]
    fn broad_phase_falls_back_to_pair_checks_for_unknown_or_invalid_bounds() {
        let calls = Rc::new(Cell::new(0));
        for invalid_bounds in [
            None,
            Some([(0.0, 1.0), (2.0, 1.0), (0.0, 0.0)]),
            Some([(0.0, f64::NAN), (0.0, 1.0), (0.0, 0.0)]),
        ] {
            let mut world = PhysicsWorld::<CountingBody>::new();
            for (id, bounds) in [invalid_bounds, Some([(4.0, 5.0), (4.0, 5.0), (4.0, 5.0)])]
                .into_iter()
                .enumerate()
            {
                world
                    .add(CountingBody {
                        id,
                        bounds,
                        resolver_calls: Rc::clone(&calls),
                        resolver_trace: Rc::new(Cell::new(0)),
                        move_other_to: None,
                    })
                    .unwrap();
            }
            assert_eq!(world.step(0.0, 0.0).unwrap(), 0);
            assert_eq!(calls.get(), 1);
            calls.set(0);
        }
    }

    #[test]
    fn bounds_tree_prunes_distant_subtrees() {
        let bounds = (0..128)
            .map(|index| {
                let min_x = index as f64 * 4.0;
                [(min_x, min_x + 1.0), (0.0, 1.0), (0.0, 1.0)]
            })
            .collect::<Vec<_>>();
        let mut tree = BoundsTree::default();
        tree.prepare_capacity(bounds.len()).unwrap();
        tree.rebuild(&bounds);

        let mut candidates = Vec::new();
        candidates.try_reserve(bounds.len()).unwrap();
        let visited = tree.query(bounds[64], 63, &mut candidates);
        candidates.sort_unstable();

        assert_eq!(candidates, [64]);
        assert!(visited <= 20, "visited {visited} of 255 tree nodes");
    }

    #[test]
    fn small_physics_worlds_skip_tree_setup() {
        let calls = Rc::new(Cell::new(0));
        let mut world = PhysicsWorld::<CountingBody>::new();
        for id in 0..128 {
            let min_x = id as f64 * 4.0;
            world
                .add(CountingBody {
                    id,
                    bounds: Some([(min_x, min_x + 1.0), (0.0, 1.0), (0.0, 1.0)]),
                    resolver_calls: Rc::clone(&calls),
                    resolver_trace: Rc::new(Cell::new(0)),
                    move_other_to: None,
                })
                .unwrap();
        }

        assert_eq!(world.step(0.0, 0.0).unwrap(), 0);
        assert!(world.stepper.bounds_tree.root.is_none());
        assert_eq!(calls.get(), 0);
    }

    #[test]
    fn automatic_broad_phase_uses_density_to_select_traversal() {
        const BODY_COUNT: usize = 512;

        for dense in [false, true] {
            let calls = Rc::new(Cell::new(0));
            let mut world =
                PhysicsWorld::<CountingBody>::with_growable_capacity(BODY_COUNT).unwrap();
            for id in 0..BODY_COUNT {
                let bounds = if dense {
                    [(0.0, 1.0); 3]
                } else {
                    let min_x = id as f64 * 4.0;
                    [(min_x, min_x + 1.0), (0.0, 1.0), (0.0, 1.0)]
                };
                world
                    .add(CountingBody {
                        id,
                        bounds: Some(bounds),
                        resolver_calls: Rc::clone(&calls),
                        resolver_trace: Rc::new(Cell::new(0)),
                        move_other_to: None,
                    })
                    .unwrap();
            }

            assert_eq!(world.step(0.0, 0.0).unwrap(), 0);
            assert_eq!(world.stepper.bounds_tree.root.is_some(), !dense);
            assert_eq!(world.stepper.bounds_tree.nodes.capacity() > 0, !dense);
            assert_eq!(world.stepper.candidate_scratch.capacity() > 0, !dense);
            assert_eq!(
                calls.get(),
                if dense {
                    BODY_COUNT * (BODY_COUNT - 1) / 2
                } else {
                    0
                }
            );
        }
    }

    #[test]
    #[ignore = "manual release-mode sparse/dense broad-phase benchmark"]
    fn physics_broadphase_sparse_dense_microbenchmark() {
        const SAMPLES: usize = 7;
        const ITERATIONS_PER_SAMPLE: usize = 4;

        for (scene, body_count) in [
            ("sparse", 64),
            ("dense", 64),
            ("sparse", 128),
            ("dense", 128),
            ("sparse", 256),
            ("dense", 256),
            ("sparse", 512),
            ("dense", 512),
            ("sparse", 1024),
            ("dense", 1024),
            ("sparse", 2048),
        ] {
            let bounds = (0..body_count)
                .map(|index| {
                    if scene == "dense" {
                        [(0.0, 1.0); 3]
                    } else {
                        let min_x = index as f64 * 4.0;
                        [(min_x, min_x + 1.0), (0.0, 1.0), (0.0, 1.0)]
                    }
                })
                .collect::<Vec<_>>();
            let max_pairs = body_count * (body_count - 1) / 2;
            let mut naive_pairs = Vec::with_capacity(max_pairs);
            let mut tree_pairs = Vec::with_capacity(max_pairs);
            let mut candidates = Vec::with_capacity(body_count);
            let mut tree = BoundsTree::default();

            for _ in 0..2 {
                collect_naive_pairs(&bounds, &mut naive_pairs);
                collect_tree_pairs(&bounds, &mut tree, &mut candidates, &mut tree_pairs);
                assert_eq!(
                    tree_pairs, naive_pairs,
                    "candidate mismatch in {scene} scene"
                );
            }

            let mut naive_samples = Vec::with_capacity(SAMPLES);
            let mut tree_samples = Vec::with_capacity(SAMPLES);
            let mut build_samples = Vec::with_capacity(SAMPLES);
            let mut query_samples = Vec::with_capacity(SAMPLES);
            let mut visited_per_iteration = 0usize;

            for _ in 0..SAMPLES {
                let naive_start = Instant::now();
                for _ in 0..ITERATIONS_PER_SAMPLE {
                    collect_naive_pairs(&bounds, &mut naive_pairs);
                    black_box(naive_pairs.len());
                }
                naive_samples.push(naive_start.elapsed() / ITERATIONS_PER_SAMPLE as u32);

                let tree_start = Instant::now();
                let mut build_time = Duration::ZERO;
                let mut query_time = Duration::ZERO;
                let mut visited = 0usize;
                for _ in 0..ITERATIONS_PER_SAMPLE {
                    let build_start = Instant::now();
                    tree.prepare_capacity(body_count).unwrap();
                    tree.rebuild(&bounds);
                    build_time += build_start.elapsed();

                    let query_start = Instant::now();
                    collect_tree_candidates(
                        &bounds,
                        &tree,
                        &mut candidates,
                        &mut tree_pairs,
                        &mut visited,
                    );
                    query_time += query_start.elapsed();
                    black_box(tree_pairs.len());
                }
                tree_samples.push(tree_start.elapsed() / ITERATIONS_PER_SAMPLE as u32);
                build_samples.push(build_time / ITERATIONS_PER_SAMPLE as u32);
                query_samples.push(query_time / ITERATIONS_PER_SAMPLE as u32);
                visited_per_iteration = visited / ITERATIONS_PER_SAMPLE;
                assert_eq!(
                    tree_pairs, naive_pairs,
                    "candidate mismatch in {scene} scene"
                );
            }

            let candidate_count = naive_pairs.len();
            let naive_median_us = median_micros(&mut naive_samples);
            let tree_median_us = median_micros(&mut tree_samples);
            let build_median_us = median_micros(&mut build_samples);
            let query_median_us = median_micros(&mut query_samples);
            eprintln!(
                "physics broadphase scene={scene} bodies={body_count} candidates={candidate_count} tree_nodes_visited={visited_per_iteration} naive_median_us={naive_median_us:.2} tree_build_median_us={build_median_us:.2} tree_query_median_us={query_median_us:.2} tree_total_median_us={tree_median_us:.2} naive_over_tree={:.2}",
                naive_median_us / tree_median_us
            );
        }
    }

    #[test]
    #[ignore = "manual release-mode full PhysicsWorld sparse/dense step benchmark"]
    fn physics_world_step_sparse_dense_microbenchmark() {
        const SAMPLES: usize = 7;
        const ITERATIONS_PER_SAMPLE: usize = 4;

        for (scene, body_count) in [
            ("sparse", 64),
            ("dense", 64),
            ("sparse", 128),
            ("dense", 128),
            ("sparse", 256),
            ("dense", 256),
            ("sparse", 512),
            ("dense", 512),
            ("sparse", 1024),
            ("dense", 1024),
        ] {
            for (strategy, use_tree) in [("naive", false), ("tree", true)] {
                let calls = Rc::new(Cell::new(0));
                let mut world =
                    PhysicsWorld::<CountingBody>::with_growable_capacity(body_count).unwrap();
                for id in 0..body_count {
                    let bounds = if scene == "dense" {
                        [(0.0, 1.0); 3]
                    } else {
                        let min_x = id as f64 * 4.0;
                        [(min_x, min_x + 1.0), (0.0, 1.0), (0.0, 1.0)]
                    };
                    world
                        .add(CountingBody {
                            id,
                            bounds: Some(bounds),
                            resolver_calls: Rc::clone(&calls),
                            resolver_trace: Rc::new(Cell::new(0)),
                            move_other_to: None,
                        })
                        .unwrap();
                }

                let expected_calls = if scene == "dense" {
                    body_count * (body_count - 1) / 2
                } else {
                    0
                };
                assert_eq!(world.step_with_tree(0.0, 0.0, use_tree).unwrap(), 0);
                assert_eq!(calls.get(), expected_calls);

                let mut samples = Vec::with_capacity(SAMPLES);
                for _ in 0..SAMPLES {
                    calls.set(0);
                    let start = Instant::now();
                    for _ in 0..ITERATIONS_PER_SAMPLE {
                        assert_eq!(world.step_with_tree(0.0, 0.0, use_tree).unwrap(), 0);
                    }
                    samples.push(start.elapsed() / ITERATIONS_PER_SAMPLE as u32);
                    assert_eq!(calls.get(), expected_calls * ITERATIONS_PER_SAMPLE);
                }

                let median_us = median_micros(&mut samples);
                eprintln!(
                    "physics world step scene={scene} strategy={strategy} bodies={body_count} resolver_calls={expected_calls} median_us={median_us:.2}"
                );
            }
        }
    }

    fn benchmark_builtin_world<T>(
        dimension: &str,
        scene: &str,
        body_count: usize,
        build_world: impl Fn() -> PhysicsWorld<T>,
    ) where
        T: PhysicsBody + Clone + std::fmt::Debug + PartialEq,
    {
        const SAMPLES: usize = 5;
        const ITERATIONS: usize = 4;

        let initial_world = build_world();
        let bounds = (0..body_count)
            .map(|index| conservative_bounds(&initial_world.get_cloned(index).unwrap()))
            .collect::<Vec<_>>();
        let mut candidate_pairs = Vec::new();
        collect_naive_pairs(&bounds, &mut candidate_pairs);
        let candidate_count = candidate_pairs.len();
        let all_pairs = body_count * body_count.saturating_sub(1) / 2;

        let mut expected_world = build_world();
        let expected_resolved = expected_world.step_with_tree(0.0, 0.0, false).unwrap();
        let expected_bodies = (0..body_count)
            .map(|index| expected_world.get_cloned(index).unwrap())
            .collect::<Vec<_>>();
        let mut tree_world = build_world();
        assert_eq!(
            tree_world.step_with_tree(0.0, 0.0, true).unwrap(),
            expected_resolved
        );
        for (index, expected) in expected_bodies.iter().enumerate() {
            assert_eq!(tree_world.get_cloned(index).as_ref(), Some(expected));
        }

        for (strategy, forced_tree) in [
            ("naive", Some(false)),
            ("tree", Some(true)),
            ("automatic", None),
        ] {
            let mut samples = Vec::with_capacity(SAMPLES);
            let mut selected_tree = None;
            for _ in 0..SAMPLES {
                let mut world = build_world();
                let step_world = |world: &mut PhysicsWorld<T>| match forced_tree {
                    Some(use_tree) => world.step_with_tree(0.0, 0.0, use_tree),
                    None => world.step(0.0, 0.0),
                };
                assert_eq!(step_world(&mut world).unwrap(), expected_resolved);
                let selected = world.stepper.bounds_tree.root.is_some();
                if let Some(previous) = selected_tree {
                    assert_eq!(selected, previous);
                } else {
                    selected_tree = Some(selected);
                }
                let start = Instant::now();
                for _ in 0..ITERATIONS {
                    assert_eq!(step_world(&mut world).unwrap(), expected_resolved);
                }
                samples.push(start.elapsed() / ITERATIONS as u32);
                for (index, expected) in expected_bodies.iter().enumerate() {
                    assert_eq!(world.get_cloned(index).as_ref(), Some(expected));
                }
            }

            let median_us = median_micros(&mut samples);
            eprintln!(
                "builtin physics step dimension={dimension} scene={scene} strategy={strategy} selected_traversal={} bodies={body_count} candidate_pairs={candidate_count} all_pairs={all_pairs} resolved_pairs={expected_resolved} steady_step_median_us={median_us:.2}",
                if selected_tree.unwrap_or(false) {
                    "tree"
                } else {
                    "naive"
                }
            );
        }
    }

    fn benchmark_dynamic_world<T>(
        dimension: &str,
        body_count: usize,
        build_world: impl Fn() -> PhysicsWorld<T>,
    ) where
        T: PhysicsBody + std::fmt::Debug + PartialEq,
    {
        const SAMPLES: usize = 7;
        const ITERATIONS: usize = 4;
        let mut baseline = build_world();
        let expected_resolved = baseline.step_with_tree(0.01, 0.1, false).unwrap();
        assert!(
            expected_resolved > 0,
            "benchmark must resolve moving contacts"
        );
        let expected_bodies = (0..body_count)
            .map(|index| baseline.get_cloned(index).unwrap())
            .collect::<Vec<_>>();

        for (strategy, forced_tree) in [
            ("naive", Some(false)),
            ("tree", Some(true)),
            ("automatic", None),
        ] {
            let step_world = |world: &mut PhysicsWorld<T>| match forced_tree {
                Some(use_tree) => world.step_with_tree(0.01, 0.1, use_tree),
                None => world.step(0.01, 0.1),
            };
            let mut warmup = build_world();
            assert_eq!(step_world(&mut warmup).unwrap(), expected_resolved);
            for (index, expected) in expected_bodies.iter().enumerate() {
                assert_eq!(warmup.get_cloned(index).as_ref(), Some(expected));
            }

            let mut samples = Vec::with_capacity(SAMPLES);
            let mut selected_tree = None;
            for _ in 0..SAMPLES {
                let mut sample_time = Duration::ZERO;
                for _ in 0..ITERATIONS {
                    // Rebuild outside the timer so every step starts from the same state.
                    let mut world = build_world();
                    let start = Instant::now();
                    let resolved = step_world(&mut world).unwrap();
                    sample_time += start.elapsed();
                    assert_eq!(resolved, expected_resolved);
                    let selected = world.stepper.bounds_tree.root.is_some();
                    if let Some(previous) = selected_tree {
                        assert_eq!(selected, previous);
                    } else {
                        selected_tree = Some(selected);
                    }
                    for (index, expected) in expected_bodies.iter().enumerate() {
                        assert_eq!(world.get_cloned(index).as_ref(), Some(expected));
                    }
                    std::hint::black_box(resolved);
                }
                samples.push(sample_time / ITERATIONS as u32);
            }

            let median_us = median_micros(&mut samples);
            eprintln!(
                "dynamic physics step dimension={dimension} strategy={strategy} selected_traversal={} bodies={body_count} resolved_pairs={expected_resolved} reset=full-world median_us={median_us:.2}",
                if selected_tree.unwrap_or(false) {
                    "tree"
                } else {
                    "naive"
                }
            );
        }
    }

    #[test]
    #[ignore = "manual release-mode moving collision step benchmark with reset worlds"]
    fn moving_builtin_collision_step_benchmark() {
        for body_count in [512, 1024] {
            benchmark_dynamic_world("2d", body_count, || build_dynamic_2d_fixture(body_count));
            benchmark_dynamic_world("3d", body_count, || build_dynamic_3d_fixture(body_count));
        }
    }

    #[test]
    #[ignore = "manual release-mode real 2D/3D sparse, clustered, and dense physics benchmark"]
    fn builtin_physics_objects_sparse_clustered_dense_step_benchmark() {
        for (scene, body_count) in [
            ("sparse", 512),
            ("cluster8", 512),
            ("cluster16", 512),
            ("cluster32", 512),
            ("cluster64", 512),
            ("cluster128", 512),
            ("dense", 512),
            ("sparse", 1024),
            ("clustered", 1024),
            ("dense", 1024),
        ] {
            benchmark_builtin_world("2d", scene, body_count, || {
                build_2d_fixture(scene, body_count)
            });
            benchmark_builtin_world("3d", scene, body_count, || {
                build_3d_fixture(scene, body_count)
            });
        }
    }

    fn collect_naive_pairs(bounds: &[[(f64, f64); 3]], pairs: &mut Vec<(usize, usize)>) {
        pairs.clear();
        for first in 0..bounds.len() {
            for second in first + 1..bounds.len() {
                if bounds_overlap(bounds[first], bounds[second]) {
                    pairs.push((first, second));
                }
            }
        }
    }

    fn collect_tree_pairs(
        bounds: &[[(f64, f64); 3]],
        tree: &mut BoundsTree,
        candidates: &mut Vec<usize>,
        pairs: &mut Vec<(usize, usize)>,
    ) {
        tree.prepare_capacity(bounds.len()).unwrap();
        tree.rebuild(bounds);
        let mut visited = 0;
        collect_tree_candidates(bounds, tree, candidates, pairs, &mut visited);
    }

    fn collect_tree_candidates(
        bounds: &[[(f64, f64); 3]],
        tree: &BoundsTree,
        candidates: &mut Vec<usize>,
        pairs: &mut Vec<(usize, usize)>,
        visited: &mut usize,
    ) {
        pairs.clear();
        for (first, bounds) in bounds.iter().copied().enumerate() {
            candidates.clear();
            *visited += tree.query(bounds, first, candidates);
            candidates.sort_unstable();
            pairs.extend(candidates.iter().map(|second| (first, *second)));
        }
    }

    fn median_micros(samples: &mut [Duration]) -> f64 {
        samples.sort_unstable();
        samples[samples.len() / 2].as_secs_f64() * 1_000_000.0
    }
}
