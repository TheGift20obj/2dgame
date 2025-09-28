use bevy::prelude::*;
use rapier2d::prelude::*;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

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
pub struct AttackStatus(pub bool);

#[derive(Component)]
pub struct FinishStatus(pub bool);

#[derive(Component)]
pub struct PointText(pub u32);

#[derive(Component, Clone)]
pub struct AnimationIndices {
    pub first: usize,
    pub last: usize,
}

#[derive(Resource)]
pub struct AtlasHandles (
    pub HashMap<String, AnimationIndices>,
);

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
pub struct Fog;

#[derive(Component)]
pub struct FogHalo;

#[derive(Component)]
pub struct Wall;

#[derive(Component)]
pub struct Pending;

#[derive(Component)]
pub struct Monster;

#[derive(Component)]
pub struct MonsterSprite;

#[derive(Debug, Deserialize, Resource)]
pub struct ItemConfig {
    pub items: HashMap<String, Item>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Item {
    pub id: String,
    pub path: String,
    pub value: f32,
    pub item_type: String,
    pub amount: u32,
}

pub struct Inventory {
    pub items: HashMap<u32, Item>,
    pub capacity: u32,
}

impl Inventory {
    pub fn new() -> Self {
        Self {
            capacity: 16,
            items: HashMap::new(),
        }
    }

    pub fn init(&mut self, config: &Res<ItemConfig>) {
        if let Some(sword) = config.items.get("sword_basic") {
            // np. wrzucamy miecz do slota 0
            self.items.insert(0, sword.clone());
        }
        if let Some(apple) = config.items.get("apple_red") {
            // np. wrzucamy jabłko do slota 1
            self.items.insert(1, apple.clone());
        }
    }

    pub fn add_item(&mut self, slot: u32, item: Item) -> bool {
        if slot < self.capacity {
            self.items.insert(slot, item);
            true
        } else {
            false
        }
    }

    pub fn remove_item(&mut self, slot: u32) -> Option<Item> {
        self.items.remove(&slot)
    }

    pub fn remove_one(&mut self, slot: u32) -> Option<Item> {
        if let Some(item) = self.items.get_mut(&slot) {
            if item.amount > 1 {
                item.amount -= 1;
                // Zwracamy kopię itemu ze zmniejszoną ilością
                Some(Item {
                    id: item.id.clone(),
                    path: item.path.clone(),
                    value: item.value,
                    item_type: item.item_type.clone(),
                    amount: 1, // zwracamy tylko tę jedną sztukę
                })
            } else {
                // amount == 1, więc usuwamy całkowicie
                self.items.remove(&slot)
            }
        } else {
            None
        }
    }

    pub fn get_item(&self, slot: u32) -> Option<&Item> {
        self.items.get(&slot)
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
    pub time_heal: Timer,
    pub time_satamina: Timer,
}

impl PlayerData {
    pub fn new(config: &Res<ItemConfig>) -> Self {
        let mut inventory = Inventory::new();
        inventory.init(config);
        Self {
            health: 100.0,
            max_health: 100.0,
            inventory: inventory,
            can_heal: Timer::from_seconds(3.14, TimerMode::Once),
            satamina: 360.0,
            min_satamina: 25.0,
            max_satamina: 360.0,
            time_heal: Timer::from_seconds(0.375, TimerMode::Once),
            time_satamina: Timer::from_seconds(0.025, TimerMode::Once)
        }
    }

    pub fn heal(&mut self, value: f32, time: &Res<Time>) {
        if self.time_heal.finished() {
            self.health = (self.health + value).min(self.max_health);
            self.can_heal.reset();
        } else {
            self.time_heal.tick(time.delta());
        }
    }

    pub fn damage(&mut self, value: f32) {
        self.health = (self.health - value).clamp(0.0, self.max_health);
    }

    pub fn run(&mut self, value: f32, time: &Res<Time>) {
        if self.time_satamina.finished() {
            self.satamina = (self.satamina - value).clamp(0.0, self.max_satamina);
            self.time_satamina.reset();
        } else {
            self.time_satamina.tick(time.delta());
        }
    }

    pub fn rest(&mut self, value: f32, time: &Res<Time>) {
        if self.time_satamina.finished() {
            self.satamina = (self.satamina + value).clamp(0.0, self.max_satamina);
            self.time_satamina.reset();
        } else {
            self.time_satamina.tick(time.delta());
        }
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

#[derive(Event)]
pub struct ConsumeEvent {
    pub slot: u32,     // z którego slotu pochodzi
    pub item_id: String,
}

/// Event użycia przedmiotu funkcjonalnego (np. broń, narzędzie)
#[derive(Event)]
pub struct FunctionalEvent {
    pub slot: u32,
    pub item_id: String,
}

#[derive(Resource)]
pub struct InventoryState {
    pub selected: usize, // aktualnie wybrany slot
    pub slots: usize,    // liczba slotów
}

#[derive(Component)]
pub struct InventorySlot(pub usize);

#[derive(Component)]
pub struct InventoryImage(pub String);

pub const WORLD_SIZE: i32 = 96;   // liczba kafelków widocznych w danym "obszarze"
pub const TILE_SIZE: f32 = 64.0;

#[derive(Component)]
pub struct MonsterAI {
    pub target_player: bool,
    pub random_timer: Timer,
    pub random_dir: Vec2,
    pub action_timer: Timer,
    pub action_cooldown: Timer,
    pub health: f32,
    pub last_health: f32,
    pub stun_cooldown: Timer,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MenuButtonAction {
    NewGame,
    LoadGame,
    Options,
    Exit,
}

#[derive(Component, Clone, Copy)]
pub struct MenuButton(pub MenuButtonAction);

#[derive(Component)]
pub struct MenuRoot;

#[derive(Component)]
pub struct MenuCamera;