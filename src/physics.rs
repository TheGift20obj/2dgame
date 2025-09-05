use bevy::prelude::*;
use rapier2d::prelude::*;
use crate::physics_resources::*;

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ResPhysicsWork(false));
        app.insert_resource(ResGravity(vector![0.0, 0.0]));
        app.insert_resource(ResRigidBodySet(RigidBodySet::new()));
        app.insert_resource(ResColliderSet(ColliderSet::new()));
        app.insert_resource(ResIntegrationParameters(IntegrationParameters::default()));
        app.insert_resource(ResPhysicsPipeline(PhysicsPipeline::new()));
        app.insert_resource(ResIslandManager(IslandManager::new()));
        app.insert_resource(ResDefaultBroadPhase(DefaultBroadPhase::new()));
        app.insert_resource(ResNarrowPhase(NarrowPhase::new()));
        app.insert_resource(ResImpulseJointSet(ImpulseJointSet::new()));
        app.insert_resource(ResMultibodyJointSet(MultibodyJointSet::new()));
        app.insert_resource(ResCCDSolver(CCDSolver::new()));
        app.insert_resource(ResQueryPipeline(QueryPipeline::new()));
        app.add_systems(Startup, init_physics);
        app.add_systems(FixedUpdate, step_physics);
        app.add_systems(FixedPostUpdate, sync_physics_to_transform);
    }
}

fn init_physics(
    mut physics_work: ResMut<ResPhysicsWork>,
) {
    physics_work.0 = true;
}

fn step_physics(
    mut pipeline: ResMut<ResPhysicsPipeline>,
    mut query_pipeline: ResMut<ResQueryPipeline>,
    mut rigid_bodies: ResMut<ResRigidBodySet>,
    mut colliders: ResMut<ResColliderSet>,
    mut impulse_joints: ResMut<ResImpulseJointSet>,
    mut multibody_joints: ResMut<ResMultibodyJointSet>,
    integration_parameters: ResMut<ResIntegrationParameters>,
    mut island_manager: ResMut<ResIslandManager>,
    mut broad_phase: ResMut<ResDefaultBroadPhase>,
    mut narrow_phase: ResMut<ResNarrowPhase>,
    mut ccd_solver: ResMut<ResCCDSolver>,
    gravity: Res<ResGravity>,
) {
    pipeline.0.step(
        &gravity.0,
        &integration_parameters.0,
        &mut island_manager.0,
        &mut broad_phase.0,
        &mut narrow_phase.0,
        &mut rigid_bodies.0,
        &mut colliders.0,
        &mut impulse_joints.0,
        &mut multibody_joints.0,
        &mut ccd_solver.0,
        Some(&mut query_pipeline.0),
        &(),
        &(),
    );
}

fn sync_physics_to_transform(
    rigid_bodies: Res<ResRigidBodySet>,
    mut query_single: Query<(&RigidBodyHandleComponent, &mut Transform)>,
) {
    for (rb_handle, mut transform) in &mut query_single {
        if let Some(rb) = rigid_bodies.0.get(rb_handle.0) {
            let pos = rb.position();
            let translation = pos.translation;
            let rotation = pos.rotation.angle();;

            transform.translation = Vec3::new(translation.x, translation.y, transform.translation.z);
            transform.rotation = Quat::from_rotation_z(rotation);
        }
    }
}