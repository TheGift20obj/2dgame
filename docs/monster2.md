# Monster 2 — developer guide

Monster 2 spawns, wanders randomly, avoids walls/water, detects/chases the
player using the same senses Monster 1 uses, and attacks with a **leap**: it
locks onto the player's position, jumps at them (with a temporary speed
boost while airborne), bites, lands, and resumes normal behavior. It can be
killed and grants its own configured XP. It has **no special abilities,
unique loot, or boss mechanics** — this document says exactly where each of
those (and any other future behavior) should be added, and which existing
systems (shared with Monster 1) to reuse.

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
  touching Monster 1 at all). The same file also has `Monster2Sprite`
  (marks Monster 2's sprite *child* entity specifically — see §2) and
  `Monster2AttackJump` (the attack-leap's per-entity state — see §5).
- **Stats (HP, XP, spawn mix, attack-leap numbers)**: `Monster2Config`
  resource in `src/systems/monster.rs`. Fields:
  - `test_hp: f32` — HP a freshly spawned Monster 2 starts with.
  - `test_xp_min` / `test_xp_max: u32` — random per-kill XP range (same
    mechanism as Monster 1's `MonsterCombatConfig::kill_xp_min`/
    `kill_xp_max`, just a separate, independently-tunable range).
  - `spawn_chance: f32` — chance (0.0–1.0) that a given spawn slot becomes a
    Monster 2 instead of a Monster 1.
  - `jump_speed_multiplier: f32` — how much faster than normal ground
    `MonsterCombatConfig::move_speed` Monster 2 travels while airborne.
  - `jump_duration_secs: f32` — how long the leap's airborne (boosted
    movement) phase lasts.
  - `jump_height: f32` — purely visual: how high (world units) the sprite
    arcs above ground at the leap's midpoint.
  - `attack_jump_distance: f32` — max distance to the player at which
    Monster 2 commits to a leap instead of continuing to Chase — its own
    (longer, pounce-sized) equivalent of `MonsterCombatConfig::attack_range`.
  - `attack_cooldown_secs: f32` — seconds between leaps.
  - `attack_damage: f32` — damage dealt on a successful bite.

  Default values are set once, in one place, in `MonsterPlugin::build`
  (`src/systems/monster.rs`):
  ```rust
  .insert_resource(Monster2Config {
      test_hp: 60.0,
      test_xp_min: 15,
      test_xp_max: 30,
      spawn_chance: 0.25,
      jump_speed_multiplier: 3.0,
      jump_duration_secs: 0.45,
      jump_height: 40.0,
      attack_jump_distance: 4.0 * TILE_SIZE,
      attack_cooldown_secs: 3.0,
      attack_damage: 15.0,
  })
  ```
  **To change any Monster 2 number — HP, XP, or the whole attack-leap feel —
  edit this one block.** Nothing else needs to change.
- **Registration**: `MonsterKind` doesn't need a separate "registry" —
  `spawn_monsters_system` (below) is the only place a kind is chosen, and
  `spawn_monster_at` is the only place a kind is turned into an entity.

---

## 2. Sprite and animation

- **`monster2_combined.png`** lives at `assets/textures/monster2_combined.png`
  and is referenced in `load_monster2_texture` in `src/systems/monster.rs`
  (Monster 2's walk layout) and in `Monster2AnimationLayouts`
  (`src/systems/loader.rs`, all 3 layouts — see below).
- **How it was generated**: same vertical-stack pipeline as Monster 1's
  `monster_combined.png` (`image` crate, `ImageBuffer`/`copy_from`,
  left-aligned at x=0, no gaps) — see the commented-out block in
  [`src/main.rs`](../src/main.rs) right after the `player_combined.png`
  block — plus two corrections the raw source images needed that Monster
  1's didn't (both done once, baked into the PNG — nothing about this
  happens at runtime): a uniform (aspect-preserving) downscale of
  `monster2_attack.png` so every section shares one width, and mirroring
  every frame in place so the art faces the same direction Monster 1's does
  (see the block's own comments for the exact numbers). Section layout in
  the current `monster2_combined.png` (320 wide, 500 tall total):
  - y=0 — **walk**, 320x160, 80x80 cells, 4x2 grid (from `monster2.png`)
  - y=160 — **attack**, 320x180, 80x90 cells, 4x2 grid (from
    `monster2_attack.png`)
  - y=340 — **jump**, 320x160, 80x80 cells, 4x2 grid (from
    `monster2_jump.png`)
- **How the combined sprite is interpreted**: `TextureAtlasLayout::from_grid`
  slices a rectangular pixel region into a grid of equal-size cells, indexed
  row-major (left-to-right, top-to-bottom); its `offset` parameter is where
  that grid starts within the source image, which is how the attack/jump
  sections (not at y=0) get sliced correctly. `AtlasHandles` (a
  `HashMap<String, AnimationIndices>` resource, populated in
  [`src/systems/loader.rs`](../src/systems/loader.rs)'s `init`) maps a name
  (e.g. `"walk2"`) to a `first`/`last` index range *within* one specific
  grid, so an animation is just "cycle the atlas index from `first` to
  `last`, then wrap" (see `animate_monster_sprite` in `monster.rs`) — but
  which `TextureAtlasLayout` (i.e. which grid) the sprite's `atlas.layout`
  currently points at has to be swapped alongside the index range whenever
  Monster 2 switches sections, since walk/attack/jump don't share one grid
  the way Monster 1's 2 sections do (both are 64x64 there).
- **All 3 sections are wired up**, each with its own `AtlasHandles` entry
  and `TextureAtlasLayout` (`loader::init`, `Monster2AnimationLayouts`):

  | Section | `AtlasHandles` key | `Monster2AnimationLayouts` field | Cell size | Frames |
  |---|---|---|---|---|
  | Walk  | `"walk2"`   | `.walk`   | 80x80 | 0–7 |
  | Attack | `"attack2"` | `.attack` | 80x90 | 0–7 |
  | Jump  | `"jump2"`   | `.jump`   | 80x80 | 0–7 |

  `MonsterKind::walk_animation_key()` (`monster.rs`) maps
  `Monster2 -> "walk2"` (and `Monster1 -> "walk"`) for the sprite's
  *initial* animation at spawn. Switching to/from attack at runtime is done
  by `monster_ai`'s `MonsterState::Attack` handling for Monster 2 (§5),
  which sets both `atlas.layout` (to `monster2_layouts.attack`/`.walk`) and
  `AnimationIndices`/`atlas.index` (to `"attack2"`/`"walk2"`'s range)
  together — the two must always change as a pair, or the sprite renders
  the wrong grid's pixels at a given index. `animate_monster_sprite` does
  the same pairing when an attack/jump animation naturally finishes (it
  checks the sprite child's `Monster2Sprite` marker to know which pair to
  reset to instead of always assuming Monster 1's `"walk"`).
- **The jump section/key (`"jump2"`, `Monster2AnimationLayouts.jump`) is
  registered but not currently triggered by any code** — Monster 2's attack
  uses the **attack** animation for its leap (per spec: "the existing
  `monster2_attack` animation should be used for this attack" — see §5),
  not the jump one. If a future developer wants the jump sheet to actually
  play (e.g. for a distinct non-attack traversal hop), the pattern to copy
  is exactly the attack takeoff/landing code in `monster_ai`'s
  `MonsterState::Attack` arm (§5): set `atlas.layout =
  monster2_layouts.jump.clone()` and the `AnimationIndices`/`atlas.index`
  to `"jump2"`'s range together, and reset back to walk2 the same way.
- **Sprite facing / horizontal flip**: both Monster 1 and Monster 2 rely on
  the *same* shared flip logic at the end of `monster_ai` (`monster.rs`) —
  it mirrors the monster's root `rb_transform.scale.x` based on
  `move_dir.x`, and assumes the sprite's un-mirrored (positive `scale.x`)
  frame faces **left**. This is purely a property of the baked-in pixel art
  (see the mirroring step above) — nothing in the animation/movement code
  is monster-kind-specific here, and none of it should become
  monster-kind-specific: any new monster's source art just needs to face
  left by default (or be corrected the same way Monster 2's was) to work
  with this shared logic unmodified. During the attack leap, `move_dir`
  comes from the *locked* jump direction (§5), so the sprite still flips to
  face the direction it's actually leaping, consistently, for the whole
  jump — it just can't flip again mid-leap toward some new direction, since
  the locked direction never changes until landing.

---

## 3. Movement

- **Where Monster 2's normal (non-attacking) movement is implemented**:
  nowhere new — it reuses the `MonsterState::Idle` and
  `MonsterState::Chase`/`Investigate` match arms inside the `monster_ai`
  system in `src/systems/monster.rs` verbatim, unmodified, exactly like
  Monster 1. Idle wandering picks a random clear direction via
  `pick_wander_direction`; Chase/Investigate use `nav_target_for` +
  `seek_along_path` (A* pathfinding, `src/systems/monster_ai/pathfinding.rs`).
  Monster-vs-monster separation (`resolve_monster_collisions`) applies to
  both states too. None of this required any Monster-2-specific code.
- **Where the attack-leap's movement is implemented**: a dedicated
  "airborne" branch in `monster_ai`, checked *before* the
  `match perception.state` block — see §5 and §6. It reuses the exact same
  movement pipeline as everything else (`velocity` -> collision resolution
  -> `rigid_body.set_linvel(...)`), just with a boosted `velocity` and a
  frozen direction; it's not a separate movement system.
- **Where future movement behavior should be added or modified**: if
  Monster 2 needs bespoke *grounded* wandering (different speed, different
  wander radius, pauses, etc.) rather than reusing Monster 1's exact
  `MonsterCombatConfig::move_speed`/`wander_lookahead`, add the numbers to
  `Monster2Config` and branch on `is_monster2` inside the `MonsterState::Idle`
  arm of `monster_ai`. For a genuinely different movement *pattern* (e.g.
  patrol routes), that arm is the place to branch. For anything about the
  attack leap itself, see §5.

---

## 4. AI

- **Where Monster AI behavior is located**: the `monster_ai` system in
  `src/systems/monster.rs`, plus the state machine it drives,
  `src/systems/monster_ai/state.rs` (`MonsterState`: `Idle`, `Investigate`,
  `Chase`, `Attack`), and the per-monster senses data,
  `src/systems/monster_ai/perception.rs` (`MonsterPerception`).
- **Where target detection/aggro is handled**: `state::evaluate` in
  `src/systems/monster_ai/state.rs` — the *only* function that can move a
  monster's `perception.state` out of `Idle` into `Investigate`, `Chase`, or
  (from there) `Attack`. It does the vision cone, hearing radius, and
  proximity checks against the player, and decides Chase vs. Attack purely
  by comparing distance to whatever `attack_range` value it's given.
  **Monster 1 and Monster 2 share this detection/state machine completely,
  unmodified** — `monster_ai` calls it identically for both kinds:
  ```rust
  let attack_range = if is_monster2 {
      loot_assets.monster2.attack_jump_distance
  } else {
      combat_config.attack_range
  };
  state::evaluate(&mut perception, ..., attack_range, ...);
  ```
  The *only* difference is which distance threshold flips Chase -> Attack:
  Monster 2 gets its own (longer, pounce-sized) `attack_jump_distance`
  instead of Monster 1's short melee `attack_range`, since Monster 2's
  "attack" is a leap that needs real distance to jump across (see §5).
- **If Monster 2's senses (vision range, hearing, memory) should ever
  differ from Monster 1's** (not currently the case — both use
  `sense_config(sense_inputs.difficulty.0)`): either give Monster 2 its own
  variant in `MonsterSenseConfig`/`sense_config`
  (`src/systems/monster_ai/difficulty.rs`), or branch on `is_monster2` at
  that call site in `monster_ai`.
- **Once Monster 2 is airborne mid-leap, `perception.state` is no longer
  authoritative for its movement** — see §5/§6 for why, and where that
  priority is enforced.

---

## 5. Combat — the attack leap

Monster 2's attack is a **leap toward the player**, using the `monster2_attack`
animation (per spec) — not a stand-still swing like Monster 1's. Its
lifecycle, all in `monster_ai` (`src/systems/monster.rs`):

```
Grounded, Chase state, within attack_jump_distance, cooldown ready
        |
        v
  TAKEOFF  (MonsterState::Attack arm, is_monster2 branch)
    - direction = normalize(player_pos - monster_pos)   <- calculated ONCE
    - Monster2AttackJump { airborne: true, direction, elapsed: 0.0 }
    - velocity = direction * move_speed * jump_speed_multiplier
    - attack2 animation starts (atlas.layout + AnimationIndices swapped together)
        |
        v
  AIRBORNE  (the "airborne bypass" branch, checked BEFORE the state match)
    - every frame: velocity = leap.direction * move_speed * jump_speed_multiplier
      (leap.direction is only ever READ here, never recalculated)
    - sprite child's local Y = MONSTER_SPRITE_BASE_Y + parabolic arc(elapsed / jump_duration_secs)
      (root Transform, collider, light — all untouched, see §8/§9)
    - when elapsed >= jump_duration_secs: airborne = false (lands)
        |
        v
  BITE  (driven by the attack2 animation's own last frame, via FinishStatus —
         independent of jump_duration_secs, may happen slightly after landing)
    - if still within combat_config.attack_range: damage the player
      (Monster2Config::attack_damage, via the existing PlayerData::damage)
    - sprite resets to walk2 (layout + indices swapped together)
    - ATTACK END — perception.state (whatever state::evaluate has since
      set it to) takes back over next frame
```

- **Where existing (Monster 1) monster attacks are implemented**: the
  `MonsterState::Attack` arm of `monster_ai`'s `match perception.state`
  block — driven by `ai.action_cooldown` and the child sprite's
  `AttackStatus`/`FinishStatus` components, completely unmodified. Monster
  2's takeoff/bite logic lives in the *same* arm, in an `if is_monster2 {
  ... } else { ...original Monster 1 code... }` split, so Monster 1's path
  is untouched code, not just untouched behavior.
- **Where the jump direction is locked, and why it can't drift**: takeoff
  computes `direction` once and stores it in `Monster2AttackJump.direction`
  (`src/resourses/physics_resources.rs`). Every frame of the *airborne*
  branch reads `leap.direction` back out — it is never recalculated from
  the player's current position while airborne, so the player moving
  mid-leap has no effect on where Monster 2 is headed (see the type's own
  doc comment).
- **Where normal movement/pathfinding is prevented from overriding the
  leap**: the airborne branch is checked *before* `match perception.state`,
  so while `leap.airborne` is true, the `Idle`/`Chase`/`Investigate`/`Attack`
  arms (wandering, pathfinding, a fresh takeoff) simply don't run at all
  that frame — regardless of what `perception.state` currently is.
  This matters because `state::evaluate` keeps reassigning
  `perception.state` on its own timer independent of the leap (e.g. it
  could decide Chase again mid-air if the player runs out of
  `attack_jump_distance`); the airborne check ignores that entirely.
  `resolve_monster_collisions` (monster-vs-monster separation/yield
  steering) is skipped the same way (`... && !airborne`) so a nearby
  packmate can't redirect an airborne leap either — obstacle-avoidance
  steering only ever applies to *grounded* movement (Idle/Chase); real wall
  collision (physics) still applies, same as it would for a real leap.
- **Where damage is calculated**: `Monster2Config::attack_damage` — read in
  the takeoff/bite's `is_monster2` branch in place of Monster 1's
  `MonsterCombatConfig::attack_damage`.
- **Where the player receives damage**: same `finish.0` (animation-landed)
  branch, same `PlayerData::damage` call
  ([`src/resourses/physics_resources.rs`](../src/resourses/physics_resources.rs))
  Monster 1 already uses — no duplicate combat system.
- **Speed boost — where it applies and where it doesn't**: both the takeoff
  and airborne branches compute `velocity` from
  `combat_config.move_speed * loot_assets.monster2.jump_speed_multiplier` —
  `move_speed` itself (used by grounded Idle/Chase movement) is never
  written to, so there's no permanent speed change; the boost only ever
  shows up in the one `velocity` value used for that frame's leap movement.

---

## 6. "Do not steer during the jump" — how that's actually enforced

This was called out as easy to get wrong, so it's summarized here
separately from §5's walkthrough:

- `Monster2AttackJump.direction` is written in exactly one place (takeoff)
  and read in exactly one other place (the airborne branch, every frame) —
  there is no third place that touches it, so nothing can recompute it
  mid-leap.
- The airborne branch runs *instead of* (not alongside) the
  `match perception.state` block for that frame, so `nav_target_for` /
  `seek_along_path` / the wander code physically cannot execute while
  `leap.airborne` is true, regardless of `perception.state`.
- `resolve_monster_collisions` is explicitly skipped while airborne (see
  §5), so packmate separation/yield can't nudge the direction either.

If a future developer wants homing/steerable leaps, the change is narrow
and obvious from this structure: read `player_pos` again inside the
airborne branch instead of `leap.direction`. That's a deliberate design
change, not a bug fix — the current behavior (locked direction) is what was
specified.

---

## 7. Death and XP

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
  since it's Monster 1-specific flavor, not part of Monster 2's scope — add
  Monster 2 loot in that same `if`/`else` if/when it's wanted.
- **Where the existing XP reward system is called**: `MonsterKilledEvent`
  (defined in `src/resourses/physics_resources.rs`) is read by
  `track_kills` in
  [`src/systems/quests/progress.rs`](../src/systems/quests/progress.rs),
  which calls `PlayerLevel::add_xp` (`src/systems/progression.rs`) — the
  same level-up/carry-over/multi-level-up logic used for every other XP
  source in the game. Monster 2 does not have, and must not get, its own
  separate XP-granting code path.
- **Where the Monster 2 XP reward is configured**: `Monster2Config::test_xp_min`/
  `test_xp_max` in `src/systems/monster.rs` — see §1.
- **Duplicate-reward safety**: `MonsterKilledEvent` is a standard Bevy
  message — `MessageReader`/`Messages` delivers each event to a given
  reader exactly once, so `track_kills` can't double-grant XP for one kill.
  The kill itself is only ever reported from the one `ai.health <= 0.0`
  check inside `monster_ai` (server-authoritative gameplay logic), never
  from UI/input code. The attack-leap's damage is guarded the same way —
  `finish.0` is a one-shot flag, cleared the instant it's read, so a bite
  can't land twice from one leap.

---

## 8. Spawning

- **Where Monster 2 is registered with the existing spawn system**:
  `spawn_monsters_system` in `src/systems/monster.rs` — the same system
  that already spawns Monster 1, extended (not duplicated) to also spawn
  Monster 2. It loads both kinds' textures once per spawn tick
  (`load_monster_texture` + `load_monster2_texture`), then for each open
  spawn slot rolls `MonsterKind` via `monster2_config.spawn_chance` and
  calls the shared `spawn_monster_at(..., kind, ...)`.
- **How its spawn configuration works**: Monster 1 and Monster 2 currently
  share one population budget — `population_config(difficulty)`
  (`src/systems/monster_ai/difficulty.rs`) still returns a single
  `max_monsters` for *all* monsters combined; `spawn_chance` only decides
  the *mix* of kinds within that shared budget.
- **How the existing terrain/noise-map validation works**: unchanged, fully
  shared — the candidate-position loop's `terrain::is_area_clear` check
  (`src/systems/terrain.rs`) runs identically regardless of which kind ends
  up spawned there.
- **Save/restore**: a monster's kind is persisted —
  `MonsterSaveData.kind: MonsterKind` in
  [`src/systems/save.rs`](../src/systems/save.rs) (`#[serde(default)]` for
  back-compat). Restoring a save (`handle_play_requested` in
  [`src/systems/lifecycle.rs`](../src/systems/lifecycle.rs)) picks the
  right texture pair per saved monster via `monster.kind`. A mid-leap
  Monster 2 is *not* specially preserved across a save/leave — it simply
  restores grounded (`Monster2AttackJump` isn't persisted), which is
  harmless since a leap is a few-hundred-millisecond transient, not
  meaningful state to round-trip.
- **Which part should be modified if Monster 2 needs unique spawn rules
  later**: `spawn_monsters_system` — either give it its own
  `to_spawn`/candidate loop keyed off a separate count, or add an extra
  condition guarded on `kind == MonsterKind::Monster2` into the existing
  candidate-position loop.

---

## 9. Occlusion/shadow correctness during the leap

**The root cause this avoids**: this is a 2D top-down game where
`Transform.translation.y` doubles as both "position on the ground plane"
*and* the input to draw-order/depth sorting (`YSort`) — it is **not** an
elevation/height axis. Each monster's root entity carries a `PointLight2d`
child (local offset `(0, 15, 0)`), and shadows on walls come from
`bevy_firefly`'s occlusion system computing them from that light's *world*
position against `Occluder2d`-tagged wall colliders
(`src/systems/terrain.rs`) — monsters themselves are never occluders. If a
"jump height" effect were implemented by moving the monster's actual
`Transform.translation.y` (the root, physics-synced transform) upward, that
would simultaneously (and incorrectly): move the physics collider, move the
light (shifting every wall-shadow it casts to look like the monster is
standing somewhere it isn't), and feed the wrong value into Y-sort.

**The fix**: the visual arc (§5, "AIRBORNE") only ever writes to the
**sprite child's own local `Transform.translation.y`**
(`sprite_transform.translation.y = MONSTER_SPRITE_BASE_Y + arc`, in the
airborne branch of `monster_ai`) — never to `rb_transform` (the root). The
root's translation stays exactly at Monster 2's true ground position for
the entire leap, so:
- the physics collider stays where the monster actually is,
- the `PointLight2d` child (a sibling of the sprite child, unaffected by
  the sprite's own transform) stays at true ground height, so wall shadows
  stay correctly projected,
- Y-sort/draw-order stays correct, since it reads the root's position.

Only the sprite *renders* higher, exactly the "sprite rises, shadow stays
grounded" split the spec describes. `MONSTER_SPRITE_BASE_Y` (`monster.rs`)
names the constant both `spawn_monster_at`'s bundle and the arc math use, so
the two can't drift out of sync. The Attack arm's `finish.0` (landing)
branch also explicitly resets the sprite's Y back to
`MONSTER_SPRITE_BASE_Y`, in case the bite animation finishes before
`jump_duration_secs` elapses, so no visual offset can linger after a leap
ends early.

**If a future developer adds height to anything else** (a different
monster's jump, a projectile arc, ...), the same rule applies: only ever
offset a *sprite child's* local transform for a visual height effect, never
the entity's own root/physics transform — that's what keeps occlusion,
collision, and Y-sort all still reading the real ground position.

---

## What's deliberately NOT implemented

- Special abilities, unique attacks beyond the one leap-bite, boss
  mechanics, status effects
- A distinct jump/traversal move using the `"jump2"` sheet (registered but
  unused — see §2; the attack leap uses the attack sheet, per spec)
- Unique Monster 2 loot (§7 — currently drops nothing on death)
- A separate Monster 2 population cap (§8 — currently shares Monster 1's
  budget via `spawn_chance`)
- Homing/steerable leaps (§6 — direction is locked by design)
