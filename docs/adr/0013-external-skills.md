# 0013. External skills, out of process

- Status: Accepted
- Date: 2026-11-24

## Context

Built-in skills are compiled into the binary. Users and host applications
will want capabilities Talos does not ship — home automation, a company
directory, a host's existing tools — without forking Talos or learning Rust.
A host such as Tomos already has a library of tools worth exposing.

Loading native plugins at run time is fragile and unsafe; embedding a
scripting runtime costs memory that the budget does not have.

## Decision

A skill's manifest may declare an external handler instead of a compiled one.
The handler runs outside the Talos process; Talos sends it the intent, typed
slots, language, original text and conversation, and gets back `answer`,
`ask`, `decline` or `error` with a template identifier and variables.

**The transport is not chosen yet.** Two candidates, decided in v0.5:

- **Talos's own JSON protocol** — one request, one response, over a
  subprocess or HTTP. Minimal, no dependency.
- **Model Context Protocol (MCP)** — the handler is a tool on an MCP server;
  slots become the tool's arguments and its structured result becomes the
  reply variables. Talos would be an MCP client.

MCP is the leading candidate, for one reason: the owner intends Tomos's
skills to become MCP servers that both Tomos and Talos can use. That would
give the two programs shared capabilities with no coupling between them —
each talks to the same servers, neither talks to the other about tools.

Whatever the transport:

- The manifest still owns intents, examples, patterns and replies, so external
  skills are recognised, bilingual and rendered exactly like built-in ones.
- External skills are **allow-listed in configuration**. Talos never runs a
  handler merely because a manifest was found on disk.
- Each call has a timeout; failure renders an honest error and never blocks
  the next turn.
- Skills declare capabilities (`network`, `filesystem`, `exec`) and
  configuration grants them.
- A skill whose handler uses a language model declares `generated = true`;
  its outcomes are marked accordingly ([0012](0012-no-language-model.md)).
- Adding or removing an external skill's training examples requires
  retraining the classifier; until then its rule patterns still work.

## Consequences

- Any language can implement a skill; a host can expose its tools with a
  small adapter.
- Process spawn or an HTTP round trip costs milliseconds; fine for the kinds
  of skill that will be external, wrong for the core set.
- The protocol is an interface to keep stable and versioned.
- If MCP is chosen, Talos needs an MCP client (JSON-RPC over stdio at least),
  whose weight must be checked against the budget, and tool results must be
  structured enough to fill templates rather than free text.
- Talos and Tomos share no skills through their integration
  ([0010](0010-one-binary-three-front-ends.md)); MCP servers would be the
  shared layer.
- Security rests on the allow-list and on the rule that side-effecting intents
  need a rule match or confirmation.

## Alternatives considered

- **Dynamic libraries.** No stable Rust ABI; a crash takes Talos down.
- **WebAssembly plugins.** Sandboxed and portable, but a runtime adds
  megabytes and complexity. A candidate for later if sandboxing becomes a
  requirement.
- **Embedded scripting (Lua, Rhai).** Smaller than Wasm, still a runtime and a
  second language to support.
- **MCP as a replacement for manifests.** MCP describes tools for a model to
  choose from; it carries no training examples, patterns or reply templates.
  The manifest stays in Talos either way. MCP is a candidate for the
  *transport* only, as described above.
