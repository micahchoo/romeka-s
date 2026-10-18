# The documentation

Audience: anyone who has to use, run or change Romeka S and wants to know which page answers their
question.

## The pages here

| Page | Audience | What it covers |
| --- | --- | --- |
| [`user-guide.md`](user-guide.md) | an administrator in the admin app, or a visitor to a public site | install, sign in, the admin pages, importing data, the public site, search |
| [`operator-guide.md`](operator-guide.md) | whoever runs the process | the process, ports, the database and the file store, backup and restore, the static origin, the checks |
| [`api.md`](api.md) | a developer integrating over HTTP | every route and method, the accepted parameters, the response and request members, the access rules, the errors |
| [`compat.md`](compat.md) | someone deciding how far Romeka S can stand in for an Omeka S installation | what is compatible and what deliberately differs, each difference against the file that carries it |
| [`gen/README.md`](gen/README.md) | whoever edits the API reference | the generator behind `api.md`, and how to rerun it |

`api.md` is generated. Never edit it by hand. Run `./gen.sh` here after a change under
`crates/omeka-api/src/`, and commit the regenerated file with the change. `./gen.sh --check` fails
while the committed file is stale, and CI runs it.

## Around the repository

| Path | What it is |
| --- | --- |
| [`../README.md`](../README.md) | what Romeka S is, the quick start, and what is ported and what is not |
| [`../NOTICE`](../NOTICE) | the fork point, the upstream attribution and the license |
| [`../CONTRIBUTING.md`](../CONTRIBUTING.md) | how to build, test and send a change |
| [`../AGENTS.md`](../AGENTS.md) | the rules an agent working in this repository follows |
| [`../deploy/DEPLOY.md`](../deploy/DEPLOY.md) | the production deployment: image, compose, TLS proxy, backup, upgrade, rollback |
| [`../crates/`](../crates/) | the Rust workspace, one README per crate |
| [`../web/admin/README.md`](../web/admin/README.md) | the admin SPA: layout, routes, wire shapes, tests, and the performance log |
| [`../packaging/README.md`](../packaging/README.md) | the container image and the systemd unit |

## Keeping these pages true

- A number lives in the artifact that measures it, not in a page that quotes it. `web/admin/PERF.md`
  is the exception, because it is the decision log for its own numbers.
- A page that describes code names the source file it was read from.
- What the product is compatible with, and where it deliberately differs, is
  [`compat.md`](compat.md). A change that adds a difference adds a row there.
