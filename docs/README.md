# The port's documentation

Audience: anyone who has to use, run or change this repository and wants to know which document answers their question.

## The documents here

| Document | Audience | What it covers |
| --- | --- | --- |
| [`user-guide.md`](user-guide.md) | an administrator in the SPA, or a visitor to a public site | install, sign in, the admin pages, importing data, the public site, search |
| [`operator-guide.md`](operator-guide.md) | whoever runs the process | the two processes, ports, the database and the file store, backup and restore, the gate, packaging |
| [`api.md`](api.md) | a developer integrating over HTTP | every route and method, the accepted parameters, the response and request members, the access rules, the errors |
| [`compat.md`](compat.md) | a reviewer or an operator deciding whether to switch | where the port stands against upstream, and where the evidence lives |
| [`contributing.md`](contributing.md) | an agent or a developer changing this repository | the brief, worktree, acceptance and merge rules |
| [`gen/README.md`](gen/README.md) | whoever edits the API reference | the generator behind `api.md`, and how to rerun it |

`api.md` is generated. Never edit it by hand. Run `./gen.sh` here after any change under
`crates/omeka-api/src/`, and commit the regenerated file.

## The rest of the repository

These are the sources of truth for the port, and these documents point at them rather than repeat them.

| Path | What it is |
| --- | --- |
| [`../README.md`](../README.md) | what the port is, and the map of the repository |
| [`../DESIGN.md`](../DESIGN.md) | the frozen target: crate layout, data shapes, the REST contract, decisions, exclusions |
| [`../VERIFY.md`](../VERIFY.md) | how "the port is complete" is defined and checked, layer by layer |
| [`../PLAN.md`](../PLAN.md) | the work program, wave by wave, with the log |
| [`../ROADMAP.md`](../ROADMAP.md) | the maximal scope and the ranking |
| [`../AGENTS.md`](../AGENTS.md) | the rules every agent in this repository follows |
| [`../briefs/`](../briefs/) | one brief per unit of work |
| [`../crates/`](../crates/) | the Rust workspace; each crate with a README is documented there |
| [`../web/admin/README.md`](../web/admin/README.md) | the admin application: layout, routes, wire shapes, tests |
| [`../verification/`](../verification/) | the matrices, the ledger, the scenario diff, the acceptance run |
| [`../oracle/README.md`](../oracle/README.md) | the upstream reference server in Docker |
| [`../research/`](../research/) | the upstream inventory the port was planned from |

## Keeping these documents true

- Numbers and per-file details live in the artifact that generates them, not in prose here. A
  document links the artifact.
- Anything that describes code names the source file it was read from.
- `../AGENTS.md` says the rules; these pages say what a user or an operator sees.
