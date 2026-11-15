#!/usr/bin/env python3
"""Adds an "auto" block to each spell in content/srd_spells.json, derived from the SRD text.

    python3 tools/derive_spell_automation.py content/srd_spells.json

Levels: "full"  = curated spells whose every rules effect the engine implements (each has a Rust test);
        "partial" = damage/healing/save core automated, remaining text is DM-adjudicated (logged as such);
        "manual"  = slot, concentration, range and description handled; effect adjudicated by the DM.
"""
import json, re, sys

DMG = "Acid|Bludgeoning|Cold|Fire|Force|Lightning|Necrotic|Piercing|Poison|Psychic|Radiant|Slashing|Thunder"
COND = "Blinded|Charmed|Deafened|Frightened|Grappled|Incapacitated|Invisible|Paralyzed|Petrified|Poisoned|Prone|Restrained|Stunned|Unconscious"
ABIL = "Strength|Dexterity|Constitution|Intelligence|Wisdom|Charisma"
FULL = {"acid_splash", "fire_bolt", "ray_of_frost", "sacred_flame", "eldritch_blast", "shocking_grasp", "light", "burning_hands",
        "cure_wounds", "healing_word", "magic_missile", "guiding_bolt", "inflict_wounds", "thunderwave", "bless", "shield",
        "mage_armor", "hold_person", "scorching_ray", "shatter", "fireball", "lightning_bolt", "mass_healing_word",
        "divine_smite", "daylight", "chill_touch", "poison_spray", "starry_wisp", "ice_knife", "cone_of_cold", "mass_cure_wounds"}

def derive(sp):
    d = sp["description"]
    main, _, upcast_txt = d.partition("Using a Higher-Level Spell Slot.")
    main, _, cantrip_txt = main.partition("Cantrip Upgrade.")
    a = {"level": "manual"}
    atk = re.search(r'[Mm]ake an? (melee|ranged) spell attack', main)
    if atk:
        a["attack"] = atk.group(1)
    sv = re.search(rf'(?:makes?|succeed on|must succeed on) an? ({ABIL}) saving throw', main)
    if sv:
        a["save"] = sv.group(1)
    dm = re.search(rf'(\d+d\d+(?: \+ \d+)?) ({DMG}) damage', main)
    if dm:
        a["damage"] = {"dice": dm.group(1).replace(" ", ""), "type": dm.group(2)}
    a["half_on_success"] = bool(re.search(r'half as much damage', main))
    heal = re.search(r'regains? (?:a number of )?Hit Points equal to (\d+d\d+) plus your spellcasting ability modifier', main)
    if heal:
        a["healing"] = {"dice": heal.group(1), "add_mod": True}
    area = re.search(r'(\d+)-foot(?:-radius|-long)?,? ?(?:\d+-foot-wide )?(Sphere|Cube|Cone|Line|Emanation|Cylinder)', main)
    if area:
        a["area"] = {"size_ft": int(area.group(1)), "shape": area.group(2)}
    cond = re.search(rf'(?:or have|has|have) the ({COND}) condition', main)
    if cond and "save" in a:
        a["condition"] = cond.group(1)
        a["repeat_save"] = bool(re.search(r'[Aa]t the end of each of its turns, the target repeats the save', main))
    up = re.search(r'(?:damage|healing) increases by (\d+d\d+) for each spell slot level above (\d)', upcast_txt)
    if up:
        a["upcast_dice"] = up.group(1)
    tgt = re.search(r'target one additional (?:creature|Humanoid) for each spell slot level above', upcast_txt)
    if tgt:
        a["upcast_targets"] = 1
    cu = re.search(r'damage increases by (\d+d\d+) when you reach levels 5', cantrip_txt)
    if cu:
        a["cantrip_scaling"] = cu.group(1)
    lt = re.search(r'sheds Bright Light in a (\d+)-foot radius and Dim Light for an additional (\d+) feet', main)
    if lt:
        a["light"] = {"bright_ft": int(lt.group(1)), "dim_ft": int(lt.group(2))}
    # Curated special cases (verified against the SRD text in tests).
    sid = sp["id"]
    if sid == "magic_missile":
        a.update({"darts": 3, "damage": {"dice": "1d4+1", "type": "Force"}, "auto_hit": True})
    if sid == "scorching_ray":
        a.update({"rays": 3})
    if sid == "eldritch_blast":
        a.update({"beams_by_level": [[1, 1], [5, 2], [11, 3], [17, 4]]})
    if sid == "bless":
        a.update({"buff": "bless", "targets": 3})
    if sid == "shield":
        a.update({"reaction": "shield"})
    if sid == "mage_armor":
        a.update({"buff": "mage_armor"})
    if sid == "hold_person":
        a.update({"humanoid_only": True, "targets": 1})
    if sid == "divine_smite":
        a.update({"smite": True, "extra_vs": ["Fiend", "Undead"], "extra_dice": "1d8"})
    if sid in ("mass_healing_word", "mass_cure_wounds"):
        a.update({"targets": 6})
    if sid == "guiding_bolt":
        a.update({"grants_advantage_next_attack": True})
    if sid == "ray_of_frost":
        a.update({"speed_reduction_ft": 10})
    if sid == "shocking_grasp":
        a.update({"no_reactions": True})
    if sid == "chill_touch":
        a.update({"no_healing": True})
    if sid == "thunderwave":
        a.update({"push_ft": 10})
    if sid == "daylight":
        a.update({"light": {"bright_ft": 60, "dim_ft": 60}})
    if sid == "ice_knife":
        a.pop("save", None)
        a.update({"attack": "ranged", "damage": {"dice": "1d10", "type": "Piercing"}, "secondary": {"save": "Dexterity", "area": {"size_ft": 5, "shape": "Sphere"}, "damage": {"dice": "2d6", "type": "Cold"}}, "upcast_dice": "1d6"})
    if sid in FULL:
        a["level"] = "full"
    elif ("damage" in a and ("attack" in a or "save" in a)) or "healing" in a or "light" in a:
        a["level"] = "partial"
    return a

def main(path):
    spells = json.load(open(path, encoding="utf-8"))
    for sp in spells:
        sp["auto"] = derive(sp)
    json.dump(spells, open(path, "w", encoding="utf-8"), ensure_ascii=False, indent=1)
    from collections import Counter
    print(Counter(s["auto"]["level"] for s in spells), file=sys.stderr)
    missing = FULL - {s["id"] for s in spells}
    assert not missing, missing

if __name__ == "__main__":
    main(sys.argv[1])
