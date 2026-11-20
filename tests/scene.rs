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
    essential::collections::StableVec,
    math::{Vec2, Vec3},
    physics::hitbox::{Hitbox, Hitbox2D, Hitbox3D},
    physics::{PhysicsObject2D, PhysicsObject3D},
    scene::{
        GameObject, Scene, SceneManager,
        default::{GameObject2D, GameObject3D, GameScene2D, GameScene3D, Transform3D},
    },
    shader::Shader,
};

struct TestScene(usize, StableVec<Shader>);

impl Scene<Vec2> for TestScene {
    fn shaders(&self) -> &StableVec<Shader> {
        &self.1
    }

    fn is_rtx_on(&self) -> bool {
        false
    }
}

#[test]
fn default_game_scene_exposes_its_ecs_object_storage() {
    let mut scene = GameScene2D::with_capacity(1).unwrap();
    assert_eq!(scene.objects().capacity(), 1);
    scene.objects_mut().push(GameObject2D::default()).unwrap();
    assert_eq!(scene.objects().len(), 1);
}

#[test]
fn active_scene_physics_component_can_be_replaced_in_its_ecs_row() {
    let mut manager = SceneManager::<GameScene2D, Vec2>::with_capacity(1).unwrap();
    manager
        .add_scene(GameScene2D::with_capacity(1).unwrap())
        .unwrap();
    manager
        .current_scene_mut()
        .unwrap()
        .objects_mut()
        .push(GameObject2D::default())
        .unwrap();

    let mut object = manager
        .current_scene()
        .unwrap()
        .objects()
        .get_cloned(0)
        .unwrap();
    object.physics_object_mut().mass = 2.0;
    let replacement = manager
        .current_scene_mut()
        .unwrap()
        .objects_mut()
        .try_replace(0, object);
    let old_object = match replacement {
        Ok(old_object) => old_object,
        Err(_) => panic!("existing scene object could not be replaced"),
    };

    assert_eq!(
        old_object.physics_object().mass,
        PhysicsObject2D::default().mass
    );
    assert_eq!(
        manager
            .current_scene()
            .unwrap()
            .objects()
            .get_cloned(0)
            .unwrap()
            .physics_object()
            .mass,
        2.0
    );
}

#[test]
fn game_scene_2d_integrates_physics_objects_in_ecs() {
    let mut scene = GameScene2D::with_capacity(1).unwrap();
    let mut object = GameObject2D::default();
    object.physics_object_mut().hitbox = Hitbox2D::circle(0.25, Vec2 { x: 0.0, y: 0.0 });
    object.physics_object_mut().velocity = Vec2 { x: 2.0, y: -1.0 };
    scene.objects_mut().push(object).unwrap();

    assert_eq!(scene.step_physics(0.5, 0.0).unwrap(), 0);

    let moved = scene.objects().get_cloned(0).unwrap();
    assert!(
        moved
            .physics_object()
            .hitbox
            .intersects(&Hitbox2D::circle(0.01, Vec2 { x: 1.0, y: -0.5 }))
    );
}

#[test]
fn game_scene_2d_steps_ecs_collisions_in_place() {
    let mut scene = GameScene2D::with_capacity(2).unwrap();
    for (x, velocity_x) in [(-1.1, 1.0), (1.1, -1.0)] {
        let mut object = GameObject2D::default();
        *object.physics_object_mut() = PhysicsObject2D::new(
            Hitbox2D::circle(1.0, Vec2 { x, y: 0.0 }),
            1.0,
            1.0,
            0.0,
            Vec2 {
                x: velocity_x,
                y: 0.0,
            },
        );
        scene.objects_mut().push(object).unwrap();
    }

    assert_eq!(scene.step_physics(0.2, 0.0).unwrap(), 1);

    let left = scene.objects().get_cloned(0).unwrap();
    let right = scene.objects().get_cloned(1).unwrap();
    assert_eq!(left.physics_object().velocity, Vec2 { x: -1.0, y: 0.0 });
    assert_eq!(right.physics_object().velocity, Vec2 { x: 1.0, y: 0.0 });
    assert_eq!(
        left.physics_object()
            .hitbox
            .separation(&right.physics_object().hitbox),
        Vec2 { x: 0.0, y: 0.0 }
    );
}

#[test]
fn game_scene_3d_integrates_physics_objects_in_ecs() {
    let mut scene = GameScene3D::with_capacity(1).unwrap();
    let mut object = GameObject3D::default();
    object.physics_object_mut().hitbox = Hitbox3D::sphere(
        0.25,
        Vec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
    );
    object.physics_object_mut().velocity = Vec3 {
        x: 1.0,
        y: 2.0,
        z: 3.0,
    };
    scene.objects_mut().push(object).unwrap();

    assert_eq!(scene.step_physics(0.5, 0.0).unwrap(), 0);

    let moved = scene.objects().get_cloned(0).unwrap();
    assert!(moved.physics_object().hitbox.intersects(&Hitbox3D::sphere(
        0.01,
        Vec3 {
            x: 0.5,
            y: 1.0,
            z: 1.5,
        }
    )));
}

#[test]
fn game_scene_3d_physics_motion_updates_render_transform() {
    let mut scene = GameScene3D::with_capacity(1).unwrap();
    let mut object = GameObject3D::default();
    *object.physics_object_mut() = PhysicsObject3D::new(
        Hitbox3D::sphere(
            0.25,
            Vec3 {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            },
        ),
        1.0,
        1.0,
        0.0,
        Vec3 {
            x: 2.0,
            y: -1.0,
            z: 0.5,
        },
    );
    object.set_transform(
        Transform3D::new(
            Vec3 {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            },
            Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            Vec3 {
                x: 2.0,
                y: 3.0,
                z: 4.0,
            },
        )
        .unwrap(),
    );
    scene.objects_mut().push(object).unwrap();

    scene.step_physics(0.5, 0.0).unwrap();

    let moved = scene.objects().get_cloned(0).unwrap();
    assert_eq!(
        moved.transform().transform_point([0.0, 0.0, 0.0]).unwrap(),
        [2.0, 1.5, 3.25]
    );
    assert_eq!(
        moved.transform().transform_point([1.0, 0.0, 0.0]).unwrap(),
        [4.0, 1.5, 3.25]
    );
}

#[test]
fn invalid_game_scene_3d_steps_preserve_render_transform_and_physics_state() {
    let mut scene = GameScene3D::with_capacity(1).unwrap();
    let mut object = GameObject3D::default();
    *object.physics_object_mut() = PhysicsObject3D::new(
        Hitbox3D::sphere(
            0.25,
            Vec3 {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            },
        ),
        1.0,
        1.0,
        0.0,
        Vec3 {
            x: 2.0,
            y: 0.0,
            z: 0.0,
        },
    );
    object.set_transform(
        Transform3D::new(
            Vec3 {
                x: 7.0,
                y: 8.0,
                z: 9.0,
            },
            Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            Vec3 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
            },
        )
        .unwrap(),
    );
    scene.objects_mut().push(object).unwrap();

    for delta_time in [-0.1, f32::NAN] {
        assert_eq!(scene.step_physics(delta_time, 0.0).unwrap(), 0);

        let object = scene.objects().get_cloned(0).unwrap();
        assert_eq!(
            object.transform().transform_point([0.0, 0.0, 0.0]).unwrap(),
            [7.0, 8.0, 9.0]
        );
        assert_eq!(
            object.physics_object().position(),
            Vec3 {
                x: 1.0,
                y: 2.0,
                z: 3.0
            }
        );
        assert_eq!(
            object.physics_object().velocity,
            Vec3 {
                x: 2.0,
                y: 0.0,
                z: 0.0
            }
        );
    }
}

#[test]
fn game_scene_3d_steps_ecs_collisions_in_place() {
    let mut scene = GameScene3D::with_capacity(2).unwrap();
    for (x, velocity_x) in [(-1.1, 1.0), (1.1, -1.0)] {
        let mut object = GameObject3D::default();
        *object.physics_object_mut() = PhysicsObject3D::new(
            Hitbox3D::sphere(1.0, Vec3 { x, y: 0.0, z: 0.0 }),
            1.0,
            1.0,
            0.0,
            Vec3 {
                x: velocity_x,
                y: 0.0,
                z: 0.0,
            },
        );
        scene.objects_mut().push(object).unwrap();
    }

    assert_eq!(scene.step_physics(0.2, 0.0).unwrap(), 1);

    let left = scene.objects().get_cloned(0).unwrap();
    let right = scene.objects().get_cloned(1).unwrap();
    assert_eq!(
        left.physics_object().velocity,
        Vec3 {
            x: -1.0,
            y: 0.0,
            z: 0.0
        }
    );
    assert_eq!(
        right.physics_object().velocity,
        Vec3 {
            x: 1.0,
            y: 0.0,
            z: 0.0
        }
    );
    assert_eq!(
        left.physics_object()
            .hitbox
            .separation(&right.physics_object().hitbox),
        Vec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    );
}

#[test]
fn default_2d_game_object_exposes_texture_physics_and_hitbox() {
    let object = GameObject2D::default();
    assert_eq!(
        (object.texture().width(), object.texture().height()),
        (1, 1)
    );
    assert_eq!(
        object.physics_object().mass,
        PhysicsObject2D::default().mass
    );
    let hitbox = GameObject::<Vec2>::hitbox(&object);
    assert!(hitbox.intersects(hitbox));
}

#[test]
fn default_3d_game_object_exposes_material_physics_and_hitbox() {
    let object = GameObject3D::default();
    assert_eq!(
        (
            object.material().texture().width(),
            object.material().texture().height()
        ),
        (1, 1)
    );
    assert_eq!(
        object.physics_object().mass,
        PhysicsObject3D::default().mass
    );
    let hitbox = GameObject::<Vec3>::hitbox(&object);
    assert!(hitbox.intersects(hitbox));
}

#[test]
fn transform3d_scales_rotates_translates_and_rejects_unrepresentable_points() {
    let transform = Transform3D::new(
        Vec3 {
            x: 3.0,
            y: 4.0,
            z: 5.0,
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
    let point = transform.transform_point([1.0, 0.0, 0.0]).unwrap();
    assert!((point[0] - 3.0).abs() < 1.0e-6);
    assert!((point[1] - 6.0).abs() < 1.0e-6);
    assert!((point[2] - 5.0).abs() < 1.0e-6);
    assert!(transform.transform_point([f32::MAX, 0.0, 0.0]).is_err());
    assert!(
        Transform3D::new(
            Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            Vec3 {
                x: 0.0,
                y: 1.0,
                z: 1.0,
            },
        )
        .is_err()
    );
}

#[test]
fn default_3d_game_scene_uses_ecs_object_storage() {
    let mut scene = GameScene3D::with_capacity(1).unwrap();
    assert_eq!(scene.objects().capacity(), 1);
    scene.objects_mut().push(GameObject3D::default()).unwrap();
    assert_eq!(scene.objects().len(), 1);
}

#[test]
fn default_2d_and_3d_scenes_start_empty_with_rtx_disabled() {
    let scene_2d = GameScene2D::default();
    let scene_3d = GameScene3D::default();

    assert!(scene_2d.objects().is_empty());
    assert!(scene_3d.objects().is_empty());
    assert!(!Scene::<Vec2>::is_rtx_on(&scene_2d));
    assert!(!Scene::<Vec3>::is_rtx_on(&scene_3d));
}

#[test]
fn scene_manager_accepts_2d_and_3d_game_scene_types() {
    let mut manager = SceneManager::<GameScene2D, Vec2>::with_capacity(1).unwrap();
    manager
        .add_scene(GameScene2D::with_capacity(1).unwrap())
        .unwrap();
    assert!(manager.current_scene().unwrap().objects().is_empty());

    let mut manager = SceneManager::<GameScene3D, Vec3>::with_capacity(1).unwrap();
    manager
        .add_scene(GameScene3D::with_capacity(1).unwrap())
        .unwrap();
    assert!(manager.current_scene().unwrap().objects().is_empty());
}

#[test]
fn active_scene_can_be_mutated_through_manager() {
    let mut manager = SceneManager::<GameScene2D, Vec2>::with_capacity(1).unwrap();
    manager
        .add_scene(GameScene2D::with_capacity(1).unwrap())
        .unwrap();

    manager
        .current_scene_mut()
        .unwrap()
        .objects_mut()
        .push(GameObject2D::default())
        .unwrap();

    assert_eq!(manager.current_scene().unwrap().objects().len(), 1);
}

#[test]
fn manager_stores_scenes_in_ecs_and_tracks_selection() {
    let mut manager = SceneManager::<TestScene, Vec2>::with_capacity(2).unwrap();
    assert!(manager.is_empty());
    assert!(manager.current_scene().is_none());
    assert!(manager.current_scene_mut().is_none());

    assert_eq!(
        manager.add_scene(TestScene(10, StableVec::new())).unwrap(),
        0
    );
    assert_eq!(
        manager.add_scene(TestScene(20, StableVec::new())).unwrap(),
        1
    );
    assert_eq!(manager.current_scene().map(|scene| scene.0), Some(10));

    assert!(manager.select_scene(1));
    assert_eq!(manager.current_index(), Some(1));
    assert_eq!(manager.current_scene().map(|scene| scene.0), Some(20));
    assert!(!manager.select_scene(2));
    assert_eq!(manager.current_scene().map(|scene| scene.0), Some(20));
}

#[test]
fn manager_preserves_selected_scene_when_multiple_preceding_rows_are_removed() {
    let mut manager = SceneManager::<TestScene, Vec2>::with_capacity(4).unwrap();
    for value in [10, 20, 30, 40] {
        manager
            .add_scene(TestScene(value, StableVec::new()))
            .unwrap();
    }
    assert!(manager.select_scene(3));

    assert_eq!(manager.remove_scene(1).map(|scene| scene.0), Some(20));
    assert_eq!(manager.current_index(), Some(2));
    assert_eq!(manager.current_scene().map(|scene| scene.0), Some(40));

    assert_eq!(manager.remove_scene(0).map(|scene| scene.0), Some(10));
    assert_eq!(manager.current_index(), Some(1));
    assert_eq!(manager.current_scene().map(|scene| scene.0), Some(40));
}

#[test]
fn manager_capacity_failure_and_removal_keep_selection_consistent() {
    let mut manager = SceneManager::<TestScene, Vec2>::with_capacity(2).unwrap();
    manager.add_scene(TestScene(10, StableVec::new())).unwrap();
    manager.add_scene(TestScene(20, StableVec::new())).unwrap();
    assert!(manager.add_scene(TestScene(30, StableVec::new())).is_err());
    assert_eq!(manager.len(), 2);
    assert!(manager.select_scene(1));

    assert_eq!(manager.remove_scene(0).map(|scene| scene.0), Some(10));
    assert_eq!(manager.current_index(), Some(0));
    assert_eq!(manager.current_scene().map(|scene| scene.0), Some(20));
    assert_eq!(manager.remove_scene(0).map(|scene| scene.0), Some(20));
    assert!(manager.current_scene().is_none());
    assert_eq!(manager.current_index(), None);
}
