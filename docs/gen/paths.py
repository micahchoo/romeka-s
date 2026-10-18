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


def strip_tests(text: str) -> str:
    """Drop everything from the first `#[cfg(test)]` on. Test modules trail."""
    cut = text.find("#[cfg(test)]")
    return text[:cut] if cut >= 0 else text
