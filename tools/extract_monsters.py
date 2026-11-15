#!/usr/bin/env python3
"""Extracts SRD 5.2.1 monster stat blocks into content/srd_monsters.json.

    python3 tools/extract_monsters.py reference/srd-5.2.1-columns.txt > content/srd_monsters.json

Validation: a block is kept only if HP equals the average of its hit dice, all six ability scores
parse, and CR/XP parse. Rejected names are reported on stderr. Nothing is invented: unparsed parts
stay as text the DM can read.
"""
import json, re, sys

SIZES = r'(?:Tiny|Small|Medium|Large|Huge|Gargantuan)'
TYPE_RE = re.compile(rf'^({SIZES}(?: or {SIZES})?) (Aberration|Beast|Celestial|Construct|Dragon|Elemental|Fey|Fiend|Giant|Humanoid|Monstrosity|Ooze|Plant|Undead|Swarm of Tiny \w+)(?: \(([^)]*)\))?, (.+)$')
ABIL = {"Str": "str", "Dex": "dex", "Con": "con", "Int": "int", "Wis": "wis", "Cha": "cha"}
SECTIONS = ["Traits", "Actions", "Bonus Actions", "Reactions", "Legendary Actions"]
DMG_TYPES = "Acid|Bludgeoning|Cold|Fire|Force|Lightning|Necrotic|Piercing|Poison|Psychic|Radiant|Slashing|Thunder"
CONDITIONS = "Blinded|Charmed|Deafened|Exhaustion|Frightened|Grappled|Incapacitated|Invisible|Paralyzed|Petrified|Poisoned|Prone|Restrained|Stunned|Unconscious"

def dice_avg(expr):
    m = re.fullmatch(r'(\d+)d(\d+)(?:\s*([+−-])\s*(\d+))?', expr.strip())
    if not m:
        return None
    n, s = int(m.group(1)), int(m.group(2))
    mod = int(m.group(4) or 0) * (-1 if m.group(3) in ("−", "-") else 1)
    return (n * (s + 1)) // 2 + mod if (n * (s + 1)) % 2 == 0 else int(n * (s + 1) / 2 + mod)

def clean(s):
    return s.replace("−", "-").replace("–", "-")

def parse_entries(text):
    """Split 'Name. text Name2. text' paragraphs (each entry starts on its own line)."""
    entries, cur = [], None
    for l in text:
        m = re.match(r'^([A-Z][\w’\'\- ,/]*?(?: \([^)]*\))?)\. (.*)$', l)
        small = {"of", "the", "and", "or", "a", "an", "in", "on", "to", "with", "from", "per", "vs"}
        title = m and all(w[0].isupper() or w[0] in "(0123456789" or w in small for w in re.sub(r'\([^)]*\)', '', m.group(1)).split())
        if m and title and len(m.group(1)) < 60 and (cur is None or cur["text"].endswith((".", ")"))):
            cur = {"name": m.group(1), "text": m.group(2)}
            entries.append(cur)
        elif cur:
            cur["text"] = cur["text"][:-1] + l if cur["text"].endswith("-") and not cur["text"].endswith(" -") else cur["text"] + " " + l
    return entries

def parse_attack(e):
    t = clean(e["text"])
    m = re.match(r'^(Melee|Ranged|Melee or Ranged) Attack Roll: ([+-]\d+), (?:reach (\d+) ft\.)?(?: or )?(?:range (\d+)/(\d+) ft\.)?.*?Hit: (.*)$', t)
    if not m:
        return None
    hit = m.group(6)
    parts = re.findall(rf'(\d+) \((\d+d\d+(?: [+-] \d+)?)\) ({DMG_TYPES}) damage', hit)
    flat = re.match(rf'^(\d+) ({DMG_TYPES}) damage', hit)
    if not parts and not flat:
        return None
    adv_extra = []
    m2 = re.search(rf'plus (\d+) \((\d+d\d+)\) ({DMG_TYPES}) damage if the attack roll had Advantage', hit)
    if m2:
        adv_extra = [{"dice": m2.group(2), "type": m2.group(3)}]
        parts = [p for p in parts if not (p[1] == m2.group(2) and p[2] == m2.group(3))]
    damage = [{"dice": p[1].replace(" ", ""), "type": p[2]} for p in parts] or [{"dice": flat.group(1), "type": flat.group(2)}]
    return {
        "name": e["name"], "kind": "ranged" if m.group(1) == "Ranged" else "melee", "to_hit": int(m.group(2)),
        "reach_ft": int(m.group(3)) if m.group(3) else (5 if m.group(1) != "Ranged" else 0),
        "range_ft": [int(m.group(4)), int(m.group(5))] if m.group(4) else None,
        "damage": damage, "extra_with_advantage": adv_extra,
        "thrown": m.group(1) == "Melee or Ranged",
    }

def parse_save_action(e):
    t = clean(e["text"])
    m = re.match(r'^(Strength|Dexterity|Constitution|Intelligence|Wisdom|Charisma) Saving Throw: DC (\d+), (.*?)\. Failure: (.*?)(?: Success: (.*?))?$', t)
    if not m:
        return None
    dmg = re.findall(rf'(\d+) \((\d+d\d+(?: [+-] \d+)?)\) ({DMG_TYPES}) damage', m.group(4))
    area = re.search(r'(\d+)-foot(?:-radius)? (Cone|Cube|Line|Sphere|Emanation|Cylinder)', m.group(3))
    return {"name": e["name"], "ability": m.group(1), "dc": int(m.group(2)), "targets": m.group(3),
            "damage": [{"dice": d[1].replace(" ", ""), "type": d[2]} for d in dmg],
            "half_on_success": bool(m.group(5) and "Half damage" in m.group(5)),
            "area": {"size_ft": int(area.group(1)), "shape": area.group(2)} if area else None,
            "conditions": [c for c in re.findall(CONDITIONS, m.group(4)) if c != "Exhaustion"]}

def main(path):
    raw = open(path, encoding="utf-8").read().split("\n")
    lines = [l.rstrip() for l in raw if not l.startswith("=== PAGE") and l.strip() != "System Reference Document 5.2.1"]
    first = next(i for i, l in enumerate(lines) if i > 30000 and l.strip() == "Aboleth")
    starts = [i for i in range(first, len(lines) - 2) if TYPE_RE.match(lines[i + 1].strip()) and lines[i + 2].startswith("AC ")]
    monsters, rejected = [], []
    for n, s in enumerate(starts):
        end = starts[n + 1] if n + 1 < len(starts) else len(lines)
        block = lines[s:end]
        name = re.sub(r"\b([A-Z]) ([a-z])", r"\1\2", block[0].strip())
        tm = TYPE_RE.match(block[1].strip())
        mon = {"id": re.sub(r'[^a-z0-9]+', '_', name.lower()).strip('_'), "name": name, "size": tm.group(1), "type": tm.group(2),
               "tags": tm.group(3), "alignment": tm.group(4), "source": "srd-5.2.1"}
        body, sections, cur = [], {k: [] for k in SECTIONS}, None
        for l in block[2:]:
            st = l.strip()
            if st in SECTIONS:
                cur = st; continue
            if cur:
                # Lore/headers between blocks (e.g. family intros) end the block's sections.
                sections[cur].append(st)
            else:
                body.append(st)
        txt = "\n".join(body)
        def field(rx, default=None):
            m = re.search(rx, txt, re.M)
            return m.group(1).strip() if m else default
        try:
            mon["ac"] = int(field(r'^AC (\d+)'))
            hp = re.search(r'^HP (\d+) \(([^)]*)\)', txt, re.M)
            mon["hp"], mon["hit_dice"] = int(hp.group(1)), clean(hp.group(2)).replace(" ", "")
            mon["speed"] = field(r'^Speed (.*)$')
            init = re.search(r'Initiative ([+−-]\d+)', txt)
            mon["initiative"] = int(clean(init.group(1)))
            scores = {}
            for m in re.finditer(r'(?:^|\s)([SIDWC]) ?(tr|nt|ex|is|on|ha) (\d+) ([+−-]\d+) ([+−-]\d+)', txt, re.M):
                key = ABIL[m.group(1) + m.group(2)]
                scores[key] = {"score": int(m.group(3)), "mod": int(clean(m.group(4))), "save": int(clean(m.group(5)))}
            mon["abilities"] = scores
            cr = re.search(r'^CR ([\d/]+) \((?:XP )?([\d,]+)(?: XP)?(?:[^;]*)?; PB \+(\d+)\)', txt, re.M)
            mon["cr"], mon["xp"], mon["pb"] = cr.group(1), int(cr.group(2).replace(",", "")), int(cr.group(3))
        except (AttributeError, TypeError, ValueError) as ex:
            rejected.append(f"{name} [{ex}]"); continue
        for key, rx in [("skills", r'^Skills (.*)$'), ("resistances", r'^Resistances (.*)$'), ("vulnerabilities", r'^Vulnerabilities (.*)$'),
                        ("immunities", r'^Immunities (.*)$'), ("senses", r'^Senses (.*)$'), ("languages", r'^Languages (.*)$'), ("gear", r'^Gear (.*)$')]:
            mon[key] = field(rx)
        imm = mon["immunities"] or ""
        dmg_part, _, cond_part = imm.partition(";")
        if not re.search(DMG_TYPES, dmg_part):
            dmg_part, cond_part = "", imm
        mon["damage_immunities"] = re.findall(DMG_TYPES, dmg_part)
        conds = re.findall(CONDITIONS, cond_part)
        mon["exhaustion_immune"] = "Exhaustion" in conds
        mon["condition_immunities"] = [c for c in conds if c != "Exhaustion"]
        mon["damage_resistances"] = re.findall(DMG_TYPES, mon["resistances"] or "")
        mon["damage_vulnerabilities"] = re.findall(DMG_TYPES, mon["vulnerabilities"] or "")
        sen = mon["senses"] or ""
        for s_ in ["Darkvision", "Blindsight", "Truesight", "Tremorsense"]:
            m = re.search(rf'{s_} (\d+) ft', sen)
            mon[s_.lower() + "_ft"] = int(m.group(1)) if m else 0
        pp = re.search(r'Passive Perception (\d+)', sen)
        mon["passive_perception"] = int(pp.group(1)) if pp else 10
        sp = mon["speed"] or ""
        walk = re.match(r'^(\d+) ft', sp)
        mon["speed_ft"] = int(walk.group(1)) if walk else 0
        if not any(sections.values()):
            # Some blocks omit the "Actions" header: entries after the CR line are actions.
            cr_idx = next((i for i, l in enumerate(body) if l.startswith("CR ")), len(body))
            sections["Actions"] = body[cr_idx + 1:]
        for sec in SECTIONS:
            mon[sec.lower().replace(" ", "_")] = parse_entries(sections[sec])
        mon["attacks"] = [a for a in (parse_attack(e) for e in mon["actions"]) if a]
        mon["save_actions"] = [a for a in (parse_save_action(e) for e in mon["actions"] + mon["bonus_actions"]) if a]
        multi = next((e for e in mon["actions"] if e["name"] == "Multiattack"), None)
        words = {"two": 2, "three": 3, "four": 4, "five": 5, "six": 6}
        mon["attacks_per_action"] = 1
        if multi:
            m = re.search(r'makes (two|three|four|five|six)', multi["text"])
            if m:
                mon["attacks_per_action"] = words[m.group(1)]
        # Validation.
        avg = dice_avg(mon["hit_dice"])
        if len(scores) != 6 or avg is None or avg != mon["hp"]:
            rejected.append(f"{name} (hp {mon['hp']} vs dice {mon['hit_dice']}={avg}, scores {len(scores)})"); continue
        monsters.append(mon)
    json.dump(monsters, sys.stdout, ensure_ascii=False, indent=1)
    print(f"{len(monsters)} monsters kept, {len(rejected)} rejected: {rejected}", file=sys.stderr)

if __name__ == "__main__":
    main(sys.argv[1])
