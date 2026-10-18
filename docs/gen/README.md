# docs/gen

Audience: whoever edits the API reference, or reviews the generator behind it.

The generator behind `docs/api.md`. Nothing in `docs/api.md` is hand-written, so a member the source
drops disappears from the reference on the next run.

## Run it

```
cd docs
./gen.sh            # rewrite api.md
./gen.sh --check    # exit 1 when api.md no longer matches the source
```

`gen.sh` needs only `python3` (no packages) and reads the crate at `../crates/omeka-api/src`. It runs
from any working directory, and CI runs `--check` on every pull request.

Two cross-checks need more than the crate, and neither runs in CI:

```
./gen/check_wire.py                    # needs the captured documents, see below
./gen/check_routes.py <base> <identity> <credential>
```

`check_wire.py` compares every top-level key of every captured upstream document under
`verification/fixtures/api/` with the member list parsed for that resource. That directory is not in
this repository, so the cross-check cannot run here. The two `omeka-api` test targets that read the
same documents (`tests/golden.rs` and `tests/property_wire.rs`) fail for the same reason.

`check_routes.py` reads the route table out of `api.md`, sends every row's request to a running
instance with a global-admin key, and holds each row to its exact claim: a 501 row must answer 501, a
500 row must answer 500, and a served row must not answer either. It creates rows, so point it at a
throwaway database on a port nobody else is using:

```
romeka install -d /tmp/proof/romeka.db --email a@b.c --name A --password x
romeka key add -d /tmp/proof/romeka.db --owner a@b.c
romeka serve -d /tmp/proof/romeka.db --port 18081 &
./gen/check_routes.py http://127.0.0.1:18081 <identity> <credential>
```

## Files here

| File | What it holds |
| --- | --- |
| `../gen.sh` | the entry point: `./gen.sh` and `./gen.sh --check` |
| `api_doc.py` | the CLI: builds the tables and calls the renderer |
| `paths.py` | the source paths, and the helpers that read a file and drop its test code |
| `russparse.py` | the Rust reader: blocks, strings, functions, match arms, the call graph |
| `wire.py` | parses `wire/*.rs` into documents, members and variants |
| `model.py` | reads the crate into the row tables |
| `markdown.py` | markdown table and inline-code helpers |
| `render.py` | assembles `api.md` from the tables |
| `check_wire.py` | the cross-check against the captured upstream documents |
| `check_routes.py` | the cross-check of the route table against a running instance |

## What it reads

| File | What is parsed |
| --- | --- |
| `routes.rs` | the path template constants, the allowed methods per shape, and the method to `Operation` mapping in `resolve` |
| `adapters/mod.rs` | the adapter order, with the test module dropped |
| `adapters/<name>.rs` | one `SPEC` per adapter: resource name, representation, operations |
| `wire/<name>.rs` | the `pub struct`s, `pub enum`s, `pub type`s and `wire_const_str!` constants, with their serde renames, optionality and flattening |
| `read/mod.rs` | which handler serves which resource, for `search` and `read_member`, and the `Api::allow` arms |
| `read/*.rs` | the `params.<method>("name")` reads of each handler and of every function those handlers reach that also takes `params` |
| `read/query.rs` | the fields of one `property[i][...]` row |
| `write/mod.rs` | which handler serves which resource, for `create`, `update` and `delete` |
| `write/<name>.rs` | the `CONTROL` member list of one resource and its literal `payload` reads |
| `write/values.rs` | the members of one value object |
| `write/access.rs` | the `ADAPTER_WRITES` table: the create, update and delete grant of each resource |

The parser is deliberately small: it understands `pub struct`, `pub enum`,
`pub type`, field-level serde attributes, `fn` bodies (with brace, string and
comment skipping) and `match` arms.

It fails rather than guesses. A dispatch function, an `Api::allow`, or the `ADAPTER_WRITES` table it
cannot find stops the run with a message, and the output is not written. A silently empty table is the
one outcome it will not produce.

## Determinism

The output is byte-identical across runs and across `PYTHONHASHSEED` values; the
tables are sorted by key, and the route table follows `adapters/mod.rs` order. The
generator checks its own parse: it fails when the route table is not
`2 + adapters * 7` rows, or when the two read dispatch tables disagree.

## Adding a resource

Nothing in this directory needs editing for a new adapter. Add the module under
`adapters/`, the wire types under `wire/`, the handlers and the row in `write/access.rs`'s
`ADAPTER_WRITES` table, then run `./gen.sh`. A resource with no wire module is reported as delegating
to its target adapter.
