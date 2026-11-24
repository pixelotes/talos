use crate::data::*;
use rusqlite::{params, types::ToSql, Connection};
use std::collections::HashSet;
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct Cfg {
    pub layout: char,
    pub cache_kib: Option<i64>,
    pub mmap_mb: i64,
    pub wal: bool,
    pub sync: String,
    pub immutable: bool,
    pub runs: usize,
    pub edge: bool,
    pub root: PathBuf,
}
impl Cfg {
    pub fn n(&self) -> usize {
        match self.layout {
            'A' => 0,
            'B' => 1,
            'C' => 3,
            _ => 8,
        }
    }
    pub fn overlay(&self) -> bool {
        self.n() > 0
    }
    pub fn dbs(&self) -> Vec<String> {
        let mut v: Vec<String> = (1..=self.n()).map(|i| format!("p{}", i)).collect();
        v.push("main".into());
        v
    }
    pub fn data_dir(&self) -> PathBuf {
        self.root.join("data").join(self.layout.to_string())
    }
    pub fn label(&self) -> String {
        format!(
            "cache={},mmap={}M,wal={},sync={},imm={}",
            self.cache_kib.map(|k| k.to_string()).unwrap_or("def".into()),
            self.mmap_mb,
            self.wal as u8,
            self.sync,
            self.immutable as u8
        )
    }
}

pub fn maxrss_mb() -> f64 {
    unsafe {
        let mut ru: libc::rusage = std::mem::zeroed();
        libc::getrusage(libc::RUSAGE_SELF, &mut ru);
        ru.ru_maxrss as f64 / 1048576.0 // bytes on macOS
    }
}

pub fn work_db(cfg: &Cfg) -> PathBuf {
    cfg.root.join("work").join(format!("{}.db", cfg.layout))
}

/// Fresh writable copy of user.db (or merged DB) so writes never touch the master files.
pub fn prepare_work(cfg: &Cfg) {
    let src = if cfg.layout == 'A' { cfg.data_dir().join("merged.db") } else { cfg.data_dir().join("user.db") };
    std::fs::create_dir_all(cfg.root.join("work")).unwrap();
    let dst = work_db(cfg);
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{}", dst.display(), ext));
    }
    std::fs::copy(src, &dst).unwrap();
}

fn views_sql(cfg: &Cfg) -> String {
    let dbs = cfg.dbs();
    let overlay = cfg.overlay();
    let mut out = String::new();
    for (view, cols, tbl) in [
        ("concept_all", "id,kind", "concept"),
        ("lexeme_all", "id,lang,lemma,pos", "lexeme"),
        ("form_all", "lexeme_id,form,features", "form"),
        ("sense_all", "id,lexeme_id,concept_id,gloss,rank,source_id", "sense"),
        ("claim_all", "id,subject,predicate,object,literal,negated,confidence,source_id,valid_from,valid_to", "claim"),
    ] {
        let parts: Vec<String> = dbs
            .iter()
            .map(|db| {
                if tbl == "claim" {
                    let mut w = "retracted_at IS NULL".to_string();
                    if overlay && db != "main" {
                        w += " AND NOT EXISTS (SELECT 1 FROM main.tombstone t WHERE t.claim_id = c.id)";
                    }
                    format!("SELECT {cols} FROM {db}.claim c WHERE {w}")
                } else {
                    format!("SELECT {cols} FROM {db}.{tbl}")
                }
            })
            .collect();
        out.push_str(&format!("CREATE TEMP VIEW {view} AS {};\n", parts.join(" UNION ALL ")));
    }
    out
}

pub fn open(cfg: &Cfg) -> Connection {
    let c = Connection::open(work_db(cfg)).unwrap();
    c.set_prepared_statement_cache_capacity(256);
    let mut pr = String::new();
    pr.push_str(&format!("PRAGMA foreign_keys=OFF;PRAGMA journal_mode={};PRAGMA synchronous={};", if cfg.wal { "WAL" } else { "DELETE" }, cfg.sync));
    if let Some(k) = cfg.cache_kib {
        pr.push_str(&format!("PRAGMA main.cache_size=-{};", k));
    }
    if cfg.mmap_mb > 0 {
        pr.push_str(&format!("PRAGMA main.mmap_size={};", cfg.mmap_mb * 1048576));
    }
    c.execute_batch(&pr).unwrap();
    for i in 1..=cfg.n() {
        let f = cfg.data_dir().join(format!("pack{}.talospack", i));
        let uri = format!("file:{}?mode=ro{}", f.display(), if cfg.immutable { "&immutable=1" } else { "" });
        let mut s = format!("ATTACH '{}' AS p{};", uri, i);
        if let Some(k) = cfg.cache_kib {
            s.push_str(&format!("PRAGMA p{}.cache_size=-{};", i, k));
        }
        if cfg.mmap_mb > 0 {
            s.push_str(&format!("PRAGMA p{}.mmap_size={};", i, cfg.mmap_mb * 1048576));
        }
        c.execute_batch(&s).unwrap();
    }
    c.execute_batch(&views_sql(cfg)).unwrap();
    c
}

pub fn build_edge(c: &Connection) -> Duration {
    let t = Instant::now();
    c.execute_batch(
        "CREATE TEMP TABLE edge AS SELECT subject, predicate, object FROM claim_all WHERE predicate IN (1,2) AND object IS NOT NULL AND negated=0;
         CREATE INDEX temp.edge_f ON edge(subject, predicate);
         CREATE INDEX temp.edge_r ON edge(object, predicate);",
    )
    .unwrap();
    t.elapsed()
}

// ------------------------------------------------------------------ helpers
fn q(c: &Connection, sql: &str, p: &[&dyn ToSql]) -> usize {
    let mut st = c.prepare_cached(sql).unwrap();
    let mut rows = st.query(p).unwrap();
    let mut n = 0;
    while let Some(r) = rows.next().unwrap() {
        black_box(r.get_ref_unwrap(0));
        n += 1;
    }
    n
}

#[derive(Clone, Copy, PartialEq)]
pub enum Src {
    View,
    Branch,
    Edge,
}

fn cte_sql(isa: bool, src: Src, cfg: &Cfg) -> String {
    let (key, out) = if isa { ("subject", "object") } else { ("object", "subject") };
    let steps: Vec<String> = match src {
        Src::View => vec![format!(
            "SELECT cl.{out}, r.d+1 FROM r JOIN claim_all cl ON cl.{key}=r.c AND cl.predicate=1 AND cl.{out} IS NOT NULL AND cl.negated=0 WHERE r.d<?3"
        )],
        Src::Branch => cfg
            .dbs()
            .iter()
            .map(|db| {
                let tomb = if cfg.overlay() && db != "main" { " AND NOT EXISTS (SELECT 1 FROM main.tombstone t WHERE t.claim_id=cl.id)" } else { "" };
                format!("SELECT cl.{out}, r.d+1 FROM r JOIN {db}.claim cl ON cl.{key}=r.c AND cl.predicate=1 AND cl.{out} IS NOT NULL AND cl.negated=0 AND cl.retracted_at IS NULL{tomb} WHERE r.d<?3")
            })
            .collect(),
        Src::Edge => vec![format!("SELECT e.{out}, r.d+1 FROM r JOIN temp.edge e ON e.{key}=r.c AND e.predicate=1 WHERE r.d<?3")],
    };
    let fin = if isa { "SELECT 1 FROM r WHERE c=?2 LIMIT 1" } else { "SELECT DISTINCT c FROM r WHERE d>0 AND ?2=?2" };
    format!("WITH RECURSIVE r(c,d) AS (SELECT ?1,0 UNION {}) {}", steps.join(" UNION "), fin)
}

fn bfs_sqls(cfg: &Cfg, forward: bool) -> Vec<String> {
    let (key, out) = if forward { ("subject", "object") } else { ("object", "subject") };
    cfg.dbs()
        .iter()
        .map(|db| {
            let tomb = if cfg.overlay() && db != "main" { " AND NOT EXISTS (SELECT 1 FROM main.tombstone t WHERE t.claim_id=cl.id)" } else { "" };
            format!("SELECT cl.{out} FROM {db}.claim cl WHERE cl.{key}=?1 AND cl.predicate=1 AND cl.{out} IS NOT NULL AND cl.negated=0 AND cl.retracted_at IS NULL{tomb}")
        })
        .collect()
}

/// Rust-side bounded BFS querying each database separately and merging.
fn bfs(c: &Connection, sqls: &[String], start: &str, target: Option<&str>, bound: usize) -> usize {
    let mut seen: HashSet<String> = HashSet::new();
    seen.insert(start.to_string());
    let mut frontier = vec![start.to_string()];
    for _ in 0..bound {
        let mut next = vec![];
        for node in &frontier {
            for sql in sqls {
                let mut st = c.prepare_cached(sql).unwrap();
                let mut rows = st.query([node]).unwrap();
                while let Some(r) = rows.next().unwrap() {
                    let o: String = r.get(0).unwrap();
                    if Some(o.as_str()) == target {
                        return 1;
                    }
                    if seen.insert(o.clone()) {
                        next.push(o);
                    }
                }
            }
        }
        if next.is_empty() {
            break;
        }
        frontier = next;
    }
    if target.is_some() { 0 } else { seen.len() - 1 }
}

pub type F = Box<dyn Fn(&Connection, usize) -> usize>;
pub struct Case {
    pub name: &'static str,
    pub variant: &'static str,
    pub sqls: Vec<String>,
    pub f: F,
}

fn fts_sql(cfg: &Cfg) -> String {
    let parts: Vec<String> = cfg
        .dbs()
        .iter()
        .map(|db| format!("SELECT * FROM (SELECT rowid AS id, rank FROM {db}.gloss_fts WHERE gloss_fts MATCH ?1 ORDER BY rank LIMIT 20)"))
        .collect();
    format!("SELECT id FROM ({}) ORDER BY rank LIMIT 20", parts.join(" UNION ALL "))
}

pub fn cases(cfg: &Cfg, s: &Samples) -> Vec<Case> {
    let mut v: Vec<Case> = vec![];
    macro_rules! case {
        ($name:expr, $var:expr, $sqls:expr, $f:expr) => {
            v.push(Case { name: $name, variant: $var, sqls: $sqls, f: Box::new($f) })
        };
    }
    // --- lemma -> senses
    let sa = "SELECT s.concept_id, s.gloss, s.rank, l.pos FROM lexeme_all l JOIN sense_all s ON s.lexeme_id=l.id WHERE l.lang=?1 AND l.lemma=?2 ORDER BY s.rank";
    let sb = "SELECT s.concept_id, s.gloss, s.rank FROM sense_all s WHERE s.lexeme_id IN (SELECT id FROM lexeme_all WHERE lang=?1 AND lemma=?2) ORDER BY s.rank";
    let d = s["lemma"].clone();
    case!("lemma->senses", "join", vec![sa.into()], move |c, i| q(c, sa, params![d[i % d.len()][0], d[i % d.len()][1]]));
    let d = s["lemma"].clone();
    case!("lemma->senses", "in-subquery", vec![sb.into()], move |c, i| q(c, sb, params![d[i % d.len()][0], d[i % d.len()][1]]));
    // --- form -> lemma
    let fa = "SELECT l.lemma, l.pos, l.lang, f.features FROM form_all f JOIN lexeme_all l ON l.id=f.lexeme_id WHERE f.form=?1";
    let fb = "SELECT lemma, pos, lang FROM lexeme_all WHERE id IN (SELECT lexeme_id FROM form_all WHERE form=?1)";
    let d = s["form"].clone();
    case!("form->lemma", "join", vec![fa.into()], move |c, i| q(c, fa, params![d[i % d.len()][0]]));
    let d = s["form"].clone();
    case!("form->lemma", "in-subquery", vec![fb.into()], move |c, i| q(c, fb, params![d[i % d.len()][0]]));
    // --- is_a and kinds-of through the 4 mechanisms
    let mut srcs = vec![("view", Src::View), ("branch", Src::Branch), ("rust", Src::Edge /*placeholder*/)];
    if cfg.edge {
        srcs.push(("edge", Src::Edge));
    }
    let _ = &mut srcs;
    for (name, key, bound) in [("isa<=6", "isa6", 6i64), ("isa deep(<=16)", "isa_deep", 16i64)] {
        for (var, src) in [("view", Src::View), ("branch", Src::Branch), ("edge", Src::Edge), ("rust", Src::View)] {
            if var == "edge" && !cfg.edge {
                continue;
            }
            if var == "branch" && cfg.n() == 0 {
                continue;
            }
            let d = s[key].clone();
            if var == "rust" {
                let sq = bfs_sqls(cfg, true);
                case!(name, var, vec![], move |c, i| bfs(c, &sq, &d[i % d.len()][0], Some(&d[i % d.len()][1]), bound as usize));
            } else {
                let sql = cte_sql(true, src, cfg);
                let s2 = sql.clone();
                case!(name, var, vec![sql], move |c, i| q(c, &s2, params![d[i % d.len()][0], d[i % d.len()][1], bound]));
            }
        }
    }
    if cfg.overlay() {
        for (var, src) in [("view", Src::View), ("branch", Src::Branch), ("edge", Src::Edge), ("rust", Src::View)] {
            if var == "edge" && !cfg.edge {
                continue;
            }
            let d = s["isa_user"].clone();
            if var == "rust" {
                let sq = bfs_sqls(cfg, true);
                case!("isa<=6 (user concept)", var, vec![], move |c, i| bfs(c, &sq, &d[i % d.len()][0], Some(&d[i % d.len()][1]), 6));
            } else {
                let sql = cte_sql(true, src, cfg);
                let s2 = sql.clone();
                case!("isa<=6 (user concept)", var, vec![sql], move |c, i| q(c, &s2, params![d[i % d.len()][0], d[i % d.len()][1], 6i64]));
            }
        }
    }
    // kinds-of
    let k1 = "SELECT subject FROM claim_all WHERE object=?1 AND predicate=1 AND negated=0";
    let d = s["kinds"].clone();
    case!("kinds-of 1 level", "view", vec![k1.into()], move |c, i| q(c, k1, params![d[i % d.len()][0]]));
    let d = s["hubs"].clone();
    case!("kinds-of 1 level (hub)", "view", vec![k1.into()], move |c, i| q(c, k1, params![d[i % d.len()][0]]));
    for (name, key) in [("kinds-of 3 levels", "kinds"), ("kinds-of 3 levels (hub)", "hubs")] {
        for (var, src) in [("view", Src::View), ("branch", Src::Branch), ("edge", Src::Edge), ("rust", Src::View)] {
            if var == "edge" && !cfg.edge {
                continue;
            }
            if var == "branch" && cfg.n() == 0 {
                continue;
            }
            let d = s[key].clone();
            if var == "rust" {
                let sq = bfs_sqls(cfg, false);
                case!(name, var, vec![], move |c, i| bfs(c, &sq, &d[i % d.len()][0], None, 3));
            } else {
                let sql = cte_sql(false, src, cfg);
                let s2 = sql.clone();
                case!(name, var, vec![sql], move |c, i| q(c, &s2, params![d[i % d.len()][0], "", 3i64]));
            }
        }
    }
    // part_of
    let pf = "SELECT object FROM claim_all WHERE subject=?1 AND predicate=2 AND object IS NOT NULL AND negated=0";
    let pr = "SELECT subject FROM claim_all WHERE object=?1 AND predicate=2 AND negated=0";
    let d = s["part_fwd"].clone();
    case!("part_of forward (X is part of?)", "view", vec![pf.into()], move |c, i| q(c, pf, params![d[i % d.len()][0]]));
    let d = s["part_rev"].clone();
    case!("part_of reverse (parts of X)", "view", vec![pr.into()], move |c, i| q(c, pr, params![d[i % d.len()][0]]));
    // neighbourhood
    let n_out = "SELECT predicate, object, literal, confidence FROM claim_all WHERE subject=?1";
    let n_in = "SELECT subject, predicate, confidence FROM claim_all WHERE object=?1";
    let n_w1 = "SELECT l.lang, l.lemma, s.gloss FROM sense_all s JOIN lexeme_all l ON l.id=s.lexeme_id WHERE s.concept_id=?1";
    let n_w2 = "SELECT lang, lemma FROM lexeme_all WHERE id IN (SELECT lexeme_id FROM sense_all WHERE concept_id=?1)";
    let d = s["neigh"].clone();
    case!("neighbourhood", "join", vec![n_out.into(), n_in.into(), n_w1.into()], move |c, i| {
        let x = &d[i % d.len()][0];
        q(c, n_out, params![x]) + q(c, n_in, params![x]) + q(c, n_w1, params![x])
    });
    let d = s["neigh"].clone();
    case!("neighbourhood", "in-subquery", vec![n_out.into(), n_in.into(), n_w2.into()], move |c, i| {
        let x = &d[i % d.len()][0];
        q(c, n_out, params![x]) + q(c, n_in, params![x]) + q(c, n_w2, params![x])
    });
    // FTS
    let fsql = fts_sql(cfg);
    let (f1, f2) = (fsql.clone(), fsql.clone());
    let d = s["fts"].clone();
    case!("fts gloss top-20", "ids", vec![fsql.clone()], move |c, i| q(c, &f1, params![d[i % d.len()][0]]));
    let d = s["fts"].clone();
    let rs = "SELECT concept_id, gloss FROM sense_all WHERE id=?1";
    case!("fts gloss top-20 + resolve 10", "ids+view", vec![fsql, rs.into()], move |c, i| {
        let mut ids: Vec<i64> = vec![];
        {
            let mut st = c.prepare_cached(&f2).unwrap();
            let mut rows = st.query(params![d[i % d.len()][0]]).unwrap();
            while let Some(r) = rows.next().unwrap() {
                ids.push(r.get(0).unwrap());
            }
        }
        let mut n = 0;
        for id in ids.iter().take(10) {
            n += q(c, rs, params![id]);
        }
        n
    });
    // teach (write)
    let d = s["concepts"].clone();
    let ins = "INSERT INTO main.claim(subject,predicate,object,negated,confidence,source_id,created_at) VALUES (?1,2,?2,0,0.9,1,?3)";
    let aud = "INSERT INTO main.audit(at,action,claim_id,detail) VALUES (?1,'teach',last_insert_rowid(),'')";
    case!("teach (insert claim+audit, 1 txn)", "write", vec![ins.into(), aud.into()], move |c, i| {
        let r = &d[i % d.len()];
        c.execute_batch("BEGIN IMMEDIATE").unwrap();
        c.prepare_cached(ins).unwrap().execute(params![r[0], r[1], i as i64]).unwrap();
        c.prepare_cached(aud).unwrap().execute(params![i as i64]).unwrap();
        c.execute_batch("COMMIT").unwrap();
        1
    });
    v
}

// ------------------------------------------------------------------ stats
pub struct Stats {
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
    pub max: f64,
    pub mean: f64,
}
pub fn stats(mut v: Vec<Duration>) -> Stats {
    v.sort();
    let us = |d: Duration| d.as_nanos() as f64 / 1000.0;
    let p = |q: f64| us(v[(((v.len() - 1) as f64) * q).round() as usize]);
    Stats { p50: p(0.50), p95: p(0.95), p99: p(0.99), max: us(*v.last().unwrap()), mean: v.iter().map(|d| us(*d)).sum::<f64>() / v.len() as f64 }
}

pub fn emit(cfg: &Cfg, case: &str, var: &str, mode: &str, runs: usize, st: &Stats, rows: f64) {
    println!(
        "RESULT\t{}\t{}\t{}\t{}\t{}\t{}\t{:.1}\t{:.1}\t{:.1}\t{:.1}\t{:.1}\t{:.1}",
        cfg.layout, cfg.label(), case, var, mode, runs, st.p50, st.p95, st.p99, st.max, st.mean, rows
    );
}

fn run_case(cfg: &Cfg, c: &Connection, case: &Case, runs: usize) {
    let warm = (runs / 10).max(50).min(300);
    // pilot to guard against pathologically slow variants
    let t = Instant::now();
    let pilot = 5;
    for i in 0..pilot {
        (case.f)(c, i);
    }
    let est = t.elapsed() / pilot as u32;
    let mut runs = runs;
    let mut warm = warm;
    if est > Duration::from_millis(20) {
        runs = ((Duration::from_secs(60).as_nanos() / est.as_nanos().max(1)) as usize).clamp(30, runs);
        warm = 5;
    }
    for i in 0..warm {
        (case.f)(c, i);
    }
    let mut ds = Vec::with_capacity(runs);
    let mut rows = 0usize;
    for i in 0..runs {
        let t = Instant::now();
        rows += black_box((case.f)(c, warm + i));
        ds.push(t.elapsed());
    }
    emit(cfg, case.name, case.variant, "warm", runs, &stats(ds), rows as f64 / runs as f64);
}

fn run_cold(cfg: &Cfg, case: &Case, runs: usize) {
    // "cold" = brand-new connection (empty SQLite page cache and statement cache) per run;
    // the OS page cache stays warm. Timed: prepare + first execution of the statement.
    let mut ds = Vec::with_capacity(runs);
    let mut rows = 0;
    for i in 0..runs {
        let c = open(cfg);
        if cfg.edge && case.variant == "edge" {
            build_edge(&c);
        }
        let t = Instant::now();
        rows += black_box((case.f)(&c, i * 7));
        ds.push(t.elapsed());
    }
    emit(cfg, case.name, case.variant, "conn-cold", runs, &stats(ds), rows as f64 / runs as f64);
}

pub fn suite(cfg: &Cfg, root: &Path, which: &str) {
    let s = read_samples(&root.join("data/samples.tsv"));
    println!("META\t{}\t{}\tbaseline_rss_mb\t{:.1}", cfg.layout, cfg.label(), maxrss_mb());
    if which == "baseline" {
        return;
    }
    prepare_work(cfg);
    if which == "open" {
        let mut ot = vec![];
        let mut ft = vec![];
        for _ in 0..300 {
            let t = Instant::now();
            let c = open(cfg);
            ot.push(t.elapsed());
            let t = Instant::now();
            let cs = cases(cfg, &s);
            let lem = cs.iter().find(|c| c.name == "lemma->senses" && c.variant == "join").unwrap();
            (lem.f)(&c, 1);
            ft.push(t.elapsed());
        }
        let st = stats(ot);
        emit(cfg, "open+attach+views", "", "open", 300, &st, 0.0);
        let st = stats(ft);
        emit(cfg, "first lemma query after open", "", "open", 300, &st, 0.0);
        println!("META\t{}\t{}\tpeak_rss_mb\t{:.1}", cfg.layout, cfg.label(), maxrss_mb());
        return;
    }
    let c = open(cfg);
    println!("META\t{}\t{}\trss_after_open_mb\t{:.1}", cfg.layout, cfg.label(), maxrss_mb());
    if cfg.edge {
        let d = build_edge(&c);
        let n: i64 = c.query_row("SELECT count(*) FROM temp.edge", [], |r| r.get(0)).unwrap();
        println!("META\t{}\t{}\tedge_build_ms\t{:.1}\tedge_rows\t{}", cfg.layout, cfg.label(), d.as_secs_f64() * 1000.0, n);
    }
    let cs = cases(cfg, &s);
    match which {
        "all" | "edge" => {
            // verification: variants of the same case must agree
            if which == "edge" || which == "all" {
                for name in ["isa<=6", "isa deep(<=16)", "kinds-of 3 levels", "kinds-of 3 levels (hub)", "isa<=6 (user concept)"] {
                    let mut vs: Vec<&Case> = cs.iter().filter(|c| c.name == name).collect();
                    vs.sort_by_key(|c| if c.variant == "rust" { 0 } else if c.variant == "branch" { 1 } else { 2 });
                    if vs.len() < 2 {
                        continue;
                    }
                    let n = if name.contains("hub") { 10 } else { 60 };
                    let base: Vec<usize> = (0..n).map(|i| (vs[0].f)(&c, i)).collect();
                    for o in &vs[1..] {
                        let mism = (0..n).filter(|&i| (o.f)(&c, i) != base[i]).count();
                        println!("META\t{}\t{}\tverify {} {} vs {}\tmismatches\t{}/{}", cfg.layout, cfg.label(), name, o.variant, vs[0].variant, mism, n);
                    }
                }
            }
            for case in &cs {
                if case.variant == "write" {
                    continue;
                }
                if which == "edge" && case.variant != "edge" {
                    continue;
                }
                run_case(cfg, &c, case, cfg.runs);
            }
            println!("META\t{}\t{}\tpeak_rss_mb\t{:.1}", cfg.layout, cfg.label(), maxrss_mb());
        }
        "quick" => {
            for case in &cs {
                let keep = matches!(
                    (case.name, case.variant),
                    ("lemma->senses", "in-subquery") | ("kinds-of 1 level", "view") | ("fts gloss top-20", "ids") | ("neighbourhood", "in-subquery")
                ) || (case.name == "isa<=6" && (case.variant == "branch" || (cfg.n() == 0 && case.variant == "view")));
                if keep {
                    run_case(cfg, &c, case, cfg.runs);
                }
            }
            println!("META\t{}\t{}\tpeak_rss_mb\t{:.1}", cfg.layout, cfg.label(), maxrss_mb());
        }
        "write" => {
            for case in &cs {
                if case.variant == "write" {
                    run_case(cfg, &c, case, cfg.runs);
                }
            }
        }
        "cold" => {
            drop(c);
            for case in &cs {
                let keep = matches!(
                    (case.name, case.variant),
                    ("lemma->senses", "join") | ("form->lemma", "join") | ("isa<=6", "view") | ("kinds-of 1 level", "view") | ("neighbourhood", "join") | ("fts gloss top-20", "ids") | ("part_of reverse (parts of X)", "view")
                );
                if keep {
                    run_cold(cfg, case, cfg.runs);
                }
            }
        }
        "prepare" => {
            // statement preparation (planning) cost, not cached
            for case in &cs {
                if case.sqls.is_empty() || case.variant == "write" {
                    continue;
                }
                if case.variant == "edge" {
                    continue;
                }
                let mut ds = vec![];
                for _ in 0..300 {
                    let t = Instant::now();
                    for sql in &case.sqls {
                        black_box(c.prepare(sql).unwrap());
                    }
                    ds.push(t.elapsed());
                }
                emit(cfg, case.name, case.variant, "prepare", 300, &stats(ds), 0.0);
            }
        }
        "plans" => {
            for case in &cs {
                for sql in &case.sqls {
                    println!("PLAN\t{}\t{} [{}]\t{}", cfg.layout, case.name, case.variant, sql);
                    let mut st = c.prepare(&format!("EXPLAIN QUERY PLAN {}", sql)).unwrap();
                    let np = st.parameter_count();
                    let mut rows = st.query(rusqlite::params_from_iter(vec![rusqlite::types::Null; np])).unwrap();
                    while let Some(r) = rows.next().unwrap() {
                        let id: i64 = r.get(0).unwrap();
                        let parent: i64 = r.get(1).unwrap();
                        let det: String = r.get(3).unwrap();
                        println!("PLANROW\t{}\t{}\t{}", id, parent, det);
                    }
                }
            }
        }
        _ => panic!("unknown suite"),
    }
}
