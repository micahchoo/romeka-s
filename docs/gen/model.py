#!/usr/bin/env python3
"""Read the omeka-api crate into the tables the reference is rendered from.

Nothing here renders; every function returns rows or plain values. The sources
are `routes.rs`, `adapters/*.rs`, `wire/*.rs`, `read/*.rs` and `write/*.rs`.
"""

from __future__ import annotations

import re
from pathlib import Path

from markdown import code
from paths import SRC, read, strip_tests
from russparse import (
    PARAM_KINDS,
    block_end,
    match_arms,
    param_closure,
    parse_functions,
)
from wire import wire_documents


class Doc:
    """Everything the renderer needs, gathered from the crate."""

    def __init__(
        self,
        adapters: list[dict[str, object]],
        consts: dict[str, str],
        allow: dict[str, list[str]],
        rows: list[list[str]],
        index: dict[str, list[object]],
        search_fn: dict[str, str],
        write_dispatch: dict[str, dict[str, tuple[str, str]]],
        wire_by_resource: dict[str, str],
        wire_by_file: dict[str, dict[str, object]],
    ) -> None:
        self.adapters = adapters
        self.consts = consts
        self.allow = allow
        self.rows = rows
        self.index = index
        self.search_fn = search_fn
        self.write_dispatch = write_dispatch
        self.wire_by_resource = wire_by_resource
        self.wire_by_file = wire_by_file
# --------------------------------------------------------------------------
# Reading the crate.
# --------------------------------------------------------------------------


def parse_const_strs(text: str) -> dict[str, str]:
    return dict(re.findall(r'pub const (\w+): &str = "([^"]*)"', text))


def parse_method_lists(text: str) -> dict[str, list[str]]:
    lists: dict[str, list[str]] = {}
    for name, body in re.findall(r"pub static (\w+): &\[Method\] = &\[(.*?)\];", text, re.S):
        lists[name] = re.findall(r"Method::(\w+)", body)
    return lists


def parse_adapters(src: Path) -> list[dict[str, object]]:
    order_text = strip_tests(read(src / "adapters" / "mod.rs"))
    order = re.findall(r"(\w+)::SPEC,", order_text)
    adapters: list[dict[str, object]] = []
    for module in order:
        text = read(src / "adapters" / f"{module}.rs")
        spec = re.search(r"pub const SPEC: AdapterSpec = AdapterSpec \{(.*?)\n\};", text, re.S)
        if not spec:
            raise SystemExit(f"no SPEC in adapters/{module}.rs")
        body = spec.group(1)
        name = re.search(r'name:\s*"([^"]+)"', body)
        representation = re.search(r'representation:\s*"((?:[^"\\]|\\.)*)"', body)
        adapters.append(
            {
                "name": name.group(1),
                "representation": representation.group(1).replace("\\\\", "\\"),
                "operations": re.findall(r"Operation::(\w+)", body),
                "module": module,
            }
        )
    return adapters


def parse_dispatch(text: str, fn_name: str) -> dict[str, tuple[str, str]]:
    """One match arm per resource in a dispatch function, as (module, function)."""
    functions = parse_functions(text, "mod")
    target = next((fn for fn in functions if fn.name == fn_name), None)
    if target is None:
        raise SystemExit(f"no fn {fn_name}")
    arms = re.findall(
        r'"([a-z_]+)"\s*=>\s*(?:Ok\()?\s*([a-z_][a-z0-9_]*)::([a-z_][a-z0-9_]*)', target.body
    )
    return {resource: (module, handler) for resource, module, handler in arms}


def collect_read_source() -> tuple[dict[str, tuple[str, str]], dict[str, tuple[str, str]]]:
    mod_text = read(SRC / "read" / "mod.rs")
    search = parse_dispatch(mod_text, "search")
    member = parse_dispatch(mod_text, "read_member")
    return search, member

def collect_route_operations(routes_text: str) -> dict[str, list[tuple[str, str]]]:
    """The method -> operation mapping in `routes::resolve`, split by path shape."""
    functions = parse_functions(routes_text, "routes.rs")
    resolve = next(fn for fn in functions if fn.name == "resolve")
    body = resolve.body
    shapes = {}
    collection_start = body.index("ApiPath::Collection")
    member_start = body.index("ApiPath::Member")
    shapes["Collection"] = body[collection_start:member_start]
    shapes["Member"] = body[member_start:]
    mapping = {}
    for shape, text in shapes.items():
        pairs = re.findall(r"Method::(\w+)\s*=>\s*\w+\(adapter,\s*Operation::(\w+)\)", text)
        mapping[shape] = pairs
    if not mapping["Collection"] or not mapping["Member"]:
        raise SystemExit("could not read the resolve() operation mapping")
    return mapping


OPERATION_NAMES = {
    "Search": "search",
    "Read": "read",
    "Create": "create",
    "Update": "update",
    "Patch": "patch (partial update)",
    "Delete": "delete",
}


def route_rows(
    consts: dict[str, str],
    allow: dict[str, list[str]],
    adapters: list[dict[str, object]],
    operation_order: dict[str, list[tuple[str, str]]],
    served_read: list[str],
    write_dispatch: dict[str, dict[str, str]],
) -> list[list[str]]:
    rows: list[list[str]] = [
        ["(root)", "GET", consts["COLLECTION_PATH"].rsplit("/", 1)[0], "api root listing", "served"],
        ["(root)", "GET", consts["CONTEXT_PATH"], "api context document", "served"],
    ]
    for adapter in adapters:
        name = str(adapter["name"])
        operations = list(adapter["operations"])
        for shape, path_const in (("Collection", "COLLECTION_PATH"), ("Member", "MEMBER_PATH")):
            for method, operation in operation_order[shape]:
                rows.append(
                    [
                        name,
                        method.upper(),
                        consts[path_const],
                        OPERATION_NAMES[operation],
                        port_status(name, operation, operations, served_read, write_dispatch),
                    ]
                )
    return rows


def port_status(
    resource: str,
    operation: str,
    adapter_operations: list[str],
    served_read: list[str],
    write_dispatch: dict[str, dict[str, str]],
) -> str:
    # The server routes PATCH to the update handler as a partial update, so a
    # PATCH the port cannot serve reports the update operation, never patch.
    partial = operation == "Patch"
    lookup = "Update" if partial else operation
    if lookup in ("Search", "Read"):
        if resource in served_read:
            return "served (" + lookup.lower() + ")"
        if lookup in adapter_operations:
            return "501: no handler yet"
        return "500: the adapter does not implement " + lookup.lower()
    # A write with no handler is a 501 whatever the adapter lists:
    # the write dispatch never consults the operation list.
    if resource in write_dispatch[lookup.lower()]:
        return "served (partial update)" if partial else "served (" + lookup.lower() + ")"
    return "501: no handler yet"


def list_parameters(
    resource: str, search_fn: dict[str, str], index: dict[str, list[Function]]
) -> list[list[str]]:
    handler_name = search_fn.get(resource)
    if handler_name is None:
        return []
    entry = next(
        (fn for fn in index.get(handler_name, []) if fn.takes_params),
        None,
    )
    if entry is None:
        return []
    reached = param_closure(entry, index)
    reads: dict[str, dict[str, set[str]]] = {}
    for function in reached:
        for kind, name in function.param_reads():
            reads.setdefault(name, {"kind": set(), "by": set()})
            reads[name]["kind"].add(PARAM_KINDS[kind])
            reads[name]["by"].add(f"{function.qualified} ({function.source_file})")
    rows = []
    for name in sorted(reads):
        kinds = ", ".join(sorted(reads[name]["kind"]))
        by = ", ".join(code(reader) for reader in sorted(reads[name]["by"]))
        rows.append([code(name), kinds, by])
    return rows


def request_body_rows(
    resource: str, write_dispatch: dict[str, dict[str, str]]
) -> list[list[str]]:
    modules = {
        operation: mapping[resource][0]
        for operation, mapping in write_dispatch.items()
        if resource in mapping
    }
    if not modules:
        return []
    rows: list[list[str]] = []
    seen: set[str] = set()
    for module in sorted(set(modules.values())):
        text = strip_tests(read(SRC / "write" / f"{module}.rs"))
        control: list[str] = []
        for block in re.findall(r"const CONTROL: &\[&str\] = &\[(.*?)\];", text, re.S):
            control.extend(re.findall(r'"([^"]+)"', block))
        reads = re.findall(
            r'payload\s*\.\s*(get|has|str_member|bool_member|ref_id|int_member)\s*\(\s*"([^"]+)"',
            text,
        )
        for name in control:
            if name in seen:
                continue
            seen.add(name)
            rows.append([code(name), "control member", f"`crates/omeka-api/src/write/{module}.rs`"])
        for kind, name in reads:
            if name in seen or name in control:
                continue
            seen.add(name)
            rows.append(
                [code(name), payload_kind(kind), f"`crates/omeka-api/src/write/{module}.rs`"]
            )
        if "values::collect" in text or "resource::hydrate" in text:
            rows.append(
                [
                    "any property term (`prefix:local_name`)",
                    "list of value objects",
                    f"`crates/omeka-api/src/write/values.rs`",
                ]
            )
    rows.append(
        [
            "value object members: `property_id`, `type`, `@value`, `@id`, `value_resource_id`, `is_public`, `@language`, `o:label`",
            "one object per value",
            "`crates/omeka-api/src/write/values.rs`",
        ]
    )
    return rows


def property_row_fields() -> list[str]:
    """The fields a `property[i][...]` query row carries, from query.rs."""
    text = strip_tests(read(SRC / "read" / "query.rs"))
    function = next(
        (fn for fn in parse_functions(text, "read/query.rs") if fn.name == "property_row"), None
    )
    if function is None:
        return []
    return sorted(
        set(
            re.findall(
                r'row\s*\.\s*(?:get|int|bool)\s*\(\s*"([^"]+)"', function.body
            )
        )
    )


def read_access_rows() -> list[list[str]]:
    """The `Api::allow` arms in read/mod.rs: who may search or read a resource."""
    text = strip_tests(read(SRC / "read" / "mod.rs"))
    function = next((fn for fn in parse_functions(text, "mod") if fn.name == "allow"), None)
    if function is None:
        raise SystemExit("no Api::allow in read/mod.rs")
    rows = []
    for pattern, condition in match_arms(function):
        names = re.findall(r'"([a-z_]+)"', pattern)
        label = ", ".join(code(name) for name in names) if names else "every other resource"
        rows.append([label, access_phrase(condition)])
    if not rows:
        raise SystemExit("no match arms in Api::allow")
    return rows


def access_phrase(condition: str) -> str:
    if "is_privileged" in condition or condition == "privileged":
        return "a caller whose actor has view-all"
    if "user().is_some()" in condition:
        return "any authenticated caller"
    if condition == "true":
        return "everyone, including anonymous callers"
    if condition == "false":
        return "nobody"
    return code(condition)


ADMIN_ROLES = ["site_admin", "global_admin"]


def grant_phrase(token: str, roles: dict[str, list[str]]) -> str:
    """One `Grant` in the `ADAPTER_WRITES` table as the roles it admits."""
    if token == "Grant::Admins":
        return "the admin roles"
    if token == "Grant::AnyRole":
        return "any caller"
    body = re.fullmatch(r"Grant::Roles\((\w+)\)", token)
    if body is None:
        raise SystemExit(f"unknown grant {token}")
    return ", ".join(code(role) for role in roles[body.group(1)] + ADMIN_ROLES)


def adapter_write_rows() -> list[list[str]]:
    """The `ADAPTER_WRITES` table in write/access.rs: the create, update and
    delete grant of each resource.

    The admin roles pass every row, so a `Grant::Roles` cell lists the other
    roles the row admits and the admin roles after them. A resource with no row
    is admin-only.
    """
    text = strip_tests(read(SRC / "write" / "access.rs"))
    table = re.search(
        r"const ADAPTER_WRITES: &\[\(&str, Grant, Grant, Grant\)\] = &\[(.*?)\n\];",
        text,
        re.S,
    )
    if table is None:
        raise SystemExit("no ADAPTER_WRITES table in write/access.rs")
    roles = {
        name: re.findall(r'"([a-z_]+)"', body)
        for name, body in re.findall(r"const (\w+): &\[&str\] = &\[(.*?)\];", text, re.S)
    }
    rows = []
    aliases = {
        name: token
        for name, token in re.findall(r"const (\w+): Grant = (Grant::\w+\([\w]+\));", text)
    }
    for row in re.findall(
        r'\(\s*"([a-z_]+)"\s*,\s*([^,]+?)\s*,\s*([^,]+?)\s*,\s*([^,]+?)\s*,?\s*\)',
        table.group(1),
    ):
        resource, *grants = [part.strip() for part in row]
        resolved = [aliases.get(grant, grant) for grant in grants]
        rows.append([code(resource)] + [grant_phrase(grant, roles) for grant in resolved])
    if not rows:
        raise SystemExit("no rows in ADAPTER_WRITES")
    return rows


def error_status_rows() -> list[list[str]]:
    """Every `ApiError` variant with the status `ApiError::status` gives it."""
    text = strip_tests(read(SRC / "errors.rs"))
    function = next((fn for fn in parse_functions(text, "mod") if fn.name == "status"), None)
    if function is None:
        raise SystemExit("no ApiError::status")
    rows = []
    for variant, status in re.findall(
        r"Self::(\w+)\s*(?:\{[^}]*\}|\([^)]*\))\s*=>\s*StatusCode::(\w+)", function.body
    ):
        rows.append(
            [
                code("ApiError::" + variant),
                str(STATUS_CODES.get(status, status)),
                ERROR_PHRASES.get(variant, ""),
            ]
        )
    return rows


def error_status_codes() -> list[int]:
    return [int(row[1]) for row in error_status_rows() if row[1].isdigit()]


STATUS_CODES = {
    "BAD_REQUEST": 400,
    "UNAUTHORIZED": 401,
    "FORBIDDEN": 403,
    "NOT_FOUND": 404,
    "CONFLICT": 409,
    "METHOD_NOT_ALLOWED": 405,
    "UNSUPPORTED_MEDIA_TYPE": 415,
    "UNPROCESSABLE_ENTITY": 422,
    "INTERNAL_SERVER_ERROR": 500,
    "NOT_IMPLEMENTED": 501,
}

ERROR_PHRASES = {
    "BadRequest": "the body is not a JSON object, or a member the resource rejects",
    "PermissionDenied": "the caller's role does not hold the operation",
    "NotFound": "an unknown resource name, or a row the caller may not see",
    "Conflict": "the request clashes with stored state, such as deleting a job a running import holds",
    "StalePage": (
        "Romeka: a site page `PUT` or `PATCH` whose `o:modified` is not the stored page's; the error"
        " object adds the stored `o:modified` and `o:user` (`o:id`, `o:name`) of who saved it."
        " A body without `o:modified` is never refused"
    ),
    "MethodNotAllowed": "the method is not routed on that path shape",
    "UnsupportedMediaType": "a write body whose Content-Type is neither JSON nor multipart",
    "Validation": "a body member failed validation, keyed by member",
    "MergedValidation": "a body member failed validation, with one entry per failing sub-entity",
    "NotImplemented": "a route with no handler yet",
    "Internal": "a storage failure, or an adapter operation with no handler",
}


def payload_kind(kind: str) -> str:
    return {
        "get": "any JSON",
        "has": "presence only",
        "str_member": "string",
        "bool_member": "boolean",
        "ref_id": "reference object with `o:id`",
        "int_member": "integer",
    }[kind]
