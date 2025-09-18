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

#[derive(Component)]
pub struct ColliderComponent(pub ColliderHandle);

#[derive(Resource)]
pub struct ResPhysicsWork(pub bool);

#[derive(Resource)]
pub struct GameStatus(pub bool);

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
pub struct PlayerUIs;

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
    pub max_health: f32,
    pub inventory: Inventory,
    pub can_heal: Timer,
    pub satamina: f32,
    pub min_satamina: f32,
    pub max_satamina: f32,
}

impl PlayerData {
    pub fn new() -> Self {
        Self {
            health: 100.0,
            max_health: 100.0,
            inventory: Inventory::new(),
            can_heal: Timer::from_seconds(3.14, TimerMode::Once),
            satamina: 360.0,
            min_satamina: 25.0,
            max_satamina: 360.0
        }
    }

    pub fn heal(&mut self, value: f32) {
        self.health = (self.health + value).clamp(0.0, self.max_health);
    }

    pub fn damage(&mut self, value: f32) {
        self.health = (self.health - value).clamp(0.0, self.max_health);
    }

    pub fn run(&mut self, value: f32) {
        self.satamina = (self.satamina - value).clamp(0.0, self.max_satamina);
    }

    pub fn rest(&mut self, value: f32) {
        self.satamina = (self.satamina + value).clamp(0.0, self.max_satamina);
    }

    pub fn fatigue(&mut self) -> f32 {
        if self.satamina >= self.min_satamina {
            1.0
        } else {
            // normalizujemy od 0 do min_satamina
            let normalized = (self.min_satamina - self.satamina) / self.min_satamina;
            -normalized.clamp(0.0, 0.75)+1.0 // upewniamy się, że nie wychodzi poza [0,1]
        }
    }
}