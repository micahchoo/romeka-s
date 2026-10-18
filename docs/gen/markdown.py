#!/usr/bin/env python3
"""Markdown table and inline-code helpers."""

from __future__ import annotations
def table(headers: list[str], rows: list[list[str]]) -> str:
    out = ["| " + " | ".join(headers) + " |", "| " + " | ".join("---" for _ in headers) + " |"]
    for row in rows:
        cells = [cell.replace("\n", " ").replace("|", "\\|").strip() or "" for cell in row]
        out.append("| " + " | ".join(cells) + " |")
    return "\n".join(out)


def code(text: str) -> str:
    return "`" + text.replace("`", "'") + "`"

