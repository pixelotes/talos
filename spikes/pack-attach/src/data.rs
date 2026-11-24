//! Deterministic synthetic WordNet-like dataset.
use std::collections::{HashMap, HashSet};

pub struct Rng(pub u64);
impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
    pub fn f(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    pub fn below(&mut self, n: usize) -> usize {
        ((self.f() * n as f64) as usize).min(n - 1)
    }
    pub fn skew(&mut self, n: usize, pow: f64) -> usize {
        ((self.f().powf(pow) * n as f64) as usize).min(n - 1)
    }
}

const CONS: &[u8] = b"bcdfghjklmnprstvwxyz";
const VOW: &[u8] = b"aeiou";
const P: u64 = 99_999_989;
pub fn word(mut n: u64) -> String {
    let mut s = String::new();
    for _ in 0..4 {
        let k = (n % 100) as usize;
        n /= 100;
        s.push(CONS[k / 5] as char);
        s.push(VOW[k % 5] as char);
    }
    s
}
fn scr(i: u64, mul: u64) -> u64 {
    (i * mul) % P
}
/// Gloss vocabulary word for a rank (shared with the benchmark's FTS queries).
pub fn gloss_word(rank: u64, lang: u8) -> String {
    word(scr(rank + 1 + lang as u64 * 1_000_003, 31337))
}

pub const PRED_IS_A: i64 = 1;
pub const PRED_PART_OF: i64 = 2;
pub const PRED_ANTONYM: i64 = 3;
pub const PRED_CAUSES: i64 = 4;
pub const PRED_PROP: i64 = 5;

pub struct Claim {
    pub id: i64,
    pub subject: u32,
    pub pred: i64,
    pub object: Option<u32>,
    pub literal: Option<String>,
    pub conf: f64,
}
pub struct Lexeme {
    pub id: i64,
    pub lang: u8,
    pub lemma: String,
    pub pos: &'static str,
    pub first_concept: u32,
}
pub struct Sense {
    pub id: i64,
    pub lex: u32,
    pub concept: u32,
    pub gloss: Option<String>,
    pub rank: i32,
}
pub struct Form {
    pub lex: u32,
    pub form: String,
    pub features: &'static str,
}

pub struct Data {
    pub cid: Vec<String>,
    pub depth: Vec<u8>,
    pub parents: Vec<Vec<u32>>,
    pub nchildren: Vec<u32>,
    pub claims: Vec<Claim>,
    pub lexemes: Vec<Lexeme>,
    pub senses: Vec<Sense>,
    pub forms: Vec<Form>,
}

pub fn lang_name(l: u8) -> &'static str {
    if l == 0 { "en" } else { "es" }
}

pub fn generate(seed: u64, nc: usize) -> Data {
    let mut rng = Rng(seed);
    // ---- hierarchy: level sizes peaked around depth 7-9, max depth 15
    let w = [1.0, 8.0, 60.0, 400.0, 2000.0, 7000.0, 16000.0, 26000.0, 26000.0, 20000.0, 11000.0, 6000.0, 3000.0, 1500.0, 600.0, 300.0];
    let sum: f64 = w.iter().sum();
    let mut sizes: Vec<usize> = w.iter().map(|x| ((x / sum) * nc as f64).round().max(1.0) as usize).collect();
    let tot: usize = sizes.iter().sum();
    sizes[8] = (sizes[8] as isize + nc as isize - tot as isize) as usize;
    let mut starts = vec![0usize];
    for s in &sizes {
        starts.push(starts.last().unwrap() + s);
    }
    let mut cid = Vec::with_capacity(nc);
    let mut depth = vec![0u8; nc];
    let mut parents: Vec<Vec<u32>> = vec![vec![]; nc];
    for i in 0..nc {
        cid.push(format!("wn:{:08}-n", scr(i as u64 + 1, 48271)));
    }
    for lvl in 1..sizes.len() {
        let (ps, pl) = (starts[lvl - 1], sizes[lvl - 1]);
        for c in starts[lvl]..starts[lvl + 1] {
            depth[c] = lvl as u8;
            let p = ps + rng.skew(pl, 2.5);
            parents[c].push(p as u32);
            if rng.f() < 0.02 {
                let p2 = ps + rng.skew(pl, 2.5);
                if p2 != p {
                    parents[c].push(p2 as u32);
                }
            }
        }
    }
    let mut nchildren = vec![0u32; nc];
    for c in 0..nc {
        for &p in &parents[c] {
            nchildren[p as usize] += 1;
        }
    }
    // ---- claims
    let mut claims: Vec<Claim> = Vec::new();
    let mut add = |claims: &mut Vec<Claim>, s: usize, p: i64, o: Option<usize>, lit: Option<String>, rng: &mut Rng| {
        let id = claims.len() as i64 + 1;
        claims.push(Claim { id, subject: s as u32, pred: p, object: o.map(|x| x as u32), literal: lit, conf: 0.6 + 0.4 * rng.f() });
    };
    for c in 1..nc {
        for &p in &parents[c].clone() {
            add(&mut claims, c, PRED_IS_A, Some(p as usize), None, &mut rng);
        }
    }
    let mut perm: Vec<usize> = (0..nc).collect();
    for i in (1..nc).rev() {
        let j = rng.below(i + 1);
        perm.swap(i, j);
    }
    // part_of: 90k, wholes skewed
    let mut n_part = 0;
    while n_part < 90_000 {
        let s = rng.below(nc);
        let o = perm[rng.skew(nc, 2.0)];
        if s != o {
            add(&mut claims, s, PRED_PART_OF, Some(o), None, &mut rng);
            n_part += 1;
        }
    }
    for _ in 0..8_000 {
        let l = 3 + rng.below(12);
        let a = starts[l] + rng.below(sizes[l]);
        let b = starts[l] + rng.below(sizes[l]);
        if a != b {
            add(&mut claims, a, PRED_ANTONYM, Some(b), None, &mut rng);
        }
    }
    for _ in 0..5_000 {
        let a = starts[6] + rng.below(nc - starts[6]);
        let b = starts[6] + rng.below(nc - starts[6]);
        if a != b {
            add(&mut claims, a, PRED_CAUSES, Some(b), None, &mut rng);
        }
    }
    for _ in 0..30_000 {
        let s = rng.below(nc);
        let lit = format!("prop_{}", rng.below(60));
        add(&mut claims, s, PRED_PROP, None, Some(lit), &mut rng);
    }
    // ---- glosses per concept and language
    let vmax = 30_000f64;
    let mut gloss = |rng: &mut Rng, lang: u8| -> String {
        let n = 5 + rng.below(10);
        let mut s = String::new();
        for k in 0..n {
            if k > 0 {
                s.push(' ');
            }
            let r = vmax.powf(rng.f()) as u64;
            s.push_str(&gloss_word(r, lang));
        }
        s
    };
    let gloss_en: Vec<String> = (0..nc).map(|_| gloss(&mut rng, 0)).collect();
    let gloss_es: Vec<Option<String>> = (0..nc).map(|_| if rng.f() < 0.6 { Some(gloss(&mut rng, 1)) } else { None }).collect();
    // ---- lexemes, senses, forms: 120k en, 80k es
    let mut lexemes = Vec::new();
    let mut senses: Vec<Sense> = Vec::new();
    let mut forms = Vec::new();
    let mut es_cover = perm.clone();
    for i in (1..nc).rev() {
        let j = rng.below(i + 1);
        es_cover.swap(i, j);
    }
    let pos_of = |rng: &mut Rng| -> &'static str {
        let f = rng.f();
        if f < 0.75 { "n" } else if f < 0.87 { "v" } else if f < 0.97 { "a" } else { "r" }
    };
    for (lang, n_lex, mul) in [(0u8, 120_000usize, 48271u64), (1u8, 80_000usize, 69621u64)] {
        for i in 0..n_lex {
            let li = lexemes.len() as u32;
            let first = if lang == 0 { i } else { es_cover[i] };
            let mut lemma = word(scr(i as u64 + 7, mul + lang as u64 * 3));
            if lang == 1 {
                lemma.push_str(["o", "a", "ar"][i % 3]);
            }
            lexemes.push(Lexeme { id: li as i64 + 1, lang, lemma: lemma.clone(), pos: pos_of(&mut rng), first_concept: first as u32 });
            let mut cs = vec![first];
            let (p2, p3) = if lang == 0 { (0.30, 0.08) } else { (0.25, 0.06) };
            if rng.f() < p2 {
                cs.push(rng.below(nc));
                if rng.f() < p3 / p2 {
                    cs.push(rng.below(nc));
                }
            }
            for (r, c) in cs.iter().enumerate() {
                let g = if lang == 0 { Some(gloss_en[*c].clone()) } else { gloss_es[*c].clone() };
                let id = senses.len() as i64 + 1;
                senses.push(Sense { id, lex: li, concept: *c as u32, gloss: g, rank: r as i32 });
            }
            forms.push(Form { lex: li, form: lemma.clone(), features: "lemma" });
            let extras: &[(&str, &str)] = if lang == 0 {
                &[("s", "pl"), ("ed", "past"), ("ing", "prog")]
            } else {
                &[("s", "pl"), ("es", "pl"), ("mente", "adv")]
            };
            let k = 1 + rng.below(3);
            for e in extras.iter().take(k) {
                forms.push(Form { lex: li, form: format!("{}{}", lemma, e.0), features: e.1 });
            }
        }
    }
    Data { cid, depth, parents, nchildren, claims, lexemes, senses, forms }
}

impl Data {
    /// concept -> pack, domain-based (subtrees under depth-2 nodes) with 15% random cross-assigned.
    pub fn assign(&self, n: usize) -> Vec<u8> {
        let nc = self.cid.len();
        if n <= 1 {
            return vec![0; nc];
        }
        let mut dom = vec![0u32; nc];
        for c in 0..nc {
            dom[c] = if self.depth[c] <= 2 { c as u32 } else { dom[self.parents[c][0] as usize] };
        }
        let mut cnt: HashMap<u32, usize> = HashMap::new();
        for c in 0..nc {
            *cnt.entry(dom[c]).or_default() += 1;
        }
        let mut ds: Vec<(u32, usize)> = cnt.into_iter().collect();
        ds.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let mut load = vec![0usize; n];
        let mut bin: HashMap<u32, u8> = HashMap::new();
        for (d, sz) in ds {
            let b = (0..n).min_by_key(|&b| load[b]).unwrap();
            load[b] += sz;
            bin.insert(d, b as u8);
        }
        let mut rng = Rng(99);
        (0..nc)
            .map(|c| if rng.f() < 0.15 { rng.below(n) as u8 } else { bin[&dom[c]] })
            .collect()
    }
    pub fn ancestors(&self, c: usize) -> HashSet<u32> {
        let mut seen = HashSet::new();
        let mut st = vec![c as u32];
        while let Some(x) = st.pop() {
            for &p in &self.parents[x as usize] {
                if seen.insert(p) {
                    st.push(p);
                }
            }
        }
        seen
    }
}

// -------------------------------------------------------------- overlay
pub struct Overlay {
    pub concepts: Vec<String>,
    pub lexemes: Vec<(i64, String)>,           // id, lemma (en)
    pub senses: Vec<(i64, i64, String, String)>, // id, lexeme, concept, gloss
    pub forms: Vec<(i64, String)>,
    pub claims: Vec<(i64, String, i64, Option<String>, Option<String>)>,
    pub tombs: Vec<i64>,
}
pub const USER_BASE: i64 = 1_000_000_000;

pub fn overlay(d: &Data) -> (Overlay, Vec<Vec<String>>) {
    let mut rng = Rng(4242);
    let nc = d.cid.len();
    let mut o = Overlay { concepts: vec![], lexemes: vec![], senses: vec![], forms: vec![], claims: vec![], tombs: vec![] };
    let mut user_samples = vec![]; // x, y (ancestor)
    let mut cl = USER_BASE;
    let mut ncl = || {
        cl += 1;
        cl
    };
    // 400 user concepts with is_a to a pack concept
    for i in 0..400 {
        let u = format!("user:{:06}", i);
        let p = loop {
            let c = rng.below(nc);
            if d.depth[c] >= 4 && d.depth[c] <= 9 {
                break c;
            }
        };
        o.concepts.push(u.clone());
        o.claims.push((ncl(), u.clone(), PRED_IS_A, Some(d.cid[p].clone()), None));
        let lid = USER_BASE + i as i64 + 1;
        let lemma = format!("usr{}", word(i as u64 * 7 + 1));
        o.lexemes.push((lid, lemma.clone()));
        o.forms.push((lid, lemma.clone()));
        o.forms.push((lid, format!("{}s", lemma)));
        o.senses.push((USER_BASE + i as i64 + 1, lid, u.clone(), format!("user taught {}", i)));
        // sample: ancestor k hops up from p, total <= 5 hops
        let k = rng.below(5);
        let mut y = p;
        let mut hops = 0;
        while hops < k && !d.parents[y].is_empty() {
            y = d.parents[y][0] as usize;
            hops += 1;
        }
        user_samples.push(vec![u, d.cid[y].clone()]);
    }
    // 200 part_of
    for i in 0..200 {
        let s = if i % 2 == 0 { o.concepts[rng.below(400)].clone() } else { d.cid[rng.below(nc)].clone() };
        o.claims.push((ncl(), s, PRED_PART_OF, Some(d.cid[rng.below(nc)].clone()), None));
    }
    // 200 overrides: tombstone pack is_a edge, add user is_a with a different parent
    let by_subject: HashMap<u32, i64> = d
        .claims
        .iter()
        .filter(|c| c.pred == PRED_IS_A)
        .map(|c| (c.subject, c.id))
        .collect();
    let mut done = 0;
    while done < 200 {
        let c = rng.below(nc);
        if d.depth[c] < 5 || d.parents[c].len() != 1 {
            continue;
        }
        let np = d.cid[rng.below(nc)].clone();
        o.tombs.push(by_subject[&(c as u32)]);
        o.claims.push((ncl(), d.cid[c].clone(), PRED_IS_A, Some(np), None));
        done += 1;
    }
    // 200 literal claims
    for i in 0..200 {
        o.claims.push((ncl(), d.cid[rng.below(nc)].clone(), PRED_PROP, None, Some(format!("user_prop_{}", i % 20))));
    }
    o.tombs.sort();
    o.tombs.dedup();
    (o, user_samples)
}

// -------------------------------------------------------------- samples
pub type Samples = HashMap<String, Vec<Vec<String>>>;

pub fn make_samples(d: &Data, user_isa: Vec<Vec<String>>) -> Samples {
    let mut rng = Rng(777);
    let nc = d.cid.len();
    let mut s: Samples = HashMap::new();
    let n = 3000;
    // lemma lookups (5% misses)
    let mut v = vec![];
    for _ in 0..n {
        if rng.f() < 0.05 {
            v.push(vec!["en".into(), format!("zz{}", word(rng.below(1_000_000) as u64))]);
        } else {
            let l = &d.lexemes[rng.below(d.lexemes.len())];
            v.push(vec![lang_name(l.lang).into(), l.lemma.clone()]);
        }
    }
    s.insert("lemma".into(), v);
    // forms
    let mut v = vec![];
    for _ in 0..n {
        if rng.f() < 0.05 {
            v.push(vec![format!("zz{}", word(rng.below(1_000_000) as u64))]);
        } else {
            v.push(vec![d.forms[rng.below(d.forms.len())].form.clone()]);
        }
    }
    s.insert("form".into(), v);
    // is_a <= 6 : 50% positive at 1..6 hops, 50% negative
    let walk = |rng: &mut Rng, x: usize, k: usize| -> usize {
        let mut y = x;
        for _ in 0..k {
            let ps = &d.parents[y];
            y = ps[rng.below(ps.len())] as usize;
        }
        y
    };
    let by_depth = |lo: u8, hi: u8| -> Vec<usize> { (0..nc).filter(|&c| d.depth[c] >= lo && d.depth[c] <= hi).collect() };
    let cand = by_depth(6, 15);
    let mut v = vec![];
    for i in 0..n {
        let x = cand[rng.below(cand.len())];
        if i % 2 == 0 {
            let k = 1 + rng.below(6.min(d.depth[x] as usize));
            let y = walk(&mut rng, x, k);
            v.push(vec![d.cid[x].clone(), d.cid[y].clone(), "1".into()]);
        } else {
            let anc = d.ancestors(x);
            let pool = by_depth(2, d.depth[x] - 1);
            let y = loop {
                let y = pool[rng.below(pool.len())];
                if !anc.contains(&(y as u32)) && y != x {
                    break y;
                }
            };
            v.push(vec![d.cid[x].clone(), d.cid[y].clone(), "0".into()]);
        }
    }
    s.insert("isa6".into(), v);
    // deep: depth >= 13
    let cand = by_depth(13, 15);
    let mut v = vec![];
    for i in 0..n {
        let x = cand[rng.below(cand.len())];
        if i % 2 == 0 {
            let y = if rng.f() < 0.5 { 0 } else { walk(&mut rng, x, 10) };
            v.push(vec![d.cid[x].clone(), d.cid[y].clone(), "1".into()]);
        } else {
            let anc = d.ancestors(x);
            let pool = by_depth(4, 12);
            let y = loop {
                let y = pool[rng.below(pool.len())];
                if !anc.contains(&(y as u32)) {
                    break y;
                }
            };
            v.push(vec![d.cid[x].clone(), d.cid[y].clone(), "0".into()]);
        }
    }
    s.insert("isa_deep".into(), v);
    s.insert("isa_user".into(), user_isa);
    // kinds-of: uniform non-leaf; hubs: top-100 fan-out
    let nonleaf: Vec<usize> = (0..nc).filter(|&c| d.nchildren[c] > 0).collect();
    s.insert("kinds".into(), (0..n).map(|_| vec![d.cid[nonleaf[rng.below(nonleaf.len())]].clone()]).collect());
    let mut by_fan: Vec<usize> = (0..nc).collect();
    by_fan.sort_by_key(|&c| std::cmp::Reverse(d.nchildren[c]));
    s.insert("hubs".into(), by_fan.iter().skip(3).take(100).map(|&c| vec![d.cid[c].clone()]).collect());
    // part_of
    let parts: Vec<&Claim> = d.claims.iter().filter(|c| c.pred == PRED_PART_OF).collect();
    s.insert("part_fwd".into(), (0..n).map(|_| vec![d.cid[parts[rng.below(parts.len())].subject as usize].clone()]).collect());
    s.insert("part_rev".into(), (0..n).map(|_| vec![d.cid[parts[rng.below(parts.len())].object.unwrap() as usize].clone()]).collect());
    s.insert("neigh".into(), (0..n).map(|_| vec![d.cid[rng.below(nc)].clone()]).collect());
    s.insert("concepts".into(), (0..n).map(|_| vec![d.cid[rng.below(nc)].clone(), d.cid[rng.below(nc)].clone()]).collect());
    // fts: 1 or 2 terms, log-uniform rank in [40, 30000] (top-40 treated as stopwords)
    let mut v = vec![];
    for i in 0..n {
        let lang = if i % 3 == 2 { 1 } else { 0 };
        let t = |rng: &mut Rng| gloss_word((40f64 * (30_000f64 / 40f64).powf(rng.f())) as u64, lang);
        let q = if rng.f() < 0.5 { t(&mut rng) } else { format!("{} {}", t(&mut rng), t(&mut rng)) };
        v.push(vec![q]);
    }
    s.insert("fts".into(), v);
    s
}

pub fn write_samples(path: &std::path::Path, s: &Samples) {
    let mut keys: Vec<_> = s.keys().collect();
    keys.sort();
    let mut out = String::new();
    for k in keys {
        for row in &s[k] {
            out.push_str(k);
            for f in row {
                out.push('\t');
                out.push_str(f);
            }
            out.push('\n');
        }
    }
    std::fs::write(path, out).unwrap();
}
pub fn read_samples(path: &std::path::Path) -> Samples {
    let mut s: Samples = HashMap::new();
    for line in std::fs::read_to_string(path).unwrap().lines() {
        let mut it = line.split('\t').map(|x| x.to_string());
        let k = it.next().unwrap();
        s.entry(k).or_default().push(it.collect());
    }
    s
}
