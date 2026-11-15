# Reference: SRD 5.2.1

`srd-5.2.1.txt` is a plain-text extraction (`pdftotext`) of the official System Reference Document
5.2.1 PDF (https://www.dndbeyond.com/srd), licensed under CC-BY-4.0 (see `docs/LEGAL.md`).

Use it to check every rule implemented in `crates/dnd_rules`, e.g.:

```bash
grep -n -A12 "^Exhaustion \[Condition\]" reference/srd-5.2.1.txt
```

Two-column layout makes some passages interleave. When a rule looks ambiguous, check the PDF.
