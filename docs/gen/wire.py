#!/usr/bin/env python3
"""Parse one `crates/omeka-api/src/wire/*.rs` module into its documents.

A `WireItem` is one `pub struct`, `pub enum`, `pub type` or `wire_const_str!`
constant, with its members and their serde renames, optionality and flattening.
"""

from __future__ import annotations

import re
from pathlib import Path

from paths import read, SRC
from russparse import block_end, doc_above, paren_end, split_fields, strip_tests
def serde_attrs(unit: str) -> dict[str, str]:
    attrs: dict[str, str] = {}
    renamed = re.search(r'rename\s*=\s*"([^"]*)"', unit)
    if renamed:
        attrs["rename"] = renamed.group(1)
    skipped = re.search(r'skip_serializing_if\s*=\s*"([^"]*)"', unit)
    if skipped:
        attrs["skip_serializing_if"] = skipped.group(1)
    if re.search(r"#\[serde\([^)]*\bflatten\b", unit, re.S):
        attrs["flatten"] = "yes"
    return attrs


class Member:
    def __init__(self, name: str, type_: str, attrs: dict[str, str], doc: str) -> None:
        self.name = name
        self.type = type_
        self.attrs = attrs
        self.doc = doc

    @property
    def key(self) -> str:
        if "flatten" in self.attrs:
            return "(flattened)"
        return self.attrs.get("rename", self.name)

    @property
    def optional(self) -> bool:
        return self.type.startswith("Option<")

    @property
    def omitted_when_unset(self) -> bool:
        return "skip_serializing_if" in self.attrs

    def note(self) -> str:
        bits = []
        if self.type.startswith("Option<") and not self.omitted_when_unset:
            bits.append("null when unset")
        if self.omitted_when_unset:
            bits.append("omitted when unset")
        if "flatten" in self.attrs:
            bits.append("flattens the value map into this object")
        if self.doc:
            bits.append(self.doc)
        return "; ".join(bits)


class WireItem:
    def __init__(self, kind: str, name: str, doc: str, source: str) -> None:
        self.kind = kind
        self.name = name
        self.doc = doc
        self.source = source
        self.members: list[Member] = []
        self.variants: list[tuple[str, str, str]] = []
        self.inner: str | None = None


ITEM_RE = re.compile(r"(?m)^pub (struct|enum|type) ([A-Za-z_][A-Za-z0-9_]*)")


def parse_wire(text: str, source: str) -> tuple[list[WireItem], str | None]:
    """The structs, enums and type aliases of one wire module, in file order."""
    text = strip_tests(text)
    resource = re.search(r'pub const RESOURCE_NAME: &str = "([^"]+)"', text)
    items: list[WireItem] = []
    for match in ITEM_RE.finditer(text):
        kind, name = match.group(1), match.group(2)
        item = WireItem(kind, name, doc_above(text, match.start()), source)
        rest = text[match.end() :]
        if kind == "struct":
            if rest.lstrip().startswith("("):
                open_index = text.index("(", match.end())
                close = paren_end(text, open_index)
                item.inner = text[open_index + 1 : close].strip()
            else:
                open_index = text.index("{", match.end())
                close = block_end(text, open_index)
                item.members = parse_members(text[open_index + 1 : close])
        elif kind == "enum":
            open_index = text.index("{", match.end())
            close = block_end(text, open_index)
            item.variants = parse_variants(text[open_index + 1 : close])
        elif kind == "type":
            item.inner = rest.split(";")[0].strip()
        items.append(item)

    for const_match in re.finditer(r"wire_const_str!\(\s*(.*?)\s*\);", text, re.S):
        body = const_match.group(1)
        name_match = re.search(r"([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.+)", body, re.S)
        if not name_match:
            continue
        consts = dict(re.findall(r'pub const (\w+): &str = "([^"]*)"', text))
        value = name_match.group(2).strip().rstrip(",").strip()
        item = WireItem(
            "const-struct", name_match.group(1), doc_above(text, const_match.start()), source
        )
        item.inner = consts.get(value, value.strip('"'))
        items.append(item)

    return items, resource.group(1) if resource else None


def parse_members(body: str) -> list[Member]:
    members: list[Member] = []
    for unit in split_fields(body):
        doc = " ".join(
            line.strip()[3:].strip() for line in unit.splitlines() if line.strip().startswith("///")
        )
        field = None
        for line in unit.splitlines():
            stripped = line.strip()
            if stripped.startswith("pub "):
                field = stripped[4:]
                break
        if field is None or ":" not in field:
            continue
        name, _, type_ = field.partition(":")
        members.append(Member(name.strip(), type_.strip(), serde_attrs(unit), doc))
    return members


def parse_variants(body: str) -> list[tuple[str, str, str]]:
    variants: list[tuple[str, str, str]] = []
    for unit in split_fields(body):
        attrs = serde_attrs(unit)
        text = "\n".join(
            line for line in unit.splitlines() if not line.strip().startswith(("///", "#["))
        )
        match = re.match(r"\s*([A-Za-z_][A-Za-z0-9_]*)\s*(\(([^)]*)\))?", text)
        if not match:
            continue
        variants.append((match.group(1), attrs.get("rename", ""), (match.group(3) or "").strip()))
    return variants

def wire_documents() -> tuple[list[dict[str, object]], dict[str, str]]:
    documents = []
    by_resource: dict[str, str] = {}
    for path in sorted((SRC / "wire").glob("*.rs")):
        if path.name == "mod.rs":
            continue
        text = read(path)
        items, resource = parse_wire(text, f"crates/omeka-api/src/wire/{path.name}")
        if resource:
            by_resource[resource] = path.name
        documents.append({"file": path.name, "resource": resource, "items": items})
    return documents, by_resource

