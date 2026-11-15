#!/usr/bin/env python3
"""Extracts the 20-level class feature tables from the layout-mode SRD text.

    python3 tools/extract_class_tables.py reference/srd-5.2.1-layout.txt > content/srd_class_tables.json

Output per class: list of 20 rows {level, pb, features, columns: [...]} where `columns` are the
class-specific numeric columns in table order (e.g. Wizard: cantrips, prepared, slots 1..9).
"""
import json, re, sys

CLASSES = ["Barbarian", "Bard", "Cleric", "Druid", "Fighter", "Monk", "Paladin", "Ranger", "Rogue", "Sorcerer", "Warlock", "Wizard"]
CELL = r'(?:\d+d\d+|D\d+|\+\d+|\d+|—|Unlimited|\d+ ft\.)'

def parse(lines, cls):
    start = next(i for i, l in enumerate(lines) if re.search(rf'\b{cls} Features\b', l))
    rows = {}
    for l in lines[start:start + 220]:
        m = re.match(r'^\s{2,}(\d{1,2})\s+\+(\d)\s+(.*)$', l)
        if not m or int(m.group(1)) in rows or not 1 <= int(m.group(1)) <= 20:
            continue
        level, pb, rest = int(m.group(1)), int(m.group(2)), m.group(3)
        # Only the table region of the row (left part of the page when two columns are printed side by side).
        parts = re.split(r'\s{2,}', rest.strip())
        feats, cols = [], []
        for p in parts:
            toks = p.split()
            if all(re.fullmatch(CELL, t) for t in toks) and (cols or feats):
                cols.extend(toks)
            elif not cols:
                feats.append(p)
            else:
                break  # text from the neighbouring column
        rows[level] = {"level": level, "pb": pb, "features": " ".join(feats).rstrip(","), "columns": cols}
        if len(rows) == 20:
            break
    return [rows[k] for k in sorted(rows)]

def main(path):
    lines = open(path, encoding="utf-8").read().split("\n")
    out = {c: parse(lines, c) for c in CLASSES}
    json.dump(out, sys.stdout, ensure_ascii=False, indent=1)
    for c, rows in out.items():
        print(c, len(rows), [len(r["columns"]) for r in rows], file=sys.stderr)

if __name__ == "__main__":
    main(sys.argv[1])
