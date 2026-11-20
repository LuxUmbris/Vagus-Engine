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

use vagus::{
    math::Vec2,
    scene::{SceneManager, default::GameScene2D},
};

#[test]
fn growable_scene_manager_expands_ecs_storage_without_losing_selection() {
    let mut manager = SceneManager::<GameScene2D, Vec2>::with_growable_capacity(1).unwrap();
    manager
        .add_scene(GameScene2D::with_capacity(0).unwrap())
        .unwrap();
    assert_eq!(manager.current_index(), Some(0));

    let mut second_scene = GameScene2D::with_capacity(1).unwrap();
    second_scene.objects_mut().push(Default::default()).unwrap();
    assert_eq!(manager.add_scene(second_scene).unwrap(), 1);

    assert_eq!(manager.len(), 2);
    assert_eq!(manager.current_index(), Some(0));
    assert!(manager.select_scene(1));
    assert_eq!(manager.current_scene().unwrap().objects().len(), 1);
}
