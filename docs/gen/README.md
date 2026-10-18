# docs/gen

Audience: whoever edits the API reference, or reviews the generator behind it.

The generator behind `docs/api.md`. Nothing in `docs/api.md` is hand-written, so a
member the source drops disappears from the reference on the next run.

## Run it

```
cd docs
./gen.sh            # rewrite api.md
./gen.sh --check    # exit 1 when api.md no longer matches the source
./gen/check_wire.py # cross-check the parsed members against the golden fixtures
```

`gen.sh` needs only `python3` (no packages) and reads the crate at
`../crates/omeka-api/src`. It runs from any working directory.

## Files here

| File | What it holds |
| --- | --- |
| `../gen.sh` | the entry point: `./gen.sh` and `./gen.sh --check` |
| `api_doc.py` | the CLI: builds the tables and calls the renderer |
| `paths.py` | the source paths and the two text helpers |
| `russparse.py` | the Rust reader: blocks, strings, functions, match arms, the call graph |
| `wire.py` | parses `wire/*.rs` into documents, members and variants |
| `model.py` | reads the crate into the row tables |
| `markdown.py` | markdown table and inline-code helpers |
| `render.py` | assembles `api.md` from the tables |
| `check_wire.py` | the cross-check against the golden fixtures |
| `check_routes.py` | the cross-check of the route table against a running instance |

`check_routes.py` needs an installed database, a minted key and a server on a free port; it is
documented in its own docstring. It checks every route row that makes an exact claim, and it is
what found that a write the port has no handler for is a 501 even when the adapter does not list
the operation.

## What it reads

| File | What is parsed |
| --- | --- |
| `routes.rs` | the path template constants, the allowed methods per shape, and the method to `Operation` mapping in `resolve` |
| `adapters/mod.rs` | the adapter order |
| `adapters/<name>.rs` | one `SPEC` per adapter: resource name, representation, operations |
| `wire/<name>.rs` | the `pub struct`s, `pub enum`s, `pub type`s and `wire_const_str!` constants, with their serde renames, optionality and flattening |
| `read/mod.rs` | which handler serves which resource, for `search` and `read_member` |
| `read/*.rs` | the `params.<method>("name")` reads of each handler and of every function those handlers reach that also takes `params` |
| `read/query.rs` | the fields of one `property[i][...]` row |
| `write/mod.rs` | which handler serves which resource, for `create`, `update` and `delete` |
| `write/<name>.rs` | the `CONTROL` member list of one resource and its literal `payload` reads |
| `write/values.rs` | the members of one value object |
| `write/access.rs` | the adapter-level delete grant table and its role constants |

The parser is deliberately small: it understands `pub struct`, `pub enum`,
`pub type`, field-level serde attributes, `fn` bodies (with brace, string and
comment skipping) and `match` arms. It raises rather than guessing.

## Determinism

The output is byte-identical across runs and across `PYTHONHASHSEED` values; the
tables are sorted by key, and the route table follows `adapters/mod.rs` order. The
generator checks its own parse: it fails when the route table is not
`2 + adapters * 7` rows, or when the two read dispatch tables disagree.

## Cross-check against the fixtures

`gen/check_wire.py` compares every top-level key of every document in
`../verification/fixtures/api/` with the member list parsed for that resource. The
fixtures were captured from a real upstream server, so they are evidence
independent of this parser. It reports the fixtures and the count of keys it could
not find.

## Adding a resource

Nothing in this directory needs editing for a new adapter. Add the module under
`adapters/`, the wire types under `wire/`, and the handlers, then run `./gen.sh`.
A resource with no wire module is reported as delegating to its target adapter.
