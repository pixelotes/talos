# Talos

A deterministic assistant that answers without a language model.

Talos understands a bounded set of requests, answers them the same way every
time, and says so when a message is outside what it knows. It is built to run
on small hardware: one static binary of a few megabytes, a SQLite file, and
under 10 MB of memory.

> **Status: early, working.** Twelve skills, a command line and an
> OpenAI-compatible server run today, in Spanish and English. Recognition is
> at about 84 % on its evaluation batteries, short of its target; the dictionary
> works but its Spanish side is rough. See [Where it stands](#where-it-stands).

## What it does today

| Skill | Ask it | Shortcut |
|---|---|---|
| `clock` | "qué hora es", "what day is it", "cuánto falta para navidad" | `/hora` `/time`, `/fecha` `/date` |
| `timezones` | "qué hora es en Tokio", "time difference between Madrid and Lima" | `/horaen` `/timein` |
| `calc` | "cuánto es 12 por 12", "15% of 80", "5 km a millas" | `/calc` |
| `random` | "tira dos dados de 8 caras", "flip a coin", "elige entre pizza y sushi" | `/dado` `/roll`, `/moneda` `/coin` |
| `define` | "qué significa guitarra", "what does bank mean" | `/definir` `/define` |
| `translate` | "cómo se dice perro en inglés", "translate good morning to Spanish" | `/traduce` `/translate` |
| `teach` | "cuando digo peli quiero decir dime un refrán" | |
| `preferences` | "me llamo Ana", "I live in Lisbon", "olvida mi nombre" | |
| `fortune` | "dime un refrán", "tell me a saying" | `/refran` `/fortune` |
| `help`, `identity`, `smalltalk` | "qué sabes hacer", "who are you", "hola" | `/ayuda` `/help` |

When a request lacks something, Talos asks for it ("¿Para cuándo?") and takes
the next message as the answer. When nothing matches, a small ELIZA-style
reflector hands the conversation back, and the outcome says the message was
not handled.

## What it is

- **Deterministic.** Same input, same state and same clock give the same
  answer, byte for byte. No sampling, no model in the answer path.
- **Light.** A 3.6 MB binary; 8 to 10 MB of memory; a turn in microseconds to
  about a millisecond.
- **Bilingual.** Spanish and English, each written natively.
- **Standalone.** Every skill is built in. Nothing else has to be running.
- **Honest.** Every answer carries a status — `answered`, `deflected` or
  `unhandled` — so a host can pass the turn on to something else.

## What it is not

- Not a language model, and not a wrapper around one. It does not write,
  summarise or translate free text.
- Not a companion: no mood tracking, no personality engine, no unsolicited
  messages.
- Not multi-tenant: one owner, many conversations.

## Build and run

Needs a Rust toolchain; the version is pinned in `rust-toolchain.toml`.

```sh
cargo build --release -p talos
./target/release/talos chat                # a conversation at the terminal
./target/release/talos ask "qué hora es"   # one question
./target/release/talos explain "hola"      # how a message is answered, stage by stage
./target/release/talos serve --port 8765   # the server
```

The server speaks the OpenAI chat-completions interface, so an existing chat
client can use it with the model name `talos`:

```sh
curl -s localhost:8765/v1/chat/completions \
  -d '{"model":"talos","messages":[{"role":"user","content":"cuánto es 12 por 12"}]}'
```

Each reply carries a `talos` object and an `X-Talos-Status` header with the
status and the recognised intent. The server listens on loopback by default
and refuses any other address without a token (`--token` or `TALOS_TOKEN`).
Details in [docs/API.md](docs/API.md).

### Data

Talos keeps what it remembers in `user.db` under its data directory:
`$TALOS_DATA_DIR`, else `$XDG_DATA_HOME/talos`, else `~/.local/share/talos`.
Knowledge packs are `*.talospack` files in `packs/` inside it.

### The dictionary

`define` and `translate` need a vocabulary pack, which is not in this
repository. It is built from four downloaded inputs (Open English WordNet,
the Spanish wordnet of the Multilingual Central Repository and the two CILI
maps that join them to each other and to Wikidata) and a fifth that is in the
repository: a CC0 snapshot of Wikidata's Spanish descriptions
(`knowledge/sources/`), which give a Spanish gloss to words the Spanish
wordnet has none for. Where to get the four files and their checksums is in
[docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) and
[docs/research/wordnet-sources.md](docs/research/wordnet-sources.md).

```sh
cargo run --release -p talos-pack -- wordnet DIR    # DIR holds the four downloaded files
mkdir -p ~/.local/share/talos/packs
cp target/packs/vocabulary.talospack ~/.local/share/talos/packs/
./target/release/talos packs list
```

The pack holds about 120,000 concepts, 162,000 English and 94,000 Spanish
words, in 78 MB. It carries its sources' licences, which are not this
project's: see the `vocabulary.NOTICE` written beside it.

## Where it stands

Measured on the development machine; the figures and how they were taken are
in [evals/BASELINE.md](evals/BASELINE.md) and
[docs/DEVELOPMENT.md](docs/DEVELOPMENT.md).

| | Now | Target |
|---|---|---|
| Intents recognised, on the evaluation batteries | 84 % (es), 81 % (en) | 92 %, 95 % |
| Wrongly accepted, of 99 messages that should not be | 4 (es), 1 (en) | at most 3 |
| Binary, stripped | 3.6 MB | 6 MB |
| Memory, a conversation with the dictionary attached | about 9 to 10 MB | 20 MB |
| A dictionary lookup, p99 | 1.3 ms | 10 ms |

Known to be rough:

- **Spanish in the dictionary.** Three Spanish concepts in four have no
  Spanish definition; the Spanish source does not order senses, so a rare one
  can come first; irregular Spanish verb forms are not found.
- **Recognition of the newest skills** (`define`, `teach`) is the weakest.
- **Platforms.** Developed on macOS; the Linux builds are produced by CI and
  have had little use.

Next, in order: tools that need the network (weather, news, Wikipedia,
search), then reasoning over what the dictionary already holds. The plan is in
[docs/ROADMAP.md](docs/ROADMAP.md).

## Documentation

| Document | What it covers |
|---|---|
| [docs/DESIGN.md](docs/DESIGN.md) | Architecture: the turn pipeline, components, budgets |
| [docs/SKILLS.md](docs/SKILLS.md) | Skills, manifests, slots, the reflector |
| [docs/SKILL_AUTHORING.md](docs/SKILL_AUTHORING.md) | Writing a skill, step by step |
| [docs/KNOWLEDGE.md](docs/KNOWLEDGE.md) | The knowledge store: schema, packs |
| [docs/MODEL.md](docs/MODEL.md) | The intent classifier and its file format |
| [docs/API.md](docs/API.md) | Command line and HTTP interfaces |
| [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) | Building, testing, measuring; decisions taken along the way |
| [docs/ROADMAP.md](docs/ROADMAP.md) | Milestones and exit criteria |
| [docs/adr/](docs/adr/README.md) | Architecture decision records |

## Origin

Talos is a from-scratch redesign of the idea behind an earlier component of the
same name inside the Tomos project. It shares the concept and none of the code.
See [ADR 0002](docs/adr/0002-clean-room-rewrite.md).

## Licence

[MIT](LICENSE). Knowledge packs built from third-party sources carry their own
licences. See [ADR 0014](docs/adr/0014-mit-licence.md).
