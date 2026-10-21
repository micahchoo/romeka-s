#!/usr/bin/env python3
"""A small Rust source reader: lexical helpers, functions, match arms, call graph.

It understands exactly the constructs the routes, wire and parameter tables use
(blocks, string literals, comments, fields, `fn` bodies, `match` arms) and raises
rather than guessing. Everything else about the crate lives in `model.py`.
"""

from __future__ import annotations

import re

from paths import strip_tests
def skip_string(text: str, i: int) -> int:
    """Index just past the string literal opening at `text[i] == '"'`."""
    j = i - 1
    hashes = 0
    while j >= 0 and text[j] == "#":
        hashes += 1
        j -= 1
    if j >= 0 and text[j] == "r":  # r"..." / r#"..."#
        closing = '"' + "#" * hashes
        end = text.find(closing, i + 1 + hashes)
        return len(text) if end < 0 else end + len(closing)
    i += 1
    while i < len(text):
        if text[i] == "\\":
            i += 2
            continue
        if text[i] == '"':
            return i + 1
        i += 1
    return len(text)


def block_end(text: str, open_index: int) -> int:
    """Index of the `}` that closes the `{` at `open_index`."""
    depth = 0
    i = open_index
    while i < len(text):
        c = text[i]
        if c == '"':
            i = skip_string(text, i)
            continue
        if c == "/" and text.startswith("//", i):
            end = text.find("\n", i)
            i = len(text) if end < 0 else end
            continue
        if c == "/" and text.startswith("/*", i):
            end = text.find("*/", i + 2)
            i = len(text) if end < 0 else end + 2
            continue
        if c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
            if depth == 0:
                return i
        i += 1
    raise ValueError("unbalanced braces")


def paren_end(text: str, open_index: int) -> int:
    depth = 0
    i = open_index
    while i < len(text):
        c = text[i]
        if c == '"':
            i = skip_string(text, i)
            continue
        if c == "(":
            depth += 1
        elif c == ")":
            depth -= 1
            if depth == 0:
                return i
        i += 1
    raise ValueError("unbalanced parentheses")


def doc_above(text: str, index: int) -> str:
    """The `///` lines directly above `index`, joined by spaces.

    An attribute may span several lines (`#[serde(` then one parameter per line),
    and its continuation lines do not start with `#[`. The scan tracks the
    attribute's brackets so a doc comment above a multi-line attribute is still
    read.
    """
    lines = text[:index].splitlines()
    collected: list[str] = []
    attribute_depth = 0
    for line in reversed(lines):
        stripped = line.strip()
        if attribute_depth > 0:
            attribute_depth += stripped.count("[") - stripped.count("]")
            attribute_depth = max(attribute_depth, 0)
            continue
        if stripped.startswith("///"):
            collected.append(stripped[3:].strip())
        elif stripped.startswith("#["):
            attribute_depth = stripped.count("[") - stripped.count("]")
            attribute_depth = max(attribute_depth, 0)
        else:
            break
    return " ".join(reversed(collected))


def split_fields(body: str) -> list[str]:
    """Split a struct body on top-level commas."""
    units: list[str] = []
    depth = 0
    start = 0
    i = 0
    while i < len(body):
        c = body[i]
        if c == '"':
            i = skip_string(body, i)
            continue
        if c in "<([{":
            depth += 1
        elif c in ">)]}":
            # `=>` and `->` are not closers.
            if c == ">" and i > 0 and body[i - 1] in "=-":
                i += 1
                continue
            depth -= 1
        elif c == "," and depth == 0:
            units.append(body[start:i])
            start = i + 1
        i += 1
    units.append(body[start:])
    return [unit for unit in units if unit.strip()]


# --------------------------------------------------------------------------
# Functions, their `params` reads and their callees.
# --------------------------------------------------------------------------

FN_RE = re.compile(r"\bfn\s+([a-z_][A-Za-z0-9_]*)\s*(?:<[^>]*>)?\s*\(([^)]*)\)")
PARAM_RE = re.compile(
    r'params\s*\.\s*(get|int|id|bool|list|rows)\s*(?:::<[^>]*>)?\s*\(\s*"([^"]+)"'
)
CALL_RE = re.compile(r"(?:([A-Za-z_][A-Za-z0-9_]*)\s*::\s*)?([a-z_][A-Za-z0-9_]*)\s*\(")

PARAM_KINDS = {
    "get": "string",
    "int": "integer",
    "id": "table id",
    "bool": "boolean",
    "list": "list",
    "rows": "table rows",
}


class Function:
    def __init__(
        self, name: str, signature: str, body: str, source: str, owner: str | None = None
    ) -> None:
        self.name = name
        self.signature = signature
        self.body = body
        self.source = source
        self.owner = owner

    @property
    def qualified(self) -> str:
        return f"{self.owner}::{self.name}" if self.owner else self.name

    @property
    def source_file(self) -> str:
        return self.source.rsplit("/", 1)[-1]

    @property
    def takes_params(self) -> bool:
        return re.search(r"\bparams\b\s*:", self.signature) is not None

    def param_reads(self) -> list[tuple[str, str]]:
        return [(kind, name) for kind, name in PARAM_RE.findall(self.body)]

    def callees(self) -> list[tuple[str | None, str]]:
        return [(match.group(1), match.group(2)) for match in CALL_RE.finditer(self.body)]


IMPL_RE = re.compile(
    r"(?m)^impl(?:<[^>]*>)?\s+(?:[A-Za-z_:]+(?:<[^>]*>)?\s+for\s+)?([A-Za-z_][A-Za-z0-9_]*)"
)


def impl_ranges(text: str) -> list[tuple[int, int, str]]:
    ranges: list[tuple[int, int, str]] = []
    for match in IMPL_RE.finditer(text):
        brace = text.find("{", match.end())
        if brace < 0:
            continue
        try:
            ranges.append((match.start(), block_end(text, brace), match.group(1)))
        except ValueError:
            continue
    return ranges


def parse_functions(text: str, source: str) -> list[Function]:
    text = strip_tests(text)
    ranges = impl_ranges(text)
    functions: list[Function] = []
    for match in FN_RE.finditer(text):
        brace = text.find("{", match.end())
        if brace < 0:
            continue
        # A `;` before the brace means a trait or extern declaration, not a body.
        if ";" in text[match.end() : brace]:
            continue
        close = block_end(text, brace)
        owner = next(
            (
                name
                for start, end, name in ranges
                if start < match.start() < end
            ),
            None,
        )
        functions.append(
            Function(match.group(1), match.group(2), text[brace + 1 : close], source, owner)
        )
    return functions


def resolve_call(
    caller: Function, qualifier: str | None, name: str, index: dict[str, list[Function]]
) -> list[Function]:
    """The definitions one call may reach. Same file, then same owner, then all."""
    candidates = index.get(name, [])
    if not candidates:
        return []
    if qualifier:
        by_owner = [fn for fn in candidates if fn.owner == qualifier]
        if by_owner:
            return by_owner
    same_file = [fn for fn in candidates if fn.source == caller.source]
    if same_file:
        return same_file
    return candidates


def param_closure(entry: Function, index: dict[str, list[Function]]) -> list[Function]:
    """`entry` plus every function it reaches that also takes `params`."""
    reached = [entry]
    seen = {id(entry)}
    queue = [entry]
    while queue:
        current = queue.pop()
        for qualifier, callee in sorted(current.callees(), key=lambda call: (call[0] or "", call[1])):
            for candidate in resolve_call(current, qualifier, callee, index):
                if id(candidate) in seen or not candidate.takes_params:
                    continue
                seen.add(id(candidate))
                reached.append(candidate)
                queue.append(candidate)
    return reached

def split_arms(interior: str) -> list[str]:
    """Split match arms. A comma separates them, unless it is inside brackets or
    the arm ends in a block, where Rust makes the comma optional."""
    arms: list[str] = []
    buf = ""
    depth = 0
    i = 0
    while i < len(interior):
        c = interior[i]
        if c == '"':
            end = skip_string(interior, i)
            buf += interior[i:end]
            i = end
            continue
        if c in "<([{":
            depth += 1
        elif c in ">)]}":
            if c == ">" and i > 0 and interior[i - 1] in "=-":
                buf += c
                i += 1
                continue
            depth -= 1
            if depth == 0 and c == "}" and "=>" in buf:
                arms.append(buf + c)
                buf = ""
                i += 1
                if i < len(interior) and interior[i] == ",":
                    i += 1
                continue
        elif c == "," and depth == 0:
            arms.append(buf)
            buf = ""
            i += 1
            continue
        buf += c
        i += 1
    if buf.strip():
        arms.append(buf)
    return arms


def match_arms(function: Function) -> list[tuple[str, str]]:
    """Split the first `match` block in a function body into (pattern, value) arms."""
    marker = function.body.find("match ")
    if marker < 0:
        return []
    brace = function.body.find("{", marker)
    if brace < 0:
        return []
    end = block_end(function.body, brace)
    arms: list[tuple[str, str]] = []
    for unit in split_arms(function.body[brace + 1 : end]):
        if "=>" not in unit:
            continue
        pattern, _, condition = unit.partition("=>")
        arms.append((" ".join(pattern.split()), " ".join(condition.split())))
    return arms

