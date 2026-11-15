# Rules Coverage Matrix — SRD 5.2.1 (2024 rules)

Legend: ✅ implemented & tested · 🟡 partial (see note) · ⏳ pending · 🚫 intentionally unsupported
(“DM adjudicates”). Every ✅ row has unit or integration tests in the crate named.

## Core mechanics

| Mechanic | Status | Where | Notes |
|---|---|---|---|
| Ability scores & modifiers | ✅ | `dnd_rules::ability` | SRD modifier table. |
| Proficiency Bonus by level | ✅ | `ability` | +2…+6. |
| D20 Tests, Advantage/Disadvantage (no stacking, cancel) | ✅ | `d20` | Reasons kept for the log. |
| Exhaustion (−2×level to D20 Tests, −5 ft×level Speed, death at 6, Long Rest −1) | ✅ | `d20`, `conditions` | Applied to attacks, initiative, death saves. |
| Dice expressions (NdS±…, d100) | ✅ | `dice` | Bounded parser for untrusted input. |
| Unbiased RNG on host | ✅ | `rng` | ChaCha20 + rejection sampling. |
| Skills (18) & saving throws | ✅ | `character` | Click-to-roll in the sheet. |
| Passive Perception | ✅ | `character` | |
| Armor Class (armor table, Dex caps, shield) | ✅ | `equipment` | SRD examples tested. |
| Hit Points, level 1 by class, fixed HP per level | ✅ | `character` | Rolled HP on level-up ⏳. |
| Temporary HP (absorb first, don't stack, choice) | ✅ | `hp` | UI choice prompt ⏳ (host keeps the higher). |
| Healing (cap at max) | ✅ | `hp` | |
| Damage types, Resistance/Vulnerability/Immunity, order of application | ✅ | `damage` | SRD 28→22 example. |
| Bloodied | ✅ | `hp` | Shown to players as status. |
| Dropping to 0 HP, Unconscious, monsters die at 0 | ✅ | `hp`, `session` | DM can treat a monster as a character via HP override. |
| Massive damage | ✅ | `hp` | SRD example. |
| Death Saving Throws (DC 10, nat 1 = 2 failures, nat 20 = 1 HP, stable at 3) | ✅ | `hp`, `session` | Rolled automatically at turn start. |
| Damage at 0 HP (1 failure, crit 2, ≥ max = death) | ✅ | `hp` | |
| Stabilizing (Help + Medicine) | ⏳ | | |
| Resting (Short/Long), Hit Point Dice spending | ⏳ | | |
| Experience & levels 2–20, milestone | 🟡 | | XP stored/shown; level-up flow ⏳. |
| Heroic Inspiration | ⏳ | | |

## Conditions

| Condition | Status | Effects implemented |
|---|---|---|
| Blinded | ✅ | Can't see (vision), attacks vs Advantage, own attacks Disadvantage. |
| Charmed | 🟡 | Tracked; targeting restriction ⏳. |
| Deafened | 🟡 | Tracked; hearing checks ⏳. |
| Exhaustion | ✅ | See above. |
| Frightened | 🟡 | Disadvantage on attacks (source assumed in sight); "can't approach" ⏳. |
| Grappled | ✅ | Speed 0, Disadvantage vs non-grappler. Grapple action ⏳. |
| Incapacitated | ✅ | No actions/reactions; Disadvantage on Initiative. |
| Invisible | ✅ | Unseen rules, Advantage on Initiative, hidden from vision (Blindsight sees). |
| Paralyzed | ✅ | Incapacitated, Speed 0, auto-fail Str/Dex saves, attacks Advantage, auto-crit within 5 ft. |
| Petrified | 🟡 | Incapacitated, Speed 0, attacks Advantage, auto-fail saves; resist-all & poison immunity ⏳ on toggle. |
| Poisoned | ✅ | Disadvantage on attacks (and ability checks in sheet ⏳). |
| Prone | ✅ | Attacks within 5 ft Advantage / beyond Disadvantage, own attacks Disadvantage. Crawl/stand cost ⏳. |
| Restrained | ✅ | Speed 0, attacks vs Advantage, own Disadvantage; Dex save Disadvantage ⏳. |
| Stunned | ✅ | Incapacitated, auto-fail Str/Dex saves, attacks Advantage. |
| Unconscious | ✅ | Incapacitated + Prone, Speed 0, auto-crit within 5 ft. |

## Combat

| Mechanic | Status | Notes |
|---|---|---|
| Initiative (Dex check, Alert +PB, Surprise = Disadvantage, Invisible = Advantage) | ✅ | Ties: Dex then id; DM reorders (`ReorderInitiative`). |
| Rounds, turns, turn ownership | ✅ | |
| Movement & Speed, grid squares, diagonals = 5 ft | ✅ | Alternating variant optional. |
| Difficult Terrain (+1 ft per ft, not cumulative) | ✅ | Tiles, objects, non-ally creatures. |
| Corner rule (no diagonal past walls) | ✅ | |
| Moving through creatures (ally / incapacitated / 2 sizes / Tiny) | ✅ | Can't end in occupied space. |
| Action economy: Action / Bonus Action / Reaction | 🟡 | Action & Reaction tracked; Bonus Actions ⏳ (no bonus-action features yet). |
| Attack action, attack rolls, nat 20/1, Critical Hits (dice twice) | ✅ | |
| Ranged: normal/long range, close-combat Disadvantage | ✅ | |
| Reach / Reach property | ✅ | |
| Unarmed Strike (damage option) | ✅ | Grapple/Shove options ⏳. |
| Unseen attackers/targets | ✅ | Uses host vision (light, darkvision, walls). |
| Opportunity Attacks (leaving reach, prompt, Disengage avoids) | ✅ | Interrupts movement; prompt to controller. |
| Dash, Disengage, Dodge | ✅ | |
| Help, Hide, Ready, Search, Study, Influence, Magic, Utilize | ⏳ | |
| Cover (half / three-quarters) | ⏳ | Total cover via walls ✅ (no LOS = can't target). |
| Extra Attack, two-weapon (Light) attacks, Nick | ⏳ | |
| Concentration | ⏳ | Needs spellcasting. |
| Grappling / Shove | ⏳ | |
| Area-of-effect templates | ⏳ | |
| Large+ creatures occupying multiple cells | ⏳ | Rendered/handled as one cell for now. |

## Weapon mastery

| Property | Status |
|---|---|
| Graze, Sap, Vex | ✅ automated (no downside to the wielder) |
| Topple, Push, Slow, Cleave, Nick | ⏳ need choice prompts / extra attack flow |

## Character creation (2024)

| Item | Status | Notes |
|---|---|---|
| Classes (12): hit die, saves, skill lists, weapon/armor training, mastery count | ✅ | Verified against SRD. |
| Class features beyond the core table (Rage, Spellcasting, Sneak Attack, …) | ⏳ | Biggest remaining block. |
| Subclasses (SRD: one per class) | ⏳ | |
| Species (9): size, speed, darkvision, Dwarven Toughness/Resilience, Human Skillful | 🟡 | Other species traits ⏳ (breath weapon, lineages, etc). |
| Backgrounds (SRD 4): ability options, origin feat, skills | ✅ | Tool proficiencies & equipment packages ⏳. |
| Ability scores: Standard Array, Point Buy (27) | ✅ | Rolled (4d6) needs host-side rolling ⏳ — currently refused so it can't be faked. |
| Origin feats: Alert (initiative) | ✅ | Magic Initiate, Savage Attacker ⏳. |
| Equipment choice, AC, attacks derived | ✅ | |
| Languages, tools, alignment | ⏳ | |

## Spells & magic

⏳ Not yet implemented: spell list, slots, casting, saves vs DC, spell attacks, concentration,
light-producing spells (the lighting engine already supports placed light sources that such spells
will create). The UI says so explicitly.

## Monsters (SRD 5.2.1 bestiary)

| Monster | Status |
|---|---|
| Goblin Warrior (incl. extra damage with Advantage) | ✅ |
| Skeleton (Bludgeoning vulnerability, Poison immunity) | ✅ |
| Remaining SRD stat blocks | ⏳ to be added as data files, each checked against `reference/` |
| Multiattack, recharge, legendary/lair actions, spellcasting | ⏳ |

## Vision & light

| Mechanic | Status |
|---|---|
| Bright / Dim / Darkness, ambient per room | ✅ |
| Point lights (objects, DM-placed), walls block light | ✅ |
| Darkvision (Dim→Bright, Dark→Dim in gray, range-limited) | ✅ |
| Blindsight, Blinded | ✅ |
| Truesight, magical Darkness, obscurement checks | ⏳ |
| Fog of war (unexplored / explored / visible), shared party exploration | ✅ |
