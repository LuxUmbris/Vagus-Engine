pub mod hitbox {
    use crate::essential::collections::StableVec;
    use crate::math::{Vec2, Vec3};
    use std::io::Result;

    #[derive(Debug, Clone, PartialEq)]
    pub enum Hitbox2D {
        Rectangular(Vec2, Vec2, Vec2, Vec2),
        Triangular(Vec2, Vec2, Vec2),
        /// radius
        Circle(f32),
        Polygon(StableVec<Vec2>),
    }

    #[derive(Debug, Clone, PartialEq)]
    pub enum Hitbox3D {
        Quad(Vec3, Vec3, Vec3, Vec3, Vec3, Vec3, Vec3, Vec3),
        /// radius
        Sphere(f32),
        Object(StableVec<Vec3>),
    }

    impl Hitbox2D {
        pub fn new() -> Self {
            Self::Rectangular(
                Vec2 { x: 0.0, y: 0.0 },
                Vec2 { x: 0.0, y: 0.0 },
                Vec2 { x: 0.0, y: 0.0 },
                Vec2 { x: 0.0, y: 0.0 },
            )
        }

        pub fn rectangular(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> Self {
            Self::Rectangular(a, b, c, d)
        }

        pub fn triangular(a: Vec2, b: Vec2, c: Vec2) -> Self {
            Self::Triangular(a, b, c)
        }

        pub fn circle(radius: f32) -> Self {
            Self::Circle(radius)
        }

        pub fn polygon(edges: StableVec<Vec2>) -> Self {
            Self::Polygon(edges)
        }

        pub fn intersects(&self, rhs: &Hitbox2D) -> bool {
            match self {
                Self::Rectangular(edge1, edge2, edge3, edge4) => match rhs {
                    Self::Rectangular(edge1, edge2, edge3, edge4) => {}
                    Self::Triangular(edge1, edge2, edge3) => {}
                    Self::Circle(radius) => {}
                    Self::Polygon(edges) => {}
                },
                Self::Triangular(edge1, edge2, edge3) => match rhs {
                    Self::Rectangular(edge1, edge2, edge3, edge4) => {}
                    Self::Triangular(edge1, edge2, edge3) => {}
                    Self::Circle(radius) => {}
                    Self::Polygon(edges) => {}
                },
                Self::Circle(radius) => match rhs {
                    Self::Rectangular(edge1, edge2, edge3, edge4) => {}
                    Self::Triangular(edge1, edge2, edge3) => {}
                    Self::Circle(radius) => {}
                    Self::Polygon(edges) => {}
                },
                Self::Polygon(edges) => match rhs {
                    Self::Rectangular(edge1, edge2, edge3, edge4) => {}
                    Self::Triangular(edge1, edge2, edge3) => {}
                    Self::Circle(radius) => {}
                    Self::Polygon(edges) => {}
                },
            }
        }

        pub fn seperation(&mut self, rhs: &Hitbox2D) -> bool {
            match self {
                Self::Rectangular(edge1, edge2, edge3, edge4) => match rhs {
                    Self::Rectangular(edge1, edge2, edge3, edge4) => {}
                    Self::Triangular(edge1, edge2, edge3) => {}
                    Self::Circle(radius) => {}
                    Self::Polygon(edges) => {}
                },
                Self::Triangular(edge1, edge2, edge3) => match rhs {
                    Self::Rectangular(edge1, edge2, edge3, edge4) => {}
                    Self::Triangular(edge1, edge2, edge3) => {}
                    Self::Circle(radius) => {}
                    Self::Polygon(edges) => {}
                },
                Self::Circle(radius) => match rhs {
                    Self::Rectangular(edge1, edge2, edge3, edge4) => {}
                    Self::Triangular(edge1, edge2, edge3) => {}
                    Self::Circle(radius) => {}
                    Self::Polygon(edges) => {}
                },
                Self::Polygon(edges) => match rhs {
                    Self::Rectangular(edge1, edge2, edge3, edge4) => {}
                    Self::Triangular(edge1, edge2, edge3) => {}
                    Self::Circle(radius) => {}
                    Self::Polygon(edges) => {}
                },
            }
        }
    }

    impl Default for Hitbox2D {
        fn default() -> Self {
            Self::new()
        }
    }
}
