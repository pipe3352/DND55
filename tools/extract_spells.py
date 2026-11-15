#!/usr/bin/env python3
"""Extracts every spell from the column-ordered SRD 5.2.1 text into content/srd_spells.json.

    python3 tools/extract_spells.py reference/srd-5.2.1-columns.txt > content/srd_spells.json

Only text present in the SRD is copied. Structured fields are parsed from the spell header lines;
automation hints are derived by tools-side patterns and verified by tests for the curated spells.
"""
import json, re, sys

LEVEL_RE = re.compile(r'^(?:Level (\d) (\w+)|(\w+) Cantrip) \((.*)$')

def main(path):
    lines = open(path, encoding="utf-8").read().split("\n")
    start = next(i for i, l in enumerate(lines) if l.strip() == "Spell Descriptions")
    # The chapter ends at the last "Zone of Truth" entry's description; we stop at the Rules Glossary page.
    lines = [l for l in lines[start + 1:] if not l.startswith("=== PAGE") and l.strip() != "System Reference Document 5.2.1"]
    spells, i = [], 0
    while i < len(lines) - 2:
        name, head = lines[i].strip(), lines[i + 1].strip()
        m = LEVEL_RE.match(head)
        if m and name and len(name) < 50 and not name.endswith("."):
            classes_txt, j = m.group(4), i + 2
            while not classes_txt.rstrip().endswith(")"):
                classes_txt += " " + lines[j].strip(); j += 1
            fields, key = {}, None
            while j < len(lines):
                l = lines[j].strip()
                fm = re.match(r'^(Casting Time|Range|Components?|Duration): (.*)$', l)
                if fm:
                    key = 'Components' if fm.group(1) == 'Component' else fm.group(1); fields[key] = fm.group(2); j += 1
                    if key == "Duration":
                        break
                elif key:
                    fields[key] += " " + l; j += 1
                else:
                    break
            if "Duration" not in fields:
                i += 1; continue
            # Description: until the next "<Name>\n<Level line>".
            k, desc = j, []
            while k < len(lines) - 1:
                if LEVEL_RE.match(lines[k + 1].strip()) and lines[k].strip() and len(lines[k].strip()) < 50:
                    break
                if lines[k].strip() in ("Rules Glossary",):
                    break
                desc.append(lines[k]); k += 1
            text = join_lines(desc)
            level = int(m.group(1)) if m.group(1) else 0
            school = m.group(2) or m.group(3)
            classes = [c.strip() for c in classes_txt.rstrip(")").split(",")]
            spells.append(build(name, level, school, classes, fields, text))
            i = k
        else:
            i += 1
    json.dump(spells, sys.stdout, ensure_ascii=False, indent=1)
    print(f"{len(spells)} spells", file=sys.stderr)

def join_lines(ls):
    out = ""
    for l in ls:
        l = l.rstrip()
        if out.endswith("-") and not out.endswith(" -"):
            out = out[:-1] + l          # de-hyphenate line breaks ("Constitu-\ntion")
        elif out:
            out += ("\n" if re.match(r'^[A-Z][\w’\' -]{1,40}\. ', l) and out.endswith(".") else " ") + l
        else:
            out = l
    return out.strip()

def build(name, level, school, classes, f, text):
    name = re.sub(r"\b([A-Z]) ([a-z])", r"\1\2", name)  # PDF drop-cap artifacts: "A cid S plash"
    f = {k: re.sub(r'(\w)- (\w)', r'\1\2', v) for k, v in f.items()}  # line-break hyphens in header fields
    comps = f["Components"]
    ct = f["Casting Time"]
    dur = f["Duration"]
    spell = {
        "id": re.sub(r'[^a-z0-9]+', '_', name.lower()).strip("_"),
        "name": name, "level": level, "school": school, "classes": classes,
        "casting_time": ct, "range": f["Range"], "components": comps, "duration": dur,
        "ritual": "Ritual" in ct, "concentration": dur.startswith("Concentration"),
        "verbal": bool(re.match(r'^V\b', comps)), "somatic": bool(re.search(r'(^|, )S\b', comps)),
        "material": re.search(r'M \((.*)\)', comps).group(1) if "M (" in comps else None,
        "description": text,
    }
    return spell

if __name__ == "__main__":
    main(sys.argv[1])
