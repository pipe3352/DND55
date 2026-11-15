#!/usr/bin/env python3
"""Extracts class and subclass feature descriptions ("Level N: Feature") from the column-ordered SRD text.

    python3 tools/extract_class_features.py reference/srd-5.2.1-columns.txt > content/srd_class_features.json
"""
import json, re, sys

CLASSES = ["Barbarian", "Bard", "Cleric", "Druid", "Fighter", "Monk", "Paladin", "Ranger", "Rogue", "Sorcerer", "Warlock", "Wizard"]
SUBCLASS_START = {
    "Barbarian": r"^Path of the Berserker$", "Bard": r"^College of Lore$", "Cleric": r"^Cleric Subclass: Life Domain",
    "Druid": r"^Circle of the Land$", "Fighter": r"^Fighter Subclass: Champion", "Monk": r"^Monk Subclass: Warrior of the",
    "Paladin": r"^Paladin Subclass: Oath of Devotion", "Ranger": r"^Ranger Subclass: Hunter", "Rogue": r"^Rogue Subclass: Thief",
    "Sorcerer": r"^Sorcerer Subclass: Draconic", "Warlock": r"^Warlock Subclass: Fiend Patron", "Wizard": r"^Wizard Subclass: Evoker",
}
SUBCLASS_NAME = {"Barbarian": "Path of the Berserker", "Bard": "College of Lore", "Cleric": "Life Domain", "Druid": "Circle of the Land",
                 "Fighter": "Champion", "Monk": "Warrior of the Open Hand", "Paladin": "Oath of Devotion", "Ranger": "Hunter",
                 "Rogue": "Thief", "Sorcerer": "Draconic Sorcery", "Warlock": "Fiend Patron", "Wizard": "Evoker"}

def main(path):
    lines = [l for l in open(path, encoding="utf-8").read().split("\n")
             if not l.startswith("=== PAGE") and l.strip() != "System Reference Document 5.2.1"]
    starts = {}
    for c in CLASSES:  # class chapter begins at "Core <Class> Traits" (first occurrence after the TOC)
        starts[c] = next(i for i, l in enumerate(lines) if i > 300 and l.strip() == f"Core {c} Traits")
    order = sorted(CLASSES, key=lambda c: starts[c])
    end_all = next(i for i, l in enumerate(lines) if i > starts[order[-1]] and l.strip() == "Character Backgrounds")
    out = {}
    for n, c in enumerate(order):
        lo = starts[c]; hi = starts[order[n + 1]] if n + 1 < len(order) else end_all
        sub_at = next((i for i in range(lo, hi) if re.match(SUBCLASS_START[c], lines[i].strip())), hi)
        feats, cur = [], None
        for i in range(lo, hi):
            m = re.match(r'^Level (\d{1,2}): (.+)$', lines[i].strip())
            if m:
                cur = {"level": int(m.group(1)), "name": m.group(2).strip(),
                       "subclass": SUBCLASS_NAME[c] if i > sub_at else None, "text": []}
                feats.append(cur)
            elif cur is not None:
                if re.match(r'^(.+ Spell List|Spells? by Level|.* Subclass: .*)$', lines[i].strip()):
                    cur = None
                    continue
                cur["text"].append(lines[i])
        for f in feats:
            f["text"] = join(f["text"])[:2500]
        out[c] = feats
        print(c, len(feats), file=sys.stderr)
    json.dump(out, sys.stdout, ensure_ascii=False, indent=1)

def join(ls):
    s = ""
    for l in ls:
        l = l.rstrip()
        if s.endswith("-"):
            s = s[:-1] + l
        else:
            s = (s + " " + l) if s else l
    return s.strip()

if __name__ == "__main__":
    main(sys.argv[1])
