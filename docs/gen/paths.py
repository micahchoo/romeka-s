#!/usr/bin/env python3
"""Where the generator reads from, and the two text helpers every module shares."""

from __future__ import annotations

from pathlib import Path

HERE = Path(__file__).resolve().parent
DOCS = HERE.parent
REPO = DOCS.parent
SRC = REPO / "crates" / "omeka-api" / "src"


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


CFG_TEST = "#[cfg(test)]"


def strip_tests(text: str) -> str:
    """Drop every `#[cfg(test)]` item.

    A test module trails, so its body is everything from its `{` to the end of
    the file. `read/mod.rs` also carries a `#[cfg(test)] mod probe;` declaration
    among the module list, and that one is a single statement: the code after it
    has to survive, or the generator finds no dispatch functions at all.
    """
    while True:
        cut = text.find(CFG_TEST)
        if cut < 0:
            return text
        start = cut + len(CFG_TEST)
        semi = text.find(";", start)
        brace = text.find("{", start)
        if 0 <= semi and (brace < 0 or semi < brace):
            text = text[:cut] + text[semi + 1 :]
        else:
            return text[:cut]
