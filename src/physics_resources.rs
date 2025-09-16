use bevy::prelude::*;
use rapier2d::prelude::*;

#[derive(Resource)]
pub struct ResRigidBodySet(pub RigidBodySet);

#[derive(Resource)]
pub struct ResColliderSet(pub ColliderSet);

#[derive(Resource)]
pub struct ResGravity(pub Vector<f32>);

#[derive(Resource)]
pub struct ResIntegrationParameters(pub IntegrationParameters);

#[derive(Resource)]
pub struct ResPhysicsPipeline(pub PhysicsPipeline);

#[derive(Resource)]
pub struct ResIslandManager(pub IslandManager);

#[derive(Resource)]
pub struct ResDefaultBroadPhase(pub DefaultBroadPhase);

#[derive(Resource)]
pub struct ResNarrowPhase(pub NarrowPhase);

#[derive(Resource)]
pub struct ResImpulseJointSet(pub ImpulseJointSet);

#[derive(Resource)]
pub struct ResMultibodyJointSet(pub MultibodyJointSet);

#[derive(Resource)]
pub struct ResCCDSolver(pub CCDSolver);

#[derive(Resource)]
pub struct ResQueryPipeline(pub QueryPipeline);

#[derive(Component)]
pub struct RigidBodyHandleComponent(pub RigidBodyHandle);

#[derive(Resource)]
pub struct ResPhysicsWork(pub bool);

#[derive(Component)]
pub struct AnimationIndices {
    pub first: usize,
    pub last: usize,
}

#[derive(Component, Deref, DerefMut)]
pub struct AnimationTimer(pub Timer);

#[derive(Component)]
pub struct Player;

#[derive(Component)]
pub struct PlayerSprite;

#[derive(Component)]
pub struct WaterSprite;

#[derive(Component)]
pub struct Floor;

#[derive(Component)]
pub struct Wall;

#[derive(Component)]
pub struct Pending;

#[derive(Component)]
pub struct Monster;

#[derive(Component)]
pub struct MonsterSprite;

pub struct Item {
    pub value: String,
}

pub struct Inventory {
    pub items: Vec<Item>,
    pub capacity: u32,
}

impl Inventory {
    pub fn new() -> Self {
        Self {
            capacity: 16,
            items: Vec::new(),
        }
    }
}

#[derive(Component)]
pub struct PlayerData {
    pub health: f32,
    pub inventory: Inventory,
    pub can_heal: Timer,
}

impl PlayerData {
    pub fn new() -> Self {
        Self {
            health: 100.0,
            inventory: Inventory::new(),
            can_heal: Timer::from_seconds(3.14, TimerMode::Once)
        }
    }

    pub fn heal(&mut self, value: f32) {
        self.health = (self.health + value).clamp(0.0, 100.0);
    }

    pub fn damage(&mut self, value: f32) {
        self.health = (self.health - value).clamp(0.0, 100.0);
    }
}