# 0018. The network skills read pages through `ureq` with rustls, in the binary only

- Status: Accepted
- Date: 2026-11-26

## Context

The first skill that needs the network is `wikipedia` (ROADMAP v0.5); weather,
news and search follow. Wikipedia is served over HTTPS only, and Talos had
no TLS. DESIGN.md §9 listed `ureq` as the candidate, undecided, behind a
`net` feature.

What the client has to do is narrow: a blocking `GET` of a small JSON or
HTML body, with a deadline, from inside a synchronous turn. What it may
cost is what the owner cares about most: memory first, then independence
from the host (Talos runs as one static binary), then binary size.

### Measurements

The same `GET` of one Wikipedia summary, release profile of the workspace,
on the development machine (macOS, ARM64; `spikes/net-client/measure.sh`):

| Client | Binary | Process's own memory (peak footprint) | Peak resident |
|---|---|---|---|
| none (baseline) | 286 KB | 1.2 MB | 1.7 MB |
| the system's `curl`, as a child process | +35 KB | 1.2 MB | 6.4 MB (the child's, while it runs) |
| `ureq` 3.5 with rustls, ring and the Mozilla roots | +0.9 MB | 3.2 MB | 12.0 MB |

In Talos itself: the stripped binary goes from 3 662 960 B to 4 511 152 B
(+848 KB); a five-turn conversation with three lookups peaks at a 9.1 MB
footprint against 7.6 MB without the network (resident 20.6 MB against
11.2 MB on macOS, where resident memory counts the system's networking
libraries; not yet measured on Linux, where the binary is static).

## Decision

- **`ureq` with rustls**, compiled into the binary behind its `net` feature
  (on by default). The roots are compiled in (`webpki-roots`), so the static
  binary needs nothing from the host.
- **The engine links none of it.** `talos-core` defines a one-method trait,
  `net::Fetch`; the front end gives the engine an implementation
  (`Engine::use_network`), and a handler reaches it through
  `Context::network`. Tests give one that answers from recorded pages, so
  nothing in the test suite or CI touches the network.
- **Off until allowed.** A front end gives the engine the client only with
  `--net` or `TALOS_NET=1`. Without it a network skill is installed all the
  same (the same manifests, so the same classifier) and answers that it has
  no network access (`unhandled`).
- **Bounded.** Five seconds for a whole request, a megabyte for a body,
  HTTPS only. One request per page; no retries inside a turn.
- **It says little about its user.** Requests are sent as `talos/<version>`.
  A contact is added only when the owner sets `TALOS_CONTACT`.

## Consequences

- +0.85 MB of binary and about 1.5 MB of memory once a request has been
  made; nothing when the network is not allowed.
- 34 more crates in the build of the binary (rustls, ring, webpki). ring
  needs a C compiler for the target, which the static builds already have
  for SQLite. The roots are data under CDLA-Permissive-2.0, added to the
  licence allow-list.
- HTTP/1.1 only. Sites that treat HTTP/1.1 clients without a contact as
  robots limit them: after about a dozen requests in a few minutes from the
  development machine, Wikipedia answered 429 ("we suspect this request is
  automated") to every HTTP/1.1 request from it, whatever the client, while
  HTTP/2 requests passed. The limit lifted by itself within the hour:
  five requests four seconds apart then passed, with a contact and, as a
  control, one without. So it is a limit on bursts, which a person asking
  questions does not reach; its policy still asks automated clients for a
  User-Agent with a contact, and a server should set one. Not yet tried
  from a cloud address, which such policies treat more strictly.
- A network answer is not deterministic: the same question gives another
  answer when the page changes. Everything else about the turn still is,
  and the reply names its source.

## Alternatives considered

- **The system's `curl` as a child process.** No cost to the process or the
  binary, and TLS kept current by the host. Rejected by the owner: a stock
  Ubuntu server image does not ship `curl`, and Talos should not depend on
  what the host has installed.
- **`native-tls` (OpenSSL).** Not static, and a host dependency again.
- **An async client (`reqwest`, `hyper`).** HTTP/2, but an async runtime and
  several megabytes for one blocking `GET`. To be reconsidered only if
  HTTP/1.1 proves unusable against the sites the skills need.
