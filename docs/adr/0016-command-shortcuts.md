# 0016. Command shortcuts are part of the engine

- Status: Accepted
- Date: 2026-11-25

## Context

The assistant is used mostly for direct requests — the weather somewhere, the
news, a sum — and far less for open conversation. For those, a typed shortcut
such as `/weather Valencia` or `/calc 1 + 2` is faster for the user and leaves
nothing to recognise: the user has named the capability.

A host could implement shortcuts itself, in front of Talos. Then every front
end (terminal, chat, server, and any host talking to the server) would parse
them separately and would need to know which skills are installed and how
their arguments are shaped. The engine already holds that knowledge in the
skill manifests ([0007](0007-declarative-skill-manifests.md)).

## Decision

A message whose first character is `/` is a **command**. Commands are handled
by the engine, as the first check of the rule stage
([0004](0004-explicit-turn-pipeline.md)).

1. **Declared in the manifest.** An intent may declare a command: a name per
   language (`/weather`, `/tiempo`) and the one slot that receives the rest of
   the line. Names are lower-case ASCII. Both languages' names always work,
   whatever the language of the conversation.
2. **Recognition is skipped, the engine is not.** A command goes straight to
   its intent with certainty: no rules, no classifier, no reflector, no safety
   check on what is a command line and not prose. Everything else is the
   ordinary path: the rest of the line fills the declared slot and passes
   through the same typed extractor as a captured slot, the same handler
   runs, the same templates render, and the outcome, the conversation state
   and the trace are produced as for any turn.
3. **Language.** The reply is in the conversation's language. A command name
   does not change it; `/tiempo` in an English conversation answers in
   English.
4. **Missing argument.** Handled as a missing slot is for a sentence: Talos
   asks for it once open questions exist, and until then gives the usage hint.
5. **Unknown command.** `unhandled`, with a one-line reply naming the command
   that lists the available ones. It never falls through to the rules or the
   reflector: `/wether Valencia` is a typing error, not a sentence.
6. **Discovery.** `/help` (`/ayuda`) lists the installed commands, generated
   from the manifests like the rest of the help answer.
7. **Validation.** `talos skills check` rejects two intents declaring the same
   command name, and a command whose slot the intent does not declare. An
   external skill ([0013](0013-external-skills.md)) declares commands the same
   way; on a clash with a built-in, the built-in keeps the name and the clash
   is reported when the skill is loaded.

An intent that needs several independent slots either takes them as one
free-text argument its handler already knows how to read, or declares no
command.

## Consequences

- One implementation serves every front end. A host forwards the text and
  reads the outcome; it needs no list of skills.
- `/calc 1 + 2` and "cuánto es 1 + 2" share the handler and the reply, so a
  shortcut cannot drift from the sentence form.
- Commands are the cheapest turns there are, and fully deterministic.
- A leading `/` is reserved. A sentence that genuinely begins with one (a
  path, a date written `/3/4`) is read as an unknown command. Accepted: it is
  rare, and the reply says how to list commands.
- A small addition to the manifest format and to the validator.

## Alternatives considered

- **Shortcuts in the host, bypassing Talos.** How the previous system worked.
  Each host reimplements parsing and argument handling, replies and status
  come from a different path than the sentence form, and nothing is traced.
- **No shortcuts; rely on recognition.** Recognition is good enough for the
  sentences, but a shortcut is a different thing: an explicit instruction that
  must never be misread, including by a classifier that has not yet learned a
  new skill's phrasings.
- **Commands as ordinary rule patterns in each skill.** Works, but every skill
  would restate the same syntax, unknown commands would fall to the reflector,
  and there would be no single list to show or validate.
