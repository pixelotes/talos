#!/usr/bin/env python3
"""Turn out/*.tsv into markdown tables (used to write docs/research/pack-attachment.md)."""
import glob, os, collections, sys

here = os.path.dirname(os.path.abspath(__file__))
out = os.path.join(here, "out")
L = "ABCD"


def rows(pattern):
    res = []
    for f in sorted(glob.glob(os.path.join(out, pattern))):
        for line in open(f):
            p = line.rstrip("\n").split("\t")
            if p[0] == "RESULT":
                res.append(dict(layout=p[1], label=p[2], case=p[3], var=p[4], mode=p[5], runs=int(p[6]),
                                p50=float(p[7]), p95=float(p[8]), p99=float(p[9]), mx=float(p[10]), mean=float(p[11]), rows=float(p[12])))
    return res


def fmt(us):
    return f"{us/1000:.1f} ms" if us >= 1000 else f"{us:.0f} µs" if us >= 100 else f"{us:.1f} µs"


def table(rs, title, key=lambda r: (r["case"], r["var"]), stat=("p50", "p99"), runs=True):
    data = collections.OrderedDict()
    for r in rs:
        data.setdefault(key(r), {})[r["layout"]] = r
    print(f"\n### {title}\n")
    print("| case / variant | " + " | ".join(f"{l} p50 / p99" for l in L) + (" | runs (A/B/C/D) |" if runs else " |"))
    print("|---|" + "---|" * (len(L) + (1 if runs else 0)))
    for k, d in data.items():
        cells = []
        for l in L:
            r = d.get(l)
            cells.append("-" if not r else " / ".join(fmt(r[s]) for s in stat))
        rr = "/".join(str(d[l]["runs"]) if l in d else "-" for l in L)
        print(f"| {' / '.join(k)} | " + " | ".join(cells) + (f" | {rr} |" if runs else " |"))


warm = rows("all_*.tsv")
table(warm, "Warm (all variants)")
table(rows("edge_*.tsv"), "Edge-table variant (TEMP table of is_a/part_of edges)")
table(rows("cold_*.tsv"), "Connection-cold (fresh connection each run; includes prepare)")
table(rows("prepare_*.tsv"), "Statement preparation (planning) cost, not cached", stat=("p50", "p99"), runs=False)
op = rows("open_*.tsv")
table(op, "Open + attach + create views, and first lemma query after open", stat=("p50", "p99"), runs=False)
table(rows("write_*_normal.tsv"), "Teach write, synchronous=NORMAL (WAL)", stat=("p50", "p99"), runs=False)
table(rows("write_*_full.tsv"), "Teach write, synchronous=FULL (WAL)", stat=("p50", "p99"), runs=False)

print("\n### Memory / META\n")
for f in sorted(glob.glob(os.path.join(out, "*.tsv"))):
    for line in open(f):
        p = line.rstrip("\n").split("\t")
        if p[0] == "META" and any(k in line for k in ("peak_rss", "rss_after_open", "edge_build", "verify")) and "mismatches\t0/" not in line:
            print(os.path.basename(f), "|", " | ".join(p[1:]))

print("\n### Settings sweep (quick suite)\n")
st = rows("settings*.tsv")
by = collections.OrderedDict()
for r in st:
    by.setdefault((r["layout"], r["label"]), {})[(r["case"], r["var"])] = r
cases_ = sorted({k for d in by.values() for k in d})
print("| layout | setting | " + " | ".join(c[0] for c in cases_) + " |")
print("|---|---|" + "---|" * len(cases_))
for (lay, lab), d in by.items():
    print(f"| {lay} | {lab} | " + " | ".join(f"{fmt(d[c]['p50'])} / {fmt(d[c]['p99'])}" if c in d else "-" for c in cases_) + " |")
print()
for f in sorted(glob.glob(os.path.join(out, "settings*.tsv"))):
  for line in open(f):
    p = line.rstrip("\n").split("\t")
    if p[0] == "META" and "peak_rss" in line:
        print("RSS", p[1], p[2], p[4])
