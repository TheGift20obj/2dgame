# Monster 2 — developer guide

Monster 2 is currently a **basic wandering test creature**: it spawns, walks
around randomly, avoids walls/water, can be killed, and grants XP. It has
**no player detection, no chase, no attack, and no special abilities** —
none of that is implemented, on purpose. This document says exactly where
each of those future behaviors should be added, and which existing systems
(shared with Monster 1) to reuse.

Everything below names the actual file/function/component in this project —
not a generic description.

---

## 1. Monster 2 configuration

- **Identity/type**: `MonsterKind` enum in
  [`src/systems/monster.rs`](../src/systems/monster.rs) — variants
  `Monster1` and `Monster2`. This is the single source of truth for "which
  monster is this"; a monster's `MonsterKind` is passed into
  `spawn_monster_at` and determines its texture/atlas/animation key. To add
  a Monster 3, add a variant here and one arm in each of `MonsterKind`'s
  methods (currently just `walk_animation_key`).
- **Marker component**: `Monster2` in
  [`src/resourses/physics_resources.rs`](../src/resourses/physics_resources.rs),
  next to the existing generic `Monster` marker. Every Monster 2 entity has
  *both* `Monster` (so every generic monster system — spawning, despawning,
  collision, death — keeps working unmodified) and `Monster2` (so code that
  needs to single Monster 2 out can `Query<..., With<Monster2>>` without
  touching Monster 1 at all).
- **Stats (HP, XP reward, spawn mix)**: `Monster2Config` resource in
  `src/systems/monster.rs`. Fields:
  - `test_hp: f32` — HP a freshly spawned Monster 2 starts with.
  - `test_xp_min` / `test_xp_max: u32` — random per-kill XP range (same
    mechanism as Monster 1's `MonsterCombatConfig::kill_xp_min`/
    `kill_xp_max`, just a separate, independently-tunable range).
  - `spawn_chance: f32` — chance (0.0–1.0) that a given spawn slot becomes a
    Monster 2 instead of a Monster 1.

  Default values are set once, in one place, in `MonsterPlugin::build`
  (`src/systems/monster.rs`):
  ```rust
  .insert_resource(Monster2Config {
      test_hp: 60.0,
      test_xp_min: 15,
      test_xp_max: 30,
      spawn_chance: 0.25,
  })
  ```
  **To change Monster 2's test HP/XP, edit these 4 numbers.** Nothing else
  needs to change.
- **Registration**: `MonsterKind` doesn't need a separate "registry" —
  `spawn_monsters_system` (below) is the only place a kind is chosen, and
  `spawn_monster_at` is the only place a kind is turned into an entity.

---

## 2. Sprite and animation

- **`monster2_combined.png`** lives at `assets/textures/monster2_combined.png`
  and is referenced in exactly one place: `load_monster2_texture` in
  `src/systems/monster.rs`, which loads the image and builds its
  `TextureAtlasLayout`. It's the Monster 2 counterpart of
  `load_monster_texture` (Monster 1's equivalent, same file).
- **How it was generated**: same vertical-stack pipeline as Monster 1's
  `monster_combined.png` (`image` crate, `ImageBuffer`/`copy_from`,
  left-aligned at x=0, no gaps) — see the commented-out block in
  [`src/main.rs`](../src/main.rs) right after the `player_combined.png`
  block — plus two corrections the raw source images needed that Monster
  1's didn't (both done once, baked into the PNG — nothing about this
  happens at runtime):
  1. **Width mismatch**: `monster2_attack.png` (512x288, 128x144 frames) was
     wider than `monster2.png`/`monster2_jump.png` (320x160, 80x80 frames).
     Monster 1's sections all share one width (128); Monster 2's need to as
     well. Fixed by scaling the *whole* attack sheet down by one uniform
     factor on both axes (0.625x: 512->320, 288->180) — not a horizontal-only
     squeeze, so the art's proportions are unchanged, just smaller. 512 and
     144 both divide evenly by that factor (320 and 90 exactly), so the
     result fills the target width edge-to-edge with no leftover gap and no
     padding needed.
  2. **Backwards-facing art**: Monster 2's source frames face right by
     default; Monster 1's (`monster1.png`/`monster_attack.png`) face left,
     which is what the shared flip logic in `monster_ai` (§4 below) assumes.
     Every frame in all 3 sections was mirrored left-right *in place*
     (each grid cell's content flipped, its position/column order left
     alone) so frame sequencing is untouched and Monster 2 now visually
     faces/walks the direction `monster_ai` actually moves it.

  Section layout in the current `monster2_combined.png` (320 wide):
  - `assets/textures/monster2.png`, mirrored, at y=0 — **walk** (320x160)
  - `assets/textures/monster2_attack.png`, mirrored + scaled to 320x180, at
    y=160 — **attack**
  - `assets/textures/monster2_jump.png`, mirrored, at y=340 — **jump**
    (320x160)

  Combined size: 320x500.
- **How the combined sprite is interpreted**: `TextureAtlasLayout::from_grid`
  slices a rectangular pixel region into a grid of equal-size cells, indexed
  row-major (left-to-right, top-to-bottom). `AtlasHandles` (a
  `HashMap<String, AnimationIndices>` resource, populated in
  [`src/systems/loader.rs`](../src/systems/loader.rs)'s `init`) then maps a
  name (e.g. `"walk2"`) to a `first`/`last` index range within that grid, so
  an animation is just "cycle the atlas index from `first` to `last`, then
  wrap" (see `animate_monster_sprite` in `monster.rs`).
- **Normal/idle (walk) animation** — already wired up:
  - `load_monster2_texture` builds a layout with `TextureAtlasLayout::from_grid(UVec2::new(80, 80), 4, 2, None, None)`
    — an 80x80, 4-column x 2-row grid, which is exactly `monster2.png`'s
    320x160 top section (8 frames, indices 0–7).
  - `loader::init` registers it as `AnimationIndices { first: 0, last: 7 }`
    under the key `"walk2"` in `AtlasHandles`.
  - `MonsterKind::walk_animation_key()` (`monster.rs`) maps
    `MonsterKind::Monster2 -> "walk2"` (and `Monster1 -> "walk"`), and
    `spawn_monster_at` looks the key up via `atlas_handles.0.get(kind.walk_animation_key())`
    to set the sprite's initial `AnimationIndices`/atlas index.
- **Where the attack animation would be connected later**: the attack
  section is already present in `monster2_combined.png` at y=160, but **its
  cell size is still different from the walk section even after the width
  fix** — it's an 8-frame, 4-column x 2-row grid of **80x90** cells (not
  80x80 — same width as walk now, but taller per frame). Because of that,
  one `TextureAtlasLayout` still can't cover both sections — a future
  developer needs a *second* layout, built with the `offset` parameter
  `TextureAtlasLayout::from_grid` already accepts (passed as `None` today):
  ```rust
  TextureAtlasLayout::from_grid(UVec2::new(80, 90), 4, 2, None, Some(UVec2::new(0, 160)))
  ```
  then register its 8 frames under a new `AtlasHandles` key (e.g.
  `"attack2"`) in `loader::init`, and load that second layout alongside the
  walk one in `load_monster2_texture` (or a new function) so both are
  available when spawning.
- **Where the jump animation would be connected later**: same idea, the
  jump section starts at y = 160 + 180 = 340, is 320x160, an 8-frame,
  4-column x 2-row grid of **80x80** cells (same cell size as walk):
  ```rust
  TextureAtlasLayout::from_grid(UVec2::new(80, 80), 4, 2, None, Some(UVec2::new(0, 340)))
  ```
  registered under a new key (e.g. `"jump2"`).
- **Sprite facing / horizontal flip**: both Monster 1 and Monster 2 rely on
  the *same* shared flip logic at the end of `monster_ai` (`monster.rs`) —
  it mirrors the monster's root `rb_transform.scale.x` based on
  `move_dir.x`, and assumes the sprite's un-mirrored (positive `scale.x`)
  frame faces **left**. This is purely a property of the baked-in pixel art
  (see fix 2 above) — nothing in the animation/movement code is
  monster-kind-specific here, and none of it should become
  monster-kind-specific: any new monster's source art just needs to face
  left by default (or be corrected the same way Monster 2's was) to work
  with this shared logic unmodified.

---

## 3. Movement

- **Where Monster 2's current random movement is implemented**: nowhere new
  — it reuses the `MonsterState::Idle` match arm inside the `monster_ai`
  system in `src/systems/monster.rs` verbatim, unmodified. That arm already
  does exactly "wander in a random direction, re-picking every couple of
  seconds, only committing to directions that are clear of walls/water a
  short distance ahead" via `pick_wander_direction` (also in `monster.rs`),
  which itself calls `terrain::is_area_clear`
  (`src/systems/terrain.rs`). Monster 2 gets this for free because it never
  leaves the `Idle` state — see §4 (AI) for exactly why.
- **Which existing movement/navigation system it uses**: the same one
  Monster 1 uses for idle wandering — no new movement code, no new
  navigation system, no new obstacle-avoidance code was written for Monster
  2. Monster-vs-monster separation (`resolve_monster_collisions`) and the
  stuck/pathfinding machinery (`seek_along_path`, `pathfinding::find_path`
  in `src/systems/monster_ai/pathfinding.rs`) are also fully shared — they
  key off `perception.state`, not monster identity.
- **Where future movement behavior should be added or modified**: if
  Monster 2 needs bespoke wandering (different speed, different wander
  radius, pauses, etc.) rather than reusing Monster 1's exact
  `MonsterCombatConfig::move_speed`/`wander_lookahead`, the cleanest path is
  a parallel small resource (mirroring `Monster2Config`) with its own
  movement numbers, read inside the `MonsterState::Idle` arm of
  `monster_ai` behind an `is_monster2` check (the query already fetches
  `Option<&Monster2>` — see §4). For a genuinely different movement
  *pattern* (e.g. patrol routes), that arm is the place to branch.

---

## 4. AI

- **Where Monster AI behavior is located**: the `monster_ai` system in
  `src/systems/monster.rs`, plus the state machine it drives,
  `src/systems/monster_ai/state.rs` (`MonsterState`: `Idle`, `Investigate`,
  `Chase`, `Attack`), and the per-monster senses data,
  `src/systems/monster_ai/perception.rs` (`MonsterPerception`).
- **Where target detection is handled**: `state::evaluate` in
  `src/systems/monster_ai/state.rs`. This is the *only* function that can
  move a monster's `perception.state` out of `Idle` into `Investigate`,
  `Chase`, or (from there) `Attack` — it does the vision cone, hearing
  radius, and proximity checks against the player.
- **Where player detection/aggro is handled for existing monsters
  (Monster 1)**: `monster_ai` calls `state::evaluate` once per monster per
  `perception.sense_timer` tick, gated by:
  ```rust
  if has_player && !is_monster2 {
      if perception.sense_timer.just_finished() {
          state::evaluate(&mut perception, ...);
          // + pack telepathy (MonsterHiveMind / PlayerEscapeModel)
      }
  } else {
      perception.state = MonsterState::Idle;
      ...
  }
  ```
  **This `!is_monster2` condition is the entire reason Monster 2 currently
  has no detection/aggro/chase/attack**: it's grouped into the same branch
  as "no player exists yet", which forces `perception.state` back to `Idle`
  every tick and never calls `state::evaluate` at all. `is_monster2` comes
  from `Option<&Monster2>` in `monster_ai`'s query tuple.
- **Where future Monster 2 chase behavior should be integrated**: remove
  (or relax, e.g. tie to a new `Monster2Config` flag) the `&& !is_monster2`
  condition above. At that point Monster 2 will start running through
  exactly the same `state::evaluate` / pack-telepathy / `MonsterState::Chase`
  path Monster 1 already uses — no new state machine needed. If Monster 2's
  senses (vision range, hearing, memory) should differ from Monster 1's,
  that's `MonsterSenseConfig`/`sense_config` in
  `src/systems/monster_ai/difficulty.rs` — either give Monster 2 its own
  variant there or branch on `is_monster2` where `monster_ai` currently
  calls `sense_config(sense_inputs.difficulty.0)`.

---

## 5. Combat

- **Where existing monster attacks are implemented**: the
  `MonsterState::Attack` arm of `monster_ai`'s big `match perception.state`
  block, in `src/systems/monster.rs`. It's driven by `ai.action_cooldown`
  and the child sprite's `AttackStatus`/`FinishStatus` components.
- **Where attack animations are triggered**: inside that same `Attack` arm —
  it looks up `atlas_handles.0.get("attack")` and sets the sprite's
  `AnimationIndices`/atlas index to it, then `animate_monster_sprite`
  (`monster.rs`) cycles frames and flips `FinishStatus` when the swing's
  last frame plays.
- **Where damage is calculated**: `MonsterCombatConfig::attack_damage`
  (`monster.rs`), a flat number, difficulty-independent (see that struct's
  doc comment).
- **Where the player receives damage**: still inside the `Attack` arm, on
  the `finish.0` (swing-landed) branch:
  ```rust
  if monster_pos.distance(player_pos) <= combat_config.attack_range {
      player_data.damage(combat_config.attack_damage);
      player_data.can_heal.reset();
  }
  ```
  `PlayerData::damage` lives in
  [`src/resourses/physics_resources.rs`](../src/resourses/physics_resources.rs).
- **Where Monster 2's future attack behavior should be connected**: Monster
  2 never reaches `MonsterState::Attack` today (see §4 — it never leaves
  `Idle`), so this whole arm is currently dead code for Monster 2. Once §4's
  detection gate is opened up for Monster 2, it will start entering `Attack`
  automatically and this arm will just work — but it currently calls
  `atlas_handles.0.get("attack")` (Monster 1's key) unconditionally, so
  before Monster 2 can actually attack, that lookup needs to branch on
  `is_monster2` to use `"attack2"` instead (see §2 for registering that
  key), and any Monster-2-specific damage/range numbers belong in
  `Monster2Config` alongside `test_hp`, read the same way `combat_config`'s
  fields are read here.

---

## 6. Death and XP

- **Where Monster 2 death is handled**: the same generic kill branch every
  monster (both kinds) goes through in `monster_ai`, `src/systems/monster.rs`:
  ```rust
  if ai.health <= 0.0 {
      let (xp_min, xp_max) = if is_monster2 {
          (loot_assets.monster2.test_xp_min, loot_assets.monster2.test_xp_max)
      } else {
          (combat_config.kill_xp_min, combat_config.kill_xp_max)
      };
      let xp_reward = rand::thread_rng().gen_range(xp_min..=xp_max);
      killed.write(MonsterKilledEvent { xp_reward });
      ...
  }
  ```
  Physics teardown and despawn below that are 100% shared/unmodified. Loot
  (the apple drop) is explicitly skipped for Monster 2 (`if !is_monster2 && ...`)
  since it's Monster 1-specific flavor, not part of Monster 2's basic scope
  — see §7 in the task/PR description for why, and add Monster 2 loot in
  that same `if`/`else` if/when it's wanted.
- **Where the existing XP reward system is called**: `MonsterKilledEvent`
  (defined in `src/resourses/physics_resources.rs`) is read by
  `track_kills` in
  [`src/systems/quests/progress.rs`](../src/systems/quests/progress.rs),
  which calls `PlayerLevel::add_xp` (`src/systems/progression.rs`) — the
  same level-up/carry-over/multi-level-up logic used for every other XP
  source in the game (quest rewards, Monster 1 kills). Monster 2 does not
  have, and must not get, its own separate XP-granting code path.
- **Where the Monster 2 XP reward is configured**: `Monster2Config::test_xp_min`/
  `test_xp_max` in `src/systems/monster.rs` — see §1.
- **How a future developer can change the XP reward**: edit
  `Monster2Config`'s values in `MonsterPlugin::build` (§1). No other file
  needs to change — the kill branch above already reads from
  `Monster2Config` for any `Monster2`-marked entity.
- **Duplicate-reward safety**: `MonsterKilledEvent` is a standard Bevy
  message — `MessageReader`/`Messages` delivers each event to a given
  reader exactly once, so `track_kills` can't double-grant XP for one kill
  regardless of UI state, respawns, or reconnects. The kill itself is only
  ever reported from this one `ai.health <= 0.0` check inside `monster_ai`
  (server-authoritative gameplay logic), never from UI/input code.

---

## 7. Spawning

- **Where Monster 2 is registered with the existing spawn system**:
  `spawn_monsters_system` in `src/systems/monster.rs` — the same system
  that already spawns Monster 1, extended (not duplicated) to also spawn
  Monster 2. It loads both kinds' textures once per spawn tick
  (`load_monster_texture` + `load_monster2_texture`), then for each open
  spawn slot:
  ```rust
  let kind = if rand::random::<f32>() < monster2_config.spawn_chance {
      MonsterKind::Monster2
  } else {
      MonsterKind::Monster1
  };
  ```
  and calls the shared `spawn_monster_at(..., kind, ...)`.
- **How its spawn configuration works**: Monster 1 and Monster 2 currently
  share one population budget — `population_config(difficulty)` in
  `src/systems/monster_ai/difficulty.rs` still returns a single
  `max_monsters` for *all* monsters combined; `Monster2Config::spawn_chance`
  only decides the *mix* of kinds within that shared budget, not an
  additional/separate cap. This was the simplest way to fold a second
  monster into the existing spawner without a parallel system.
- **How the existing terrain/noise-map validation works**: unchanged,
  fully shared — `spawn_monsters_system`'s candidate-position loop (pick a
  random point in an annulus around the player, check
  `terrain::is_area_clear(&terrain_map, pos, spawn_half_extent)`, retry up
  to `MonsterConfig::max_spawn_attempts`) runs identically regardless of
  which kind ends up spawned there; `terrain::is_area_clear`
  (`src/systems/terrain.rs`) already handles both "real terrain is loaded"
  and "fall back to the same noise-map prediction terrain generation uses"
  cases, so Monster 2 gets that for free.
- **Save/restore**: a monster's kind is persisted —
  `MonsterSaveData.kind: MonsterKind` in
  [`src/systems/save.rs`](../src/systems/save.rs) (`#[serde(default)]`, so
  saves from before Monster 2 existed load as `Monster1`). Restoring a save
  (`handle_play_requested` in
  [`src/systems/lifecycle.rs`](../src/systems/lifecycle.rs)) loads both
  texture pairs and picks the right one per saved monster via
  `monster.kind`. The Leave-handler and the Pause-menu Save button (also in
  `lifecycle.rs` and `src/systems/menu_ui.rs` respectively) read a monster's
  kind back off its `Option<&Monster2>` component when writing the save.
- **Which part should be modified if Monster 2 needs unique spawn rules
  later** (its own population cap, a different spawn distance/ring, being
  restricted to a specific biome, etc.): `spawn_monsters_system` is the one
  place to change — either give it a second `to_spawn`/candidate loop keyed
  off a new `Monster2`-specific count (rather than sharing Monster 1's
  budget via `spawn_chance`), or add the extra condition (biome check,
  etc.) into the existing candidate-position loop's `if !terrain::is_area_clear(...)`
  check, guarded on `kind == MonsterKind::Monster2`.

---

## What's deliberately NOT implemented

Per the current task scope, none of the following exist for Monster 2 yet —
each section above says exactly where to add it:

- Player chasing / detection / aggro (§4)
- Attacking the player / damage / combat AI (§5)
- Special abilities, unique attacks, boss mechanics, status effects
- Advanced jump behavior (the jump animation is only present in the
  combined sprite, see §2 — no jump *logic* exists)
- Unique Monster 2 loot (§6 — currently drops nothing on death)
- A separate Monster 2 population cap (§7 — currently shares Monster 1's
  budget via `spawn_chance`)
