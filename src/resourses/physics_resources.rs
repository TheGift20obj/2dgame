use bevy::prelude::*;
use rapier2d::prelude::*;
use serde::{Deserialize, Serialize};
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

#[derive(Resource)]
pub struct ResumeStatus(pub bool);

#[derive(Component)]
pub struct AttackStatus(pub bool);

#[derive(Component)]
pub struct FinishStatus(pub bool);

#[derive(Component, Clone)]
pub struct AnimationIndices {
    pub first: usize,
    pub last: usize,
}

#[derive(Resource)]
pub struct AtlasHandles(pub HashMap<String, AnimationIndices>);

#[derive(Component, Deref, DerefMut)]
pub struct AnimationTimer(pub Timer);

#[derive(Component)]
pub struct Player;

#[derive(Component)]
pub struct PlayerUIs;

#[derive(Component)]
pub struct PlayerSprite;

/// Last movement direction of the player. World interactions use it to put
/// dropped items in front of the character without depending on the camera.
#[derive(Component)]
pub struct FacingDirection(pub Vec2);

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

/// Marks a `Monster` entity as specifically Monster 2 — see
/// `monster::MonsterKind` and `docs/monster2.md`. Monster 2 entities still
/// carry the generic `Monster` marker too (every system that treats
/// monsters generically — spawning, despawning, collision, death/XP —
/// keeps working unmodified), this is only for code that needs to single
/// Monster 2 out (`Query<..., With<Monster2>>`), such as a future chase/
/// attack system that should apply to Monster 2 without touching Monster 1.
#[derive(Component)]
pub struct Monster2;

/// Tracks Monster 2's attack-leap lifecycle — see `docs/monster2.md`. The
/// cooldown between leaps reuses `MonsterAI::action_cooldown` (seeded from
/// `monster::Monster2Config::attack_cooldown_secs` at spawn, same mechanism
/// Monster 1's attack cooldown already uses), so this only tracks what's
/// actually specific to being airborne.
///
/// `direction` is captured once at takeoff (`monster::monster_ai`'s
/// `MonsterState::Attack` handling for Monster 2) and never recalculated
/// for the rest of that leap — every frame while `airborne` is true,
/// `monster_ai` reads `direction` back out rather than re-deriving it from
/// the player's current position, which is the entire mechanism behind
/// "the jump direction is locked at takeoff and can't be steered mid-air".
#[derive(Component, Default)]
pub struct Monster2AttackJump {
    pub airborne: bool,
    pub direction: Vec2,
    /// Seconds elapsed since takeoff — compared against
    /// `Monster2Config::jump_duration_secs` to know when to land, and used
    /// to compute the sprite-only visual arc height (see
    /// `monster::MONSTER_SPRITE_BASE_Y`).
    pub elapsed: f32,
}

#[derive(Component)]
pub struct MonsterSprite;

/// Marks specifically the sprite child of a Monster 2 entity (as opposed to
/// Monster 1's, which only carries the generic `MonsterSprite`) — lets
/// `monster::animate_monster_sprite` pick the correct `AtlasHandles` key
/// ("walk2" vs "walk") and `Monster2AnimationLayouts` layout to reset to
/// when an attack/jump animation finishes, instead of always assuming
/// Monster 1's sheet.
#[derive(Component)]
pub struct Monster2Sprite;

#[derive(Debug, Deserialize, Resource)]
pub struct ItemConfig {
    pub items: HashMap<String, Item>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Item {
    pub id: String,
    pub path: String,
    pub value: [f32; 2],
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

    /// Adds an item to an existing stack when possible, otherwise uses the
    /// first free slot. Returns false only when every slot is occupied.
    pub fn try_add_item(&mut self, item: Item) -> bool {
        if item.id != "sword_basic"
            && let Some(existing) = self
                .items
                .values_mut()
                .find(|existing| existing.id == item.id)
        {
            existing.amount = existing.amount.saturating_add(item.amount);
            return true;
        }

        let Some(slot) = (0..self.capacity).find(|slot| !self.items.contains_key(slot)) else {
            return false;
        };
        self.items.insert(slot, item);
        true
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

/// Starting heal-cooldown duration for a new/respawned player. Named so the
/// (non-π) 3.14 literal only needs to exist in one place.
pub const DEFAULT_HEAL_COOLDOWN_SECONDS: f32 = 3.14;

impl PlayerData {
    pub fn new(config: &Res<ItemConfig>) -> Self {
        let mut inventory = Inventory::new();
        inventory.init(config);
        Self {
            health: 100.0,
            max_health: 100.0,
            inventory: inventory,
            can_heal: Timer::from_seconds(DEFAULT_HEAL_COOLDOWN_SECONDS, TimerMode::Once),
            satamina: 360.0,
            min_satamina: 25.0,
            max_satamina: 360.0,
            time_heal: Timer::from_seconds(0.375, TimerMode::Once),
            time_satamina: Timer::from_seconds(0.025, TimerMode::Once),
        }
    }

    /// Same starting stats as `new`, but with a caller-supplied inventory
    /// instead of the config-based starting items. Used when the player
    /// keeps their inventory across a respawn/leave instead of starting over.
    pub fn respawn_with_inventory(inventory: Inventory) -> Self {
        Self {
            health: 100.0,
            max_health: 100.0,
            inventory,
            can_heal: Timer::from_seconds(DEFAULT_HEAL_COOLDOWN_SECONDS, TimerMode::Once),
            satamina: 360.0,
            min_satamina: 25.0,
            max_satamina: 360.0,
            time_heal: Timer::from_seconds(0.375, TimerMode::Once),
            time_satamina: Timer::from_seconds(0.025, TimerMode::Once),
        }
    }

    pub fn heal(&mut self, value: f32, time: &Res<Time>) {
        if self.time_heal.just_finished() {
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
        if self.time_satamina.just_finished() {
            self.satamina = (self.satamina - value).clamp(0.0, self.max_satamina);
            self.time_satamina.reset();
        } else {
            self.time_satamina.tick(time.delta());
        }
    }

    pub fn rest(&mut self, value: f32, time: &Res<Time>) {
        if self.time_satamina.just_finished() {
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
            -normalized.clamp(0.0, 0.75) + 1.0 // upewniamy się, że nie wychodzi poza [0,1]
        }
    }
}

#[derive(Message)]
pub struct ConsumeEvent {
    pub slot: u32, // z którego slotu pochodzi
    pub item_id: String,
}

/// Event użycia przedmiotu funkcjonalnego (np. broń, narzędzie)
#[derive(Message)]
pub struct FunctionalEvent {
    pub slot: u32,
    pub item_id: String,
}

/// Gameplay-fact event: a monster's health reached 0 (see
/// `monster::monster_ai`'s kill branch). The quest system
/// (`systems::quests::progress`) is the only reader — it advances any
/// "kill monsters" task's progress AND grants `xp_reward` directly to the
/// player (see `progress::track_kills`), both from this one event. Written
/// only in reaction to a confirmed HP<=0 check inside gameplay logic, never
/// from UI/input code, so there's no path for the client to fake a kill and
/// claim credit; and read via the standard `MessageReader`/`Messages`
/// double-buffer, which delivers each event to a given reader exactly once,
/// so a kill can't be rewarded twice no matter how many times the UI
/// reopens or the player respawns/reconnects.
#[derive(Message)]
pub struct MonsterKilledEvent {
    /// Randomized per-kill XP (see `monster::MonsterCombatConfig::kill_xp_min`/
    /// `kill_xp_max`), rolled once at the moment the kill is confirmed —
    /// independent of, and in addition to, any "kill monsters" task's own
    /// completion reward.
    pub xp_reward: u32,
}

/// Gameplay-fact event: a melee hit actually landed on a monster (see
/// `eventer::functional_eventer`). Carries the raw damage dealt so the quest
/// system can drive both a "deal X damage" and a "hit monsters X times"
/// objective off the same event instead of needing two separate ones.
#[derive(Message)]
pub struct MonsterHitEvent {
    pub damage: f32,
}

/// Gameplay-fact event: the player successfully consumed an item (see
/// `eventer::food_eventer`). `item_type` is copied straight from
/// `ItemConfig` at the point of consumption, so the quest system can match
/// "consume food" objectives without a second config lookup.
#[derive(Message)]
pub struct ItemConsumedEvent {
    pub item_id: String,
    pub item_type: String,
}

/// Gameplay-fact event: the player picked up a world item into their
/// inventory (see `items::pickup_nearest_item`).
#[derive(Message)]
pub struct ItemCollectedEvent {
    pub item_id: String,
    pub amount: u32,
}

/// UI intent: the user picked a save slot to play. The game lifecycle owns
/// deciding what "starting" means (fresh player, or load from that slot).
/// `difficulty` is only consulted for a fresh (no existing save) slot — an
/// existing slot always keeps its own saved difficulty.
#[derive(Message)]
pub struct PlayRequested {
    pub slot: u8,
    pub difficulty: Option<crate::systems::monster_ai::difficulty::Difficulty>,
}

/// Player-domain report: the player has died. The game lifecycle owns
/// deciding what happens next (cleanup, despawn, death screen).
#[derive(Message)]
pub struct PlayerDied(pub Entity);

/// UI intent: the user asked to leave the current run (from Pause or the
/// death screen) and return to the main menu.
#[derive(Message)]
pub struct LeaveRequested;

/// UI intent: the user asked to respawn after dying, staying in this run.
#[derive(Message)]
pub struct RespawnRequested;

#[derive(Component)]
pub struct YSort {
    pub z: f32,
}

#[derive(Resource)]
pub struct InventoryState {
    pub selected: usize, // aktualnie wybrany slot
    pub slots: usize,    // liczba slotów
    pub open: bool,
}

#[derive(Component)]
pub struct InventorySlot(pub usize);

#[derive(Component)]
pub struct InventoryImage(pub String);

#[derive(Component)]
pub struct InventoryHotbar;
#[derive(Component)]
pub struct InventoryOverlay;
#[derive(Component)]
pub struct GameplayHud;

pub const WORLD_SIZE: i32 = 96; // liczba kafelków widocznych w danym "obszarze"
pub const TILE_SIZE: f32 = 64.0;

/// Range of the player's own `PointLight2d` (see `player::init`). Shared out
/// so monster AI can tell whether a monster is within the player's light
/// without duplicating the number — see `monster::MonsterCombatConfig`'s
/// darkness speed boost.
pub const PLAYER_LIGHT_RANGE: f32 = 750.0;

#[derive(Component)]
pub struct MonsterAI {
    pub random_timer: Timer,
    pub random_dir: Vec2,
    pub action_cooldown: Timer,
    pub health: f32,
    pub last_health: f32,
    pub stun_cooldown: Timer,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MenuButtonAction {
    /// Main menu -> open the slot-select screen.
    Play,
    Options,
    Exit,
    /// Slot-select: continue an existing save.
    PlaySlot(u8),
    /// Slot-select: start a fresh save in an empty slot -> difficulty select.
    PickDifficulty(u8),
    /// Difficulty-select: start a fresh save with the chosen difficulty.
    StartWithDifficulty(u8, crate::systems::monster_ai::difficulty::Difficulty),
    ResetSlot(u8),
    DeleteSlot(u8),
    /// Slot-select -> back to the main menu.
    BackToMenu,
    /// Difficulty-select -> back to the slot-select screen.
    BackToSlotSelect,
    /// Pause menu.
    Resume,
    Save,
    /// Pause menu or death screen: leave the current run, return to the main menu.
    Leave,
    /// Death screen: revive and keep playing this run.
    Respawn,
}

#[derive(Component, Clone, Copy)]
pub struct MenuButton(pub MenuButtonAction);

#[derive(Component)]
pub struct MenuRoot;

#[derive(Component)]
pub struct MenuCamera;

pub const CAMERA_LAYER_SPRITE: usize = 1; // warstwa dla sprite'ów (podłoga, widoczne ściany)
pub const CAMERA_LAYER_LIGHT: usize = 2; // warstwa dla światła i occluderów

pub const CAMERA_LAYER_FLOOR: &[usize] = &[0];
pub const CAMERA_LAYER_ENTITY: &[usize] = &[0];
pub const CAMERA_LAYER_EFFECT: &[usize] = &[2];
pub const CAMERA_LAYER_WALL: &[usize] = &[0];
pub const CAMERA_LAYER_MONSTER: &[usize] = &[0];

#[derive(Component)]
pub struct PlayerCamera;

#[derive(Component)]
pub struct AICamera;

#[derive(Component)]
pub struct HealthBar;

#[derive(Component)]
pub struct SataminaBar;

#[derive(Component)]
pub struct DebugAI;

#[derive(Component)]
pub struct WorldItem {
    pub item: Item,
}

#[derive(Resource, Default)]
pub struct WorldItemsState {
    pub spawned_for_session: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, amount: u32) -> Item {
        Item {
            id: id.to_string(),
            path: "textures/empty.png".to_string(),
            value: [0.0, 0.0],
            item_type: "test".to_string(),
            amount,
        }
    }

    #[test]
    fn picked_up_items_stack_before_using_another_slot() {
        let mut inventory = Inventory::new();
        inventory.try_add_item(item("apple", 2));
        inventory.try_add_item(item("apple", 3));

        assert_eq!(inventory.items.len(), 1);
        assert_eq!(inventory.get_item(0).unwrap().amount, 5);
    }

    #[test]
    fn pickup_fails_when_inventory_has_no_matching_stack_or_empty_slot() {
        let mut inventory = Inventory {
            items: HashMap::new(),
            capacity: 1,
        };
        inventory.try_add_item(item("apple", 1));

        assert!(!inventory.try_add_item(item("sword", 1)));
    }
}
