#!/usr/bin/env python3
"""Rebuilds reading order of the two-column SRD 5.2.1 PDF from `pdftotext -bbox-layout` output.

Usage: pdftotext -bbox-layout SRD_CC_v5.2.1.pdf bbox.html && python3 tools/srd_columns.py bbox.html > srd-columns.txt
Each page is emitted as '=== PAGE n ===' followed by its blocks: full-width blocks and the left
column first, then the right column, each top-to-bottom. Page headers/footers are dropped.
"""
import re, sys, html

def main(path):
    data = open(path, encoding="utf-8").read()
    pages = re.findall(r'<page width="([\d.]+)" height="([\d.]+)">(.*?)</page>', data, re.S)
    out = []
    for n, (pw, ph, body) in enumerate(pages, 1):
        pw, ph = float(pw), float(ph)
        blocks = []
        for b in re.finditer(r'<block xMin="([\d.]+)" yMin="([\d.]+)" xMax="([\d.]+)" yMax="([\d.]+)">(.*?)</block>', body, re.S):
            x0, y0, x1, y1 = map(float, b.groups()[:4])
            lines = []
            for l in re.finditer(r'<line[^>]*>(.*?)</line>', b.group(5), re.S):
                words = [html.unescape(w) for w in re.findall(r'<word[^>]*>(.*?)</word>', l.group(1), re.S)]
                if words:
                    lines.append(" ".join(words))
            text = "\n".join(lines).strip()
            if not text:
                continue
            if y0 < ph * 0.045 or y1 > ph * 0.955:   # running header / page number
                if len(text) < 60:
                    continue
            mid = pw / 2
            col = 0 if x1 <= mid + 8 else (1 if x0 >= mid - 8 else -1)  # -1 = spans both columns
            blocks.append((col, y0, x0, text))
        # Full-width blocks split the page into bands; within each band read left column then right.
        blocks.sort(key=lambda b: b[1])
        bands, cur = [], []
        for b in blocks:
            if b[0] == -1:
                if cur:
                    bands.append(cur); cur = []
                bands.append([b])
            else:
                cur.append(b)
        if cur:
            bands.append(cur)
        out.append(f"=== PAGE {n} ===")
        for band in bands:
            for b in sorted(band, key=lambda b: (max(b[0], 0), b[1], b[2])):
                out.append(b[3])
    sys.stdout.write("\n".join(out) + "\n")

if __name__ == "__main__":
    main(sys.argv[1])
