#!/usr/bin/env python3
"""Validate the Talos evaluation batteries. Dependency-free; exits non-zero on failure.

Usage: python3 evals/check.py
"""
import json
import re
import sys
from collections import Counter, defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
SPEC = HERE.parent / "docs" / "spec" / "README.md"
LANGS = ("es", "en")
MIN_PER_INTENT = 8
MIN_REJECT = 70
MIN_NEAR_MISS = 15
MIN_CONVERSATIONS = 20
KINDS = {"generative", "open_question", "statement", "future_skill", "near_miss", "noise", "conversational"}
STATUSES = {"answered", "deflected", "unhandled"}  # final outcomes (README, "Which status an outcome gets")
TURN_STATUSES = STATUSES | {"asks"}  # a turn where Talos asks back is not a final outcome
# Reflector sentence shapes (reflector.md section 2) and unhandled reasons (section 4).
# Neither is an intent: they only travel in the optional `trace` field.
DEFLECTED_SHAPES = {
    "opinion", "assertion", "plan", "wish", "self", "past", "taste", "compare", "hypothetical",
    "generalisation", "uncertain", "complaint", "why-me", "third-party", "about-talos", "ask-opinion",
    "ask-experience", "compliment", "insult", "apology", "agree", "disagree", "exclaim", "backchannel",
    "presence", "misunderstood",
}
UNHANDLED_REASONS = {
    "advice", "cannot", "distress", "unknown-fact", "generative", "not-yet", "noise", "catch-all",
    "other-language", "long-text",
}
# Slot names per intent, from the "Slots" line of each intent section of the specification
# (basics.md, state.md, knowledge.md). Intents not listed define no slots. Rows may only use these names.
SLOTS = {
    "help.skill": {"skill"},
    "clock.now": {"offset"},
    "clock.date": {"day"},
    "clock.weekday": {"date"},
    "clock.until": {"target"},
    "timezones.time_in": {"place"},
    "timezones.difference": {"from", "to"},
    "calc.arithmetic": {"expression"},
    "calc.percent": {"percent", "amount", "other"},
    "calc.convert": {"amount", "from", "to"},
    "random.dice": {"count", "sides", "modifier"},
    "random.coin": {"count"},
    "random.pick": {"options"},
    "random.number": {"low", "high", "count", "distinct"},
    "define.word": {"word", "sense"},
    "relations.is_a": {"subject", "class", "time"},
    "relations.kinds": {"class", "depth", "time"},
    "relations.parts": {"whole", "part", "depth"},
    "relations.part_of": {"part", "whole"},
    "relations.causes": {"topic", "direction", "other"},
    "relations.about": {"topic"},
    "relations.provenance": {"claim", "numeric"},
    "teach.assert": {"statement"},
    "teach.forget": {"target"},
    "teach.list": {"topic", "period", "kind"},
    "teach.phrase": {"phrase", "meaning"},
    "translate.word": {"term", "target", "sense"},
    "preferences.set": {"key", "value"},
    "preferences.get": {"key"},
    "preferences.forget": {"key"},
}
errors = []


def err(msg):
    errors.append(msg)


def catalogue():
    text = SPEC.read_text(encoding="utf8")
    section = text.split("## Intent catalogue", 1)[1].split("\n## ", 1)[0]
    ids = re.findall(r"^\|\s*`([a-z_]+\.[a-z_]+)`\s*\|", section, re.M)
    if not ids:
        sys.exit(f"no intents parsed from {SPEC}")
    return ids


def norm(s):
    return " ".join(s.casefold().split())


def load(name):
    path = HERE / name
    rows = []
    if not path.exists():
        err(f"{name}: missing")
        return rows
    for n, line in enumerate(path.read_text(encoding="utf8").splitlines(), 1):
        if not line.strip():
            err(f"{name}:{n}: blank line")
            continue
        try:
            obj = json.loads(line)
        except ValueError as e:
            err(f"{name}:{n}: invalid JSON ({e})")
            continue
        if not isinstance(obj, dict):
            err(f"{name}:{n}: not an object")
            continue
        obj["_src"] = f"{name}:{n}"
        rows.append(obj)
    return rows


def check_trace(obj, where, status):
    tr = obj.get("trace")
    if tr is None:
        return
    if status == "deflected" and tr not in DEFLECTED_SHAPES:
        err(f"{where}: trace {tr!r} is not a reflector shape")
    elif status == "unhandled" and tr not in UNHANDLED_REASONS:
        err(f"{where}: trace {tr!r} is not an unhandled reason")
    elif status in ("answered", "asks"):
        err(f"{where}: trace is only for deflected and unhandled outcomes")


def need(row, fields):
    for f in fields:
        if f not in row:
            err(f"{row['_src']}: missing field '{f}'")
            return False
    return True


def main():
    cat = catalogue()
    catset = set(cat)
    seen_ids = {}
    texts = defaultdict(list)
    summary = []

    def reg_id(row):
        i = row.get("id")
        if not isinstance(i, str) or not i:
            err(f"{row['_src']}: bad id")
        elif i in seen_ids:
            err(f"{row['_src']}: duplicate id {i} (also {seen_ids[i]})")
        else:
            seen_ids[i] = row["_src"]

    def reg_text(row):
        t = row.get("text")
        if not isinstance(t, str) or not t.strip():
            err(f"{row['_src']}: empty text")
            return
        texts[norm(t)].append(row["_src"])

    per_intent = {lang: Counter() for lang in LANGS}
    for lang in LANGS:
        # intents
        name = f"intents.{lang}.jsonl"
        rows = load(name)
        for r in rows:
            reg_id(r)
            if not need(r, ["id", "text", "lang", "intent"]):
                continue
            reg_text(r)
            if r["lang"] != lang:
                err(f"{r['_src']}: lang {r['lang']!r} != {lang!r}")
            if r["intent"] not in catset:
                err(f"{r['_src']}: unknown intent {r['intent']!r}")
            else:
                per_intent[lang][r["intent"]] += 1
            if "slots" in r:
                if not isinstance(r["slots"], dict):
                    err(f"{r['_src']}: slots must be an object")
                else:
                    extra = set(r["slots"]) - SLOTS.get(r["intent"], set())
                    if extra:
                        err(f"{r['_src']}: slot(s) {sorted(extra)} not defined for {r['intent']}")
            if "tags" in r and not (isinstance(r["tags"], list) and all(isinstance(t, str) for t in r["tags"])):
                err(f"{r['_src']}: tags must be a list of strings")
        for intent in cat:
            if per_intent[lang][intent] < MIN_PER_INTENT:
                err(f"{name}: {intent} has {per_intent[lang][intent]} (< {MIN_PER_INTENT})")
        summary.append((name, len(rows), f"{len(per_intent[lang])}/{len(cat)} intents, min {min(per_intent[lang].values(), default=0)}"))

        # reject
        name = f"reject.{lang}.jsonl"
        rows = load(name)
        kinds = Counter()
        for r in rows:
            reg_id(r)
            if not need(r, ["id", "text", "lang", "intent", "kind", "status"]):
                continue
            reg_text(r)
            if r["lang"] != lang:
                err(f"{r['_src']}: lang {r['lang']!r} != {lang!r}")
            if r["intent"] is not None:
                err(f"{r['_src']}: intent must be null")
            if r["kind"] not in KINDS:
                err(f"{r['_src']}: invalid kind {r['kind']!r}")
            else:
                kinds[r["kind"]] += 1
            if r["status"] not in STATUSES:
                err(f"{r['_src']}: invalid status {r['status']!r}")
            check_trace(r, r["_src"], r["status"])
        if len(rows) < MIN_REJECT:
            err(f"{name}: {len(rows)} rows (< {MIN_REJECT})")
        if kinds["near_miss"] < MIN_NEAR_MISS:
            err(f"{name}: {kinds['near_miss']} near_miss (< {MIN_NEAR_MISS})")
        for k in KINDS:
            if kinds[k] == 0:
                err(f"{name}: no rows of kind {k}")
        summary.append((name, len(rows), ", ".join(f"{k}={kinds[k]}" for k in sorted(KINDS))))

        # conversations
        name = f"conversations.{lang}.jsonl"
        rows = load(name)
        nturns = 0
        for r in rows:
            reg_id(r)
            if not need(r, ["id", "lang", "turns"]):
                continue
            if r["lang"] != lang:
                err(f"{r['_src']}: lang {r['lang']!r} != {lang!r}")
            if not isinstance(r["turns"], list) or not r["turns"]:
                err(f"{r['_src']}: turns must be a non-empty list")
                continue
            if "given" in r and not isinstance(r["given"], str):
                err(f"{r['_src']}: given must be a string")
            prev = None
            for i, t in enumerate(r["turns"], 1):
                where = f"{r['_src']} turn {i}"
                if not isinstance(t, dict) or not isinstance(t.get("user"), str) or not t["user"].strip():
                    err(f"{where}: bad 'user'")
                    prev = None
                    continue
                e = t.get("expect")
                if not isinstance(e, dict) or "status" not in e or "intent" not in e:
                    err(f"{where}: expect needs status and intent")
                    prev = None
                    continue
                nturns += 1
                st, it = e["status"], e["intent"]
                if st not in TURN_STATUSES:
                    err(f"{where}: invalid status {st!r}")
                if it is not None and it not in catset:
                    err(f"{where}: unknown intent {it!r}")
                if st == "deflected" and it is not None:
                    err(f"{where}: deflected must have intent null")
                if st == "asks":
                    if it is None:
                        err(f"{where}: asks needs the pending intent")
                    if not isinstance(e.get("slot"), str) or not e["slot"]:
                        err(f"{where}: asks needs the 'slot' asked for")
                elif "slot" in e:
                    err(f"{where}: 'slot' is only for asks")
                if st == "answered" and it is None and not (e.get("cancel") or e.get("ack")):
                    err(f"{where}: answered needs an intent (or cancel / ack: true)")
                if (e.get("cancel") or e.get("ack")) and (st != "answered" or it is not None):
                    err(f"{where}: cancel/ack mean answered with intent null")
                if e.get("cancel") and e.get("ack"):
                    err(f"{where}: cancel and ack are exclusive")
                open_q = prev is not None and prev["status"] == "asks"
                if e.get("follow_up"):
                    if not open_q:
                        err(f"{where}: follow_up needs the previous turn to be 'asks'")
                    elif it != prev["intent"] or st != "answered":
                        err(f"{where}: follow_up must answer the pending intent {prev['intent']!r}")
                if e.get("cancel") and not open_q:
                    err(f"{where}: cancel needs an open question (otherwise it is an ack)")
                check_trace(e, where, st)
                prev = e
        if len(rows) < MIN_CONVERSATIONS:
            err(f"{name}: {len(rows)} conversations (< {MIN_CONVERSATIONS})")
        summary.append((name, len(rows), f"{nturns} turns"))

    for t, where in texts.items():
        if len(where) > 1:
            err(f"duplicate text {t!r}: {', '.join(where)}")

    w = max(len(s[0]) for s in summary)
    print(f"{'file':<{w}}  rows  detail")
    for name, n, detail in summary:
        print(f"{name:<{w}}  {n:>4}  {detail}")
    for lang in LANGS:
        tot = sum(per_intent[lang].values())
        print(f"{lang}: {tot} intent phrases over {len(cat)} catalogue intents")
    if errors:
        print(f"\nFAIL: {len(errors)} problem(s)")
        for e in errors[:100]:
            print("  -", e)
        sys.exit(1)
    print("\nOK")


if __name__ == "__main__":
    main()
