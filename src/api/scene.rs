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
    essential::collections::StableVec,
    material::Material,
    math::MathVec,
    physics::{PhysicsObject2D, PhysicsObject3D, hitbox::Hitbox},
    shader,
    texture::Texture,
};

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
    fn shaders(&self) -> StableVec<shader::Shader>;
    fn game_objects(&self) -> impl GameObject<V>;
    fn is_rtx_on(&self) -> bool;
}

struct SceneManager<S, V>
where
    V: MathVec,
    S: Scene<V>,
{
    scenes: StableVec<S>,
    current: Option<usize>,
    _vector: std::marker::PhantomData<fn() -> V>,
}

impl<S, V> SceneManager<S, V>
where
    V: MathVec,
    S: Scene<V>,
{
    pub fn new() -> Self {
        Self {
            scenes: StableVec::new(),
            current: None,
            _vector: std::marker::PhantomData,
        }
    }
}
