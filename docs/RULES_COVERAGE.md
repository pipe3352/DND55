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
| Resting (Short/Long), Hit Point Dice spending | ✅ | `dnd_session::features` | DM triggers rests; Long Rest (needs ≥ 1 HP) restores all HP, all spent Hit Point Dice, slots and long-rest features, and reduces Exhaustion by 1. Short Rest recharges short-rest features (Action Surge, Second Wind 1 use, Pact slots, Channel Divinity 1 use, Arcane Recovery). |
| Experience & levels 2–20, milestone | ✅ | `classes`, `features` | SRD XP table; DM awards XP or grants milestone levels; level-up dialog (fixed HP, ASI feat, Fighting Style, Expertise, new cantrips/prepared spells, Weapon Masteries; SRD subclass at 3), all validated by the host. |
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
| Action economy: Action / Bonus Action / Reaction | ✅ | Bonus Actions used by features, potions and spells; one spell slot per turn rule. |
| Attack action, attack rolls, nat 20/1, Critical Hits (dice twice) | ✅ | |
| Ranged: normal/long range, close-combat Disadvantage | ✅ | |
| Reach / Reach property | ✅ | |
| Unarmed Strike (damage option) | ✅ | Grapple/Shove options ⏳. |
| Unseen attackers/targets | ✅ | Uses host vision (light, darkvision, walls). |
| Opportunity Attacks (leaving reach, prompt, Disengage avoids) | ✅ | Interrupts movement; prompt to controller. |
| Dash, Disengage, Dodge | ✅ | |
| Help, Hide, Ready, Search, Study, Influence, Magic, Utilize | ⏳ | |
| Cover (half / three-quarters) | ⏳ | Total cover via walls ✅ (no LOS = can't target). |
| Extra Attack (Fighter, Barbarian, Monk, Paladin, Ranger), Multiattack (monsters) | ✅ | Attack action split into several attacks. Two-weapon (Light) extra attack ⏳. |
| Concentration | ✅ | One spell at a time; Con save DC max(10, half damage) capped at 30; ends at 0 HP/Incapacitated; End button. |
| Grappling / Shove | ⏳ | |
| Area-of-effect templates (Sphere, Cube, Cone, Line, Emanation, Cylinder) | ✅ | Grid templates with preview; line of effect from the origin. |
| Large+ creatures occupying multiple cells | ⏳ | Rendered/handled as one cell for now. |

## Weapon mastery

| Property | Status |
|---|---|
| Graze, Sap, Vex, Slow, Topple (Con save → Prone), Push (10 ft, Large or smaller) | ✅ automated (always applied; the SRD lets the wielder decline — DM can undo) |
| Cleave, Nick | 🟡 reported in the log when they trigger; the extra attack is resolved by the player with a normal Attack |

## Character creation (2024)

| Item | Status | Notes |
|---|---|---|
| Classes (12): hit die, saves, skill lists, weapon/armor training, mastery count | ✅ | Verified against SRD. |
| Class features beyond the core table | 🟡 | See "Class features" below. |
| Subclasses (SRD: one per class) | 🟡 | Assigned at level 3; features listed with SRD text; automated where noted below. |
| Species (9): size, speed, darkvision, Dwarven Toughness/Resilience, Human Skillful | 🟡 | Other species traits ⏳ (breath weapon, lineages, etc). |
| Backgrounds (SRD 4): ability options, origin feat, skills | ✅ | Tool proficiencies & equipment packages ⏳. |
| Ability scores: Standard Array, Point Buy (27) | ✅ | Rolled (4d6) needs host-side rolling ⏳ — currently refused so it can't be faked. |
| Origin feats: Alert (initiative) | ✅ | Magic Initiate, Savage Attacker ⏳. |
| Equipment choice, AC, attacks derived | ✅ | |
| Languages, tools, alignment | ⏳ | |

## Spells & magic

All **339 SRD 5.2.1 spells** are loaded from `content/srd_spells.json` (extracted from the SRD PDF by
`tools/extract_spells.py`, each with level, school, classes, casting time, range, components, duration,
concentration and full text). Automation is tagged per spell and shown in the spellbook:

| Level of automation | Count | Meaning |
|---|---|---|
| **automated** (`full`) | 31 | Targeting, attack roll or saving throw, damage/healing, upcasting, conditions, light and VFX are resolved by the host. |
| **partly automated** (`partial`) | 61 | Damage/save/attack resolved; secondary effects (movement, terrain, special conditions) are described in the log for the DM. |
| **DM adjudicates** (`manual`) | 247 | The slot is spent, the spell text is shown and the DM resolves the effect (illusions, divinations, summons, utility). |

Fully automated: Acid Splash, Bless, Burning Hands, Chill Touch, Cone of Cold, Cure Wounds, Daylight,
Divine Smite, Eldritch Blast, Fire Bolt, Fireball, Guiding Bolt, Healing Word, Hold Person, Ice Knife,
Inflict Wounds, Light, Lightning Bolt, Mage Armor, Magic Missile, Mass Cure Wounds, Mass Healing Word,
Poison Spray, Ray of Frost, Sacred Flame, Scorching Ray, Shatter, Shield, Shocking Grasp, Starry
Wisp, Thunderwave.

| Mechanic | Status | Notes |
|---|---|---|
| Spell lists per class, cantrips known, prepared spells by level | ✅ | SRD class tables; Prepare tab; host validates. |
| Always-prepared subclass spells (Life Domain, Oath of Devotion), Divine Smite, Hunter's Mark | ✅ | |
| Spell slots (full/half casters), Pact Magic (Warlock), upcasting | ✅ | One slot per turn. |
| Spell save DC, spell attack bonus | ✅ | |
| Cantrip scaling (levels 5/11/17), Eldritch Blast beams, Scorching Ray rays, Magic Missile darts | ✅ | |
| Casting time (Action, Bonus Action, Reaction, 1 minute+ only outside combat) | ✅ | |
| Range & line of sight, Total Cover | ✅ | |
| Concentration & durations (rounds, ticking at the caster's turn) | ✅ | |
| Light-producing spells (Light, Daylight, Dancing Lights is manual) | ✅ | Integration test: a dark cave becomes visible to players. |
| Reaction spells: Shield (on-hit prompt) | ✅ | Counterspell, Feather Fall ⏳ (manual). |
| Components (V/S/M) | 🟡 | Shown; not enforced (no focus/component pouch tracking). |
| Ritual casting | 🟡 | Tagged; no separate ritual flow. |
| Spells not in the SRD (e.g. PHB-only) | 🚫 | Not shipped (legal). Players can ask the DM to adjudicate homebrew. |

## Class features

| Class | Automated | Notes |
|---|---|---|
| Barbarian | Rage (uses, damage bonus, resistances, Str advantage, ends without attacks/damage), Unarmored Defense, Reckless Attack, Danger Sense, Extra Attack, Fast Movement, Frenzy (Berserker), Mindless Rage | Brutal Strike, Relentless Rage ⏳ |
| Bard | Bardic Inspiration (after a failed d20 Test), Jack of All Trades, Expertise, Font of Inspiration, Spellcasting | Cutting Words (Lore) ⏳ |
| Cleric | Spellcasting, Channel Divinity: Divine Spark, Disciple of Life, Potent Cantrip, Life Domain spells | Turn Undead, Preserve Life ⏳ |
| Druid | Spellcasting | Wild Shape ⏳ (DM adjudicates) |
| Fighter | Fighting Style feats, Second Wind, Action Surge (not for Magic), Extra Attack (×2/×3), Improved/Superior Critical (Champion), Weapon Mastery | Indomitable, Tactical Mind ⏳ |
| Monk | Martial Arts (die, bonus strike), Unarmored Defense/Movement, Focus: Flurry of Blows, Patient Defense, Step of the Wind, Stunning Strike, Deflect Attacks | Open Hand techniques ⏳ |
| Paladin | Lay On Hands pool, Divine Smite (free use), Aura of Protection, Fighting Style, Oath of Devotion spells, Extra Attack | Sacred Weapon ⏳ |
| Ranger | Spellcasting, Hunter's Mark always prepared, Fighting Style, Extra Attack | Hunter's Prey ⏳ |
| Rogue | Sneak Attack (once per turn, finesse/ranged, Advantage or ally adjacent), Expertise, Cunning Action, Steady Aim, Uncanny Dodge | Cunning Strike, Thief features ⏳ |
| Sorcerer | Spellcasting, Innate Sorcery, Draconic Resilience | Font of Magic / Metamagic ⏳ |
| Warlock | Pact Magic, Dark One's Blessing (Fiend) | Invocations ⏳ (DM adjudicates) |
| Wizard | Spellcasting, Arcane Recovery | Evoker features ⏳ |

Class feature text for every level comes from the SRD (`content/srd_class_features.json`) and is shown
in the level-up dialog, even when not automated.

## Monsters (SRD 5.2.1 bestiary)

| Item | Status | Notes |
|---|---|---|
| SRD stat blocks | ✅ 328 | Parsed from the SRD by `tools/extract_monsters.py`, validated (AC/HP/CR/abilities). Two entries (Young White Dragon, Giant Centipede) were rejected because the PDF text is malformed; they are not shipped rather than guessed. |
| Weapon attacks, Multiattack count | ✅ | |
| Save-based actions (breath weapons, etc.), recharge | ✅ | 109 monsters have at least one; DM uses them from the action bar. |
| Traits, bonus actions, reactions, legendary actions | 🟡 | Full SRD text in the stat block; resolved by the DM. |
| Damage resistances/immunities/vulnerabilities, condition immunities | ✅ | |
| Monster spellcasting | 🟡 | Text only; DM adjudicates. |
| Custom (homebrew) monsters | ✅ | Editor validates; saved per campaign. |
| Monster Manual (non-SRD) monsters | 🚫 | Not included. |

## Items

| Item | Status |
|---|---|
| Weapons & armor from the SRD equipment tables, equip/unequip outside combat | ✅ |
| Potions of Healing (4 tiers), drink or administer as a Bonus Action within 5 ft | ✅ |
| Light sources (torch, lamp, lantern) | ✅ |
| Containers/loot set by the DM, taken when adjacent and open | ✅ |
| Currency | ✅ (DM sets; no shops) |
| Magic items beyond potions | ⏳ |

## Vision & light

| Mechanic | Status |
|---|---|
| Bright / Dim / Darkness, ambient per room | ✅ |
| Point lights (objects, DM-placed), walls block light | ✅ |
| Darkvision (Dim→Bright, Dark→Dim in gray, range-limited) | ✅ |
| Blindsight, Blinded | ✅ |
| Truesight, magical Darkness, obscurement checks | ⏳ |
| Fog of war (unexplored / explored / visible), shared party exploration | ✅ |
