# 0014. MIT licence

- Status: Accepted
- Date: 2026-11-24

## Context

Talos is written from scratch ([0002](0002-clean-room-rewrite.md)) and can
choose its own terms. It is meant to be embedded in and called by other
programs, and its dependency policy already requires permissive licences.

## Decision

Talos is released under the MIT licence.

- The licence covers the code and the project's own data: skill manifests,
  reflector scripts, the seed pack, evaluation batteries, documentation.
- Knowledge packs built from third-party sources carry the licences of those
  sources and are distributed separately from the binary
  ([0006](0006-knowledge-packs.md)).
- Dependencies must be compatible with MIT distribution: MIT, Apache-2.0, BSD,
  ISC, Zlib, public domain. No copyleft in the binary. CI checks this.

## Consequences

- Anyone may use, modify and redistribute Talos, including in closed
  products, keeping the copyright notice.
- There is no explicit patent grant. Accepted for a project of this size.
- Improvements made by others need not come back.

## Alternatives considered

- **MIT OR Apache-2.0**, the Rust convention. Adds a patent grant at the cost
  of two licence files; the owner preferred the simpler single licence.
- **MPL-2.0 or AGPL-3.0.** Would require changes to flow back; not a goal.
