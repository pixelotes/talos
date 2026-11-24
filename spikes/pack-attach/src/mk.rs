//! Writes the SQLite files: merged DB, packs, user.db.
use crate::data::*;
use rusqlite::{params, Connection};
use std::path::Path;

const SCHEMA: &str = "
CREATE TABLE source (id INTEGER PRIMARY KEY, kind TEXT NOT NULL, name TEXT NOT NULL, license TEXT, url TEXT, added_at INTEGER NOT NULL);
CREATE TABLE concept (id TEXT PRIMARY KEY, kind TEXT);
CREATE TABLE lexeme (id INTEGER PRIMARY KEY, lang TEXT NOT NULL, lemma TEXT NOT NULL, pos TEXT, UNIQUE (lang, lemma, pos));
CREATE TABLE form (lexeme_id INTEGER NOT NULL REFERENCES lexeme(id), form TEXT NOT NULL, features TEXT);
CREATE TABLE sense (id INTEGER PRIMARY KEY, lexeme_id INTEGER NOT NULL REFERENCES lexeme(id), concept_id TEXT NOT NULL REFERENCES concept(id), gloss TEXT, rank INTEGER NOT NULL DEFAULT 0, source_id INTEGER NOT NULL REFERENCES source(id));
CREATE TABLE predicate (id INTEGER PRIMARY KEY, name TEXT UNIQUE NOT NULL, inverse TEXT, transitive INTEGER NOT NULL DEFAULT 0, symmetric INTEGER NOT NULL DEFAULT 0, functional INTEGER NOT NULL DEFAULT 0);
CREATE TABLE claim (id INTEGER PRIMARY KEY, subject TEXT NOT NULL REFERENCES concept(id), predicate INTEGER NOT NULL REFERENCES predicate(id), object TEXT REFERENCES concept(id), literal TEXT, negated INTEGER NOT NULL DEFAULT 0, confidence REAL NOT NULL, source_id INTEGER NOT NULL REFERENCES source(id), valid_from INTEGER, valid_to INTEGER, created_at INTEGER NOT NULL, retracted_at INTEGER);
CREATE VIRTUAL TABLE gloss_fts USING fts5(lemma, gloss, content='');
CREATE TABLE audit (id INTEGER PRIMARY KEY, at INTEGER NOT NULL, action TEXT NOT NULL, claim_id INTEGER, detail TEXT);
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
";
// Indexes: KNOWLEDGE.md section 2 lists form_lookup, claim_forward, claim_reverse only.
// sense(lexeme_id) and sense(concept_id) are NOT in the documented schema but are required
// by lemma->senses and concept->words; the spike adds them (see report).
const INDEXES: &str = "
CREATE INDEX form_lookup ON form(form);
CREATE INDEX claim_forward ON claim(subject, predicate);
CREATE INDEX claim_reverse ON claim(object, predicate);
CREATE INDEX sense_lexeme ON sense(lexeme_id);
CREATE INDEX sense_concept ON sense(concept_id);
";
const TOMB: &str = "CREATE TABLE tombstone (claim_id INTEGER PRIMARY KEY, at INTEGER NOT NULL);";

fn base(path: &Path, tomb: bool) -> Connection {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
    let c = Connection::open(path).unwrap();
    c.execute_batch("PRAGMA foreign_keys=OFF; PRAGMA journal_mode=OFF; PRAGMA synchronous=OFF; PRAGMA cache_size=-200000;").unwrap();
    c.execute_batch(SCHEMA).unwrap();
    if tomb {
        c.execute_batch(TOMB).unwrap();
    }
    c.execute_batch(
        "INSERT INTO predicate VALUES (1,'is_a','has_kind',1,0,0),(2,'part_of','has_part',1,0,0),(3,'antonym','antonym',0,1,0),(4,'causes','caused_by',1,0,0),(5,'has_property',NULL,0,0,0);",
    )
    .unwrap();
    c
}

fn finish(c: &Connection, wal: bool) {
    c.execute_batch(INDEXES).unwrap();
    c.execute_batch("INSERT INTO gloss_fts(gloss_fts) VALUES('optimize'); ANALYZE;").unwrap();
    if wal {
        c.execute_batch("PRAGMA journal_mode=WAL;").unwrap();
    } else {
        c.execute_batch("PRAGMA journal_mode=DELETE;").unwrap();
    }
}

/// Write the part of `d` owned by `pack` (concept->pack via `assign`).
pub fn write_pack(path: &Path, d: &Data, assign: &[u8], pack: u8, source_id: i64, name: &str, writable: bool) {
    let c = base(path, writable);
    let tx = c.unchecked_transaction().unwrap();
    tx.execute("INSERT INTO source VALUES (?,?,?,?,?,?)", params![source_id, if writable { "import" } else { "pack" }, name, "CC-BY-synthetic", "", 1]).unwrap();
    tx.execute("INSERT INTO meta VALUES ('name',?),('version','1'),('langs','en,es'),('schema','1')", params![name]).unwrap();
    {
        let mut st = tx.prepare("INSERT INTO concept VALUES (?,?)").unwrap();
        for i in 0..d.cid.len() {
            if assign[i] == pack {
                st.execute(params![d.cid[i], if d.nchildren[i] > 0 { "class" } else { "entity" }]).unwrap();
            }
        }
        let mut st = tx.prepare("INSERT INTO claim VALUES (?,?,?,?,?,0,?,?,NULL,NULL,1700000000,NULL)").unwrap();
        for cl in &d.claims {
            if assign[cl.subject as usize] == pack {
                st.execute(params![cl.id, d.cid[cl.subject as usize], cl.pred, cl.object.map(|o| d.cid[o as usize].clone()), cl.literal, cl.conf, source_id]).unwrap();
            }
        }
        let owned = |li: u32| assign[d.lexemes[li as usize].first_concept as usize] == pack;
        let mut st = tx.prepare("INSERT INTO lexeme VALUES (?,?,?,?)").unwrap();
        for (li, l) in d.lexemes.iter().enumerate() {
            if owned(li as u32) {
                st.execute(params![l.id, lang_name(l.lang), l.lemma, l.pos]).unwrap();
            }
        }
        let mut st = tx.prepare("INSERT INTO form VALUES (?,?,?)").unwrap();
        for f in &d.forms {
            if owned(f.lex) {
                st.execute(params![d.lexemes[f.lex as usize].id, f.form, f.features]).unwrap();
            }
        }
        let mut st = tx.prepare("INSERT INTO sense VALUES (?,?,?,?,?,?)").unwrap();
        let mut fts = tx.prepare("INSERT INTO gloss_fts(rowid,lemma,gloss) VALUES (?,?,?)").unwrap();
        for s in &d.senses {
            if owned(s.lex) {
                let l = &d.lexemes[s.lex as usize];
                st.execute(params![s.id, l.id, d.cid[s.concept as usize], s.gloss, s.rank, source_id]).unwrap();
                if let Some(g) = &s.gloss {
                    fts.execute(params![s.id, l.lemma, g]).unwrap();
                }
            }
        }
    }
    tx.commit().unwrap();
    finish(&c, writable);
}

pub fn write_user(path: &Path, o: &Overlay) {
    let c = base(path, true);
    let tx = c.unchecked_transaction().unwrap();
    tx.execute("INSERT INTO source VALUES (1,'user','user',NULL,NULL,1)", []).unwrap();
    tx.execute("INSERT INTO meta VALUES ('name','user'),('schema','1')", []).unwrap();
    for u in &o.concepts {
        tx.execute("INSERT INTO concept VALUES (?, 'entity')", params![u]).unwrap();
    }
    for (id, lemma) in &o.lexemes {
        tx.execute("INSERT INTO lexeme VALUES (?,?,?,'n')", params![id, "en", lemma]).unwrap();
    }
    for (l, f) in &o.forms {
        tx.execute("INSERT INTO form VALUES (?,?,'')", params![l, f]).unwrap();
    }
    for (id, lex, c, g) in &o.senses {
        tx.execute("INSERT INTO sense VALUES (?,?,?,?,0,1)", params![id, lex, c, g]).unwrap();
        tx.execute("INSERT INTO gloss_fts(rowid,lemma,gloss) VALUES (?,?,?)", params![id, "x", g]).unwrap();
    }
    for (id, s, p, ob, lit) in &o.claims {
        tx.execute("INSERT INTO claim VALUES (?,?,?,?,?,0,0.9,1,NULL,NULL,1700000000,NULL)", params![id, s, p, ob, lit]).unwrap();
        tx.execute("INSERT INTO audit(at,action,claim_id,detail) VALUES (1700000000,'teach',?,'')", params![id]).unwrap();
    }
    for t in &o.tombs {
        tx.execute("INSERT INTO tombstone VALUES (?,1700000000)", params![t]).unwrap();
    }
    tx.commit().unwrap();
    finish(&c, true);
}
