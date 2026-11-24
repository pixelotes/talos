mod bench;
mod data;
mod mk;

use bench::Cfg;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn gen() {
    let root = root();
    let data = root.join("data");
    let t = std::time::Instant::now();
    let d = data::generate(20241124, 120_000);
    println!("generated in {:?}", t.elapsed());
    let (ov, user_isa) = data::overlay(&d);
    let samples = data::make_samples(&d, user_isa);
    std::fs::create_dir_all(&data).unwrap();
    data::write_samples(&data.join("samples.tsv"), &samples);

    // statistics
    let mut hist = vec![0usize; 16];
    for &x in &d.depth {
        hist[x as usize] += 1;
    }
    let isa = d.claims.iter().filter(|c| c.pred == 1).count();
    let part = d.claims.iter().filter(|c| c.pred == 2).count();
    let maxfan = d.nchildren.iter().max().unwrap();
    let nonleaf = d.nchildren.iter().filter(|&&x| x > 0).count();
    println!("concepts={} claims={} is_a={} part_of={} lexemes={} senses={} forms={}", d.cid.len(), d.claims.len(), isa, part, d.lexemes.len(), d.senses.len(), d.forms.len());
    println!("depth histogram {:?}", hist);
    println!("max fan-out={} non-leaf={} multi-parent={}", maxfan, nonleaf, d.parents.iter().filter(|p| p.len() > 1).count());
    println!("overlay: concepts={} claims={} tombstones={}", ov.concepts.len(), ov.claims.len(), ov.tombs.len());
    for n in [3usize, 8] {
        let a = d.assign(n);
        let cross = |pred: i64| {
            let tot = d.claims.iter().filter(|c| c.pred == pred && c.object.is_some()).count();
            let x = d.claims.iter().filter(|c| c.pred == pred && c.object.map_or(false, |o| a[o as usize] != a[c.subject as usize])).count();
            (x, tot)
        };
        let mut per = vec![0usize; n];
        for &p in &a {
            per[p as usize] += 1;
        }
        println!("N={} concepts/pack={:?} cross-pack is_a={:?} part_of={:?}", n, per, cross(1), cross(2));
    }

    // A: merged
    let all = d.assign(1);
    let t = std::time::Instant::now();
    std::fs::create_dir_all(data.join("A")).unwrap();
    mk::write_pack(&data.join("A/merged.db"), &d, &all, 0, 100, "merged", true);
    println!("A built {:?}", t.elapsed());
    for (layout, n) in [('B', 1usize), ('C', 3), ('D', 8)] {
        let dir = data.join(layout.to_string());
        std::fs::create_dir_all(&dir).unwrap();
        let a = d.assign(n);
        for p in 0..n {
            mk::write_pack(&dir.join(format!("pack{}.talospack", p + 1)), &d, &a, p as u8, 101 + p as i64, &format!("pack{}", p + 1), false);
        }
        mk::write_user(&dir.join("user.db"), &ov);
        println!("{} built", layout);
    }
    println!("--- sizes");
    let _ = std::process::Command::new("sh").arg("-c").arg(format!("cd {} && ls -l */* | awk '{{print $5, $9}}'", data.display())).status();
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("");
    match cmd {
        "gen" => gen(),
        "info" => {
            let c = rusqlite::Connection::open_in_memory().unwrap();
            let v: String = c.query_row("select sqlite_version()", [], |r| r.get(0)).unwrap();
            println!("sqlite {}", v);
            let mut st = c.prepare("PRAGMA compile_options").unwrap();
            let opts: Vec<String> = st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect();
            println!("{}", opts.join(" "));
            let mut ok = 0;
            for i in 1..=200 {
                if c.execute_batch(&format!("ATTACH ':memory:' AS x{}", i)).is_err() {
                    break;
                }
                ok += 1;
            }
            println!("max attached databases accepted: {}", ok);
        }
        "bench" => {
            let mut cfg = Cfg { layout: args[2].chars().next().unwrap(), cache_kib: None, mmap_mb: 0, wal: true, sync: "NORMAL".into(), immutable: false, runs: 3000, edge: false, root: root() };
            let mut suite = "all".to_string();
            let mut i = 3;
            while i < args.len() {
                let v = args.get(i + 1).cloned().unwrap_or_default();
                match args[i].as_str() {
                    "--cache" => cfg.cache_kib = Some(v.parse().unwrap()),
                    "--mmap" => cfg.mmap_mb = v.parse().unwrap(),
                    "--wal" => cfg.wal = v == "1",
                    "--sync" => cfg.sync = v,
                    "--immutable" => {
                        cfg.immutable = true;
                        i -= 1;
                    }
                    "--runs" => cfg.runs = v.parse().unwrap(),
                    "--edge" => {
                        cfg.edge = true;
                        i -= 1;
                    }
                    "--suite" => suite = v,
                    _ => panic!("bad flag {}", args[i]),
                }
                i += 2;
            }
            bench::suite(&cfg, &root(), &suite);
        }
        _ => eprintln!("usage: pack-attach gen | bench <A|B|C|D> [--suite all|edge|quick|write|cold|open|prepare|plans|baseline] [--cache KiB] [--mmap MiB] [--wal 0|1] [--sync NORMAL|FULL] [--immutable] [--edge] [--runs N]"),
    }
}
