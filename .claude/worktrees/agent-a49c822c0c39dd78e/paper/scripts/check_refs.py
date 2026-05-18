#!/usr/bin/env python3
"""Lightweight BibTeX sanity check for paper/refs/refs.bib.

Validates:
  * each entry has a unique citation key
  * each entry has title + year
  * each entry has at least one of: journal, booktitle, howpublished, url, institution, publisher
  * total entry count is >= 30
"""
from __future__ import annotations

import re
import sys
from pathlib import Path


ENTRY_RE = re.compile(r"^@(\w+)\s*\{\s*([^,\s]+)\s*,", re.MULTILINE)
FIELD_RE = re.compile(r"^\s*(\w+)\s*=\s*", re.MULTILINE)


def parse_entries(text: str) -> list[tuple[str, str, dict[str, str]]]:
    entries: list[tuple[str, str, dict[str, str]]] = []
    for match in ENTRY_RE.finditer(text):
        kind = match.group(1).lower()
        if kind in {"comment", "preamble", "string"}:
            continue
        key = match.group(2)
        # find matching closing brace by walking the buffer
        start = match.end()
        depth = 1
        i = start
        while i < len(text) and depth > 0:
            ch = text[i]
            if ch == "{":
                depth += 1
            elif ch == "}":
                depth -= 1
            i += 1
        body = text[start : i - 1]
        # capture field names (values can contain braces; we only need names + a coarse value)
        fields: dict[str, str] = {}
        for fmatch in FIELD_RE.finditer(body):
            name = fmatch.group(1).lower()
            # grab a coarse value (until next ",\n  word =" or end)
            tail = body[fmatch.end():]
            depth_v = 0
            j = 0
            while j < len(tail):
                c = tail[j]
                if c == "{":
                    depth_v += 1
                elif c == "}":
                    depth_v -= 1
                elif c == "," and depth_v == 0:
                    break
                j += 1
            fields[name] = tail[:j].strip()
        entries.append((kind, key, fields))
    return entries


def main() -> int:
    repo = Path(__file__).resolve().parents[2]
    bib = repo / "paper" / "refs" / "refs.bib"
    if not bib.exists():
        print(f"ERROR: {bib} not found", file=sys.stderr)
        return 2
    text = bib.read_text(encoding="utf-8")
    entries = parse_entries(text)

    keys: list[str] = []
    failures: list[str] = []
    for kind, key, fields in entries:
        keys.append(key)
        if "title" not in fields or not fields["title"].strip():
            failures.append(f"{key}: missing title")
        if "year" not in fields or not fields["year"].strip():
            failures.append(f"{key}: missing year")
        venue_keys = {
            "journal",
            "booktitle",
            "howpublished",
            "url",
            "institution",
            "publisher",
            "organization",
            "note",
        }
        if not (venue_keys & fields.keys()):
            failures.append(f"{key}: missing venue/url/publisher")

    duplicates = [k for k in set(keys) if keys.count(k) > 1]
    if duplicates:
        failures.append(f"duplicate keys: {sorted(duplicates)}")

    print(f"entry_count={len(entries)}")
    print("keys:")
    for k in keys:
        print(f"  - {k}")

    if len(entries) < 30:
        failures.append(f"only {len(entries)} entries (need >= 30)")

    if failures:
        print("\nFAILURES:")
        for f in failures:
            print(f"  * {f}")
        return 1
    print("\nOK: bibliography passes structural checks")
    return 0


if __name__ == "__main__":
    sys.exit(main())
