# Sources for the core vocabulary pack: English and Spanish wordnets, morphology, glosses

Survey date: 2026-11-24. Factual survey, not legal advice.

Needs (from `docs/KNOWLEDGE.md` §1, §7 and ADR 0005/0006): a synset-like language-neutral concept; es and en lexemes per concept; hypernym / meronym / antonym relations; a gloss per language; inflected forms ("guitarras" -> "guitarra"); a licence that allows redistributing a derived SQLite database as a separate `*.talospack` next to an MIT binary.

Evidence labels used throughout: **[P]** read on a primary source (project page, licence file, README, paper abstract); **[S]** reported second-hand (search-engine summary, third-party page, mirror); **[U]** unverified.

## 1. Comparison table

| Source | Licence | Size (as published) | Formats | Alignment key | Maintained? | Verdict |
|---|---|---|---|---|---|---|
| Princeton WordNet 3.0 | WordNet License (permissive, notice-retaining) | 117,659 synsets, 155,287 strings, 206,941 word-sense pairs [P, via man-page mirror] | WNDB, Prolog | its own offsets (the de-facto ILI anchor) | No new releases evident [U] | Keep as join key only |
| Princeton WordNet 3.1 | WordNet License (same page as 3.0, per OMW index) | 117,791 synsets, 159,015 words, 378,203 relations [P, OEWN README] | WNDB | own offsets; ILI map exists (`ili-map-pwn31`) | as above [U] | Not needed |
| Open English WordNet 2025 / 2025+ | CC BY 4.0 (plus the WordNet notice it inherits) | 2025+: 120,564 synsets, 161,875 words, 419,226 relations; 2025 core: 107,519 synsets, 135,969 words, 355,064 relations [P] | WN-LMF XML, JSON (2025 only), RDF/Turtle, WNDB | CILI ids on synsets [P, wn docs for 2024]; CILI maps to PWN 3.0/3.1 [P] | Yes: yearly editions 2019-2025, PR-based [P] | Recommended English backbone |
| MCR 3.0 Spanish wordnet | CC BY 3.0 (English WordNet part of the package under the WordNet licence) [P] | 38,512 synsets, 36,681 words (OMW 1 table) [P]; 37,876 lexical entries in a derived LMF [S] | MCR tab/SQL; OMW tab; WN-LMF via `wn` as `omw-es:1.4` [P] | WordNet 3.0 as Inter-Lingual Index (offsets) [P] | Last release Nov 2016; OMW copy dates to 2012 edition [S] | Recommended Spanish backbone, with caveats |
| OMW 1.x / 2 / Cygnet (`omw-es`) | Project hosting is open; Spanish data licence is MCR's, CC BY 3.0 [P] | same as MCR | OMW tab; GWA LMF | PWN 3.0 via `omw-en:1.4`; CILI | Mostly a redistribution channel | Convenient packaging of MCR, not a new source |
| FreeLing Spanish sense data | CC BY (MCR-derived) [P]; morphological dictionary licence not stated on the page [P] | n/a | FreeLing dictionaries | MCR | Yes | Not needed |
| Wiktionary (en/es) | CC BY-SA 4.0 and GFDL [P] | n/a | dumps, Wiktextract JSONL | none (word-centric) | Yes | Supplement only, in its own pack |
| Wikidata | CC0 for data and lexemes [P] | Spanish lexeme count not found [U] | dumps, SPARQL | Q/L ids | Yes | Descriptions possible for entities, poor fit for common words |
| BabelNet | "BabelNet Non-Commercial licence"; sharing only with research institutions; derivatives must carry the BabelNet licence [P] | n/a | API/dumps | BabelNet synsets | Yes | Exclude |

## 2. Per-source notes

### 2.1 English

**Princeton WordNet 3.0 / 3.1.**
- Licence: the WordNet License. Permission to use, copy, modify and distribute the software and database for any purpose without fee, provided the copyright notice and the disclaimer appear on all copies; Princeton's name may not be used in advertising or publicity about distribution; title to the copyright stays with Princeton. Read at https://spdx.org/licenses/WordNet.html [P] and, for the same grant, in the Debian/Ubuntu man page https://manpages.ubuntu.com/manpages/focal/man7/wnlicens.7WN.html [S]. The Princeton page itself (https://wordnet.princeton.edu/license-and-commercial-use) returned HTTP 403 to the fetcher, so the Princeton-hosted text was not read [U]. A search result states OSI lists a "WordNet version 3.0 license" approved July 2025 (via https://opensource.org/?p=140960) [S].
- WordNet 3.0 counts, from the `wnstats(7)` man page, https://manpages.ubuntu.com/manpages/focal/man7/wnstats.7WN.html [S, mirror of the primary doc]: 82,115 noun, 13,767 verb, 18,156 adjective, 3,621 adverb synsets = 117,659 total (my sum; the page's own totals row for strings, 155,287, and word-sense pairs, 206,941, matched my sums). 147,278 distinct strings across categories.
- WordNet 3.1 counts (117,791 synsets, 159,015 words, 378,203 relations) are quoted in the Open English WordNet README: https://github.com/globalwordnet/english-wordnet [P].
- OMW packages both 3.0 (`omw-en`) and 3.1 (`omw-en31`) under that same licence URL: https://raw.githubusercontent.com/bond-lab/omw-data/main/index.toml [P].
- Whether Princeton has issued anything after 3.1: not established (the download page returned 403) [U].

**Open English WordNet (OEWN).**
- Site: https://en-word.net/ ; the front page states 120,068 synsets, 153,261 entries, CC-BY 4.0, derived from Princeton WordNet [P] (this is the older "2025" snapshot figure; see below for the edition numbers).
- Latest edition: 2025 and 2025+, both dated 2025-12-31, per https://en-word.net/downloads [P] and https://github.com/globalwordnet/english-wordnet [P]. Earlier editions 2019 to 2024 (2024-11-01).
- Counts (README): 2025+ = 161,875 words, 120,564 synsets, 419,226 relations; 2025 core = 135,969 words, 107,519 synsets, 355,064 relations [P]. "2025" has common nouns, verbs, adjectives and adverbs only; "2025+" adds proper nouns from Open English Namenet [P, downloads page].
- Formats: WN-LMF XML (10.8 MB gz for 2025), RDF/Turtle, WNDB, and JSON for 2025 only [P, downloads page].
- Licence: CC BY 4.0, on top of the Princeton WordNet licence. https://raw.githubusercontent.com/globalwordnet/english-wordnet/main/LICENSE.md says one may share and adapt "providing attribution is given to both" [P]. CC BY 4.0 §3(a)(1) duties: keep creator identification, copyright notice, licence reference, disclaimer notice, a link to the material where practical, indicate changes, state the licence [P, summary of the text fetched].
- Maintenance: yearly editions, pull-request workflow, 13 contributors listed, "we cannot accept any automatically generated results that have not been manually validated" [P]. This is clearly the better maintained of the two English options.
- Alignment: the README says correspondence to previous versions and other-language wordnets is provided through CILI [P]. The OEWN 2024 synsets carry ILI ids (example `apricot.ili` = `i77784`) in the `wn` docs, https://wn.readthedocs.io/en/latest/guides/interlingual.html [P]. CILI, https://github.com/globalwordnet/cili , ships `ili-map-pwn30.tab` (PWN 3.0 -> ILI), `ili-map-pwn31.tab`, and `ili.ttl` [P].
- OEWN is a fork that has diverged from Princeton ("quality may differ from the Princeton Wordnet") [P]. Because of merges, splits and deletions since 3.0/3.1, not every MCR synset (keyed to 3.0 offsets) can be expected to land on an OEWN 2025 synset. How many do not: [U]; requires running the join.

### 2.2 Spanish

**MCR 3.0 Spanish wordnet (the only open Spanish wordnet with real coverage).**
- Project/pages: https://adimen.ehu.eus/web/MCR (server unreachable from the fetcher on the survey day; ECONNREFUSED, also for the README files) [U for anything read only there]. The release announcement, "MCR 3.0 (Release 2016)", Nov 22 2016, https://groups.google.com/g/mcr-users/c/b315iLS581U , says English, Spanish, Catalan, Basque, Galician and Portuguese wordnets are linked through an ILI based on WordNet 3.0, with semantically tagged glosses, WordNet Domains, Top Ontology, AdimenSUMO and Basic Level Concepts [P].
- Licence: the package LICENSE, as copied in the OMW repo (https://github.com/bond-lab/omw-data/blob/main/wns/mcr/LICENSE): English WordNet synset and relation data in `engWN/` are under the original WordNet licence; "all other data in this package" is CC BY 3.0 [P]. OMW's `index.toml` lists `omw-es`, "Multilingual Central Repository (Spanish)", licence https://creativecommons.org/licenses/by/3.0/ , citation Gonzalez-Agirre, Laparra, Rigau, GWC 2012 [P]. The MCR list thread says the current Spanish wordnet was rebuilt from scratch, is "free for both commercial and research purposes", and that the older WN 1.6-linked Spanish wordnet is no longer distributed: https://groups.google.com/g/mcr-users/c/Ih_yoap7HEc [P].
- Size: 38,512 synsets and 36,681 words in OMW 1's table, https://omwn.org/omw1.html [P]. The `wn` package README lists `omw-es:1.4` with 38,512 synsets [S]. The GWC 2012 paper (https://aclanthology.org/L12-1128/) gives no counts in its abstract [P]. The Spanish side is therefore about a third of WordNet's 117,659 synsets. Lemma (variant) count: not verified; 36,681 is distinct words, not sense pairs [U for the pair count].
- Alignment: WordNet 3.0 as ILI, expressed as 3.0 offsets (`spa-30-<offset>-<pos>` ids) [P: Herrera et al. 2016 quotes ids like "spa-30-03623556-n", https://aclanthology.org/2016.gwc-1.18.pdf, text extracted locally]. In `wn`, `omw-es:1.4` requires `omw-en:1.4`, "OMW English Wordnet based on WordNet 3.0" [P], https://wn.readthedocs.io/en/latest/guides/lexicons.html . OMW 1/2 repackaging of MCR dates to the 2012 edition, not the 2016 release [S, https://arxiv.org/pdf/2303.01847 summary].
- Quality/coverage notes: Herrera et al. (GWC 2016) found missing very common words (adverbs "recién", "rápidamente"), and "no Spanish gloss was found for many of the words searched" [P, text extracted from the PDF; abstract at https://aclanthology.org/2016.gwc-1.18/ ]. See §2.3 for gloss numbers.

**Open Multilingual Wordnet (OMW 1.x, OMW 2, Cygnet).**
- Current home https://omwn.org/ ; calls OMW 1 and 2 "older versions", current browser is Cygnet (https://cygnet.maudslay.eu/); describes the data as free to use, modify and share "for any purpose" without naming a licence [P].
- OMW 1's table has one Spanish row (MCR) with CC BY 3.0 [P]; the page notes OMW 1 lacks "definitions and examples from wordnets such as Spanish" [P]. OMW 1 additionally ships automatically extracted Wiktionary and CLDR data for 150+ languages (`wns/wikt`, `wns/cldr`); the page showed no Spanish row for those and gave no per-language counts or licences [P for absence on that page, U for whether a Spanish Wiktionary-derived wordnet exists in the repo].
- The repo https://github.com/bond-lab/omw-data has `wns/mcr/` with `wn-data-spa.tab`, `spa-changes.tab`, `LICENSE` [P]. Whether `wn-data-spa.tab` includes definition lines: [U], answerable only by opening the file (not downloaded here).
- OMW 2 aims at a Collaborative Interlingual Index, https://omwn.org/omw2.html [P]; no Spanish entries are described there. Cygnet's per-wordnet table (synsets, lemmas, definitions, licence, version columns) is a JavaScript app and did not render usable numbers [U].
- Conclusion: for Spanish there is one real open source, MCR, whichever way it is packaged. OMW/`wn` (`omw-es:1.4`) is useful as a ready-made WN-LMF form of MCR, but is the 2012-era data.

**FreeLing.** Code under AGPL (v4.2 manual) [S]; dictionaries have "different copyright holders and different licenses", see file COPYING; Spanish sense data extracted from MCR 3.0 under CC BY: https://nlp.lsi.upc.edu/freeling/node/12 [P]. The Spanish morphological dictionary licence is not stated there [U]. Older repository records list GPL v3 for a "Spanish LMF Freeling Lexicon" [S]. Treat as unusable for a pack until COPYING is read.

**BabelNet.** https://babelnet.org/license : a "BabelNet Non-Commercial license"; sharing limited to research institutions; derivatives must say they are a processed version of BabelNet and be made available under the BabelNet licence [P, summary page; the full terms at babelnet.org/full-license were not read]. Incompatible with an open pack. Exclude.

**Wikidata lexemes.** CC0 for the main, Property, Lexeme and EntitySchema namespaces; "All data in Wikidata has a CC0 license": https://www.wikidata.org/wiki/Wikidata:Licensing [P]. Lexemes are word-centric with sparse sense-to-concept links; a Spanish lexeme count was not found (2020 papers give ~250k lexemes over all languages, Spanish not among the top five) [S]. Good for entities later (ADR 0005), weak for a common-word backbone.

**Wiktionary-derived wordnets.** None found for Spanish in OMW (see above). Wiktionary senses are not aligned to synsets (KNOWLEDGE.md §7 says the same); no further candidate found [U].

### 2.3 How many Spanish synsets have a Spanish gloss

**No figure could be verified from a primary source.** The primary MCR documents that would hold it were unreachable (adimen.ehu.eus refused connections: README-2016.txt, the LREC'12 and GWC'12 PDFs) or the numbers were not in the abstract. Best available, all second-hand or older:

- "Only around 15% of the glosses in the Spanish version have been translated", README of a University of the Republic (Uruguay) MCR transform tool: https://github.com/pln-fing-udelar/wn-mcr-transform [P as to what the page says; the claim itself is [S] and its version scope is unstated, though that page is the only direct quote found].
- "About 10% of the 68,000 Spanish synsets contain a definition", reported by a search summary of a study on enriching Spanish WordNet with machine-translated glosses (Gimenez and Marquez, "Low-Cost Enrichment of Spanish WordNet with Automatically Translated Glosses", via https://www.researchgate.net/publication/220873064) [S]. The 68,000 figure belongs to an older Spanish wordnet (not the 38,512-synset MCR 3.0), so it does not carry over [S].
- A search summary credited a 2024 paper (Julián-Iranzo, Rigau, Sáenz-Pérez, Velasco-Crespo, "Conversion of the Spanish WordNet databases into a Prolog-readable format", Language Resources and Evaluation, DOI 10.1007/s10579-024-09752-w) with "119,074 synsets, of which 20,960 had Spanish glosses (17.6%)". I could not read the paper (PDF unreadable to the fetcher, publisher page redirected to login) and a follow-up search found no source for 20,960 [U]. The synset total in that summary also does not match MCR 3.0's 38,512, so it likely describes a different table (possibly the whole MCR/ILI). Do not quote it.
- Herrera et al. 2016: "no Spanish gloss was found for many of the words searched" [P, qualitative].

Working estimate for planning: **roughly 10-18% of Spanish synsets have a Spanish gloss, i.e. on the order of 4,000-7,000 of 38,512; verification level: low (second-hand, version unclear).** The MCR schema stores a per-synset gloss column in `wei_spa-30_synset` "if any", and the MCR list says glosses were enriched with machine translation (MyMemory) [S]; the licence of those machine-translated strings is not documented [U].

Concretely this means the plan in KNOWLEDGE.md §7 is right: most Spanish concepts will have no Spanish gloss, and Talos must say so.

### 2.4 Morphology

| Source | Language | Licence | Size | Notes |
|---|---|---|---|---|
| WordNet `morphy` rules + `*.exc` exception lists | en | WordNet License (same as database) [S, man page] | exceptions are small lists | Rules: noun -s, -ses, -xes, -zes, -ches, -shes, -men, -ies; verb -s, -ies, -es, -ed, -ing; adjective -er, -est; no adverb rules; exceptions checked first: https://manpages.ubuntu.com/manpages/focal/man7/morphy.7WN.html [S]. Cheap, covers the lemmas WordNet knows. In OEWN, WNDB output includes `.exc` files [U, not checked] |
| UniMorph `spa` | es | CC BY-SA 3.0 [P, https://github.com/unimorph/spa] | 51,395 noun, 12,700 adjective, 6,812 verb lemmas (70,907) and 1,196,245 inflected forms [P]; source English Wiktionary [P] | Share-alike. Verbs are few (6,812), so verb coverage is thin |
| UniMorph `eng` | en | CC BY-SA 3.0 [P, https://github.com/unimorph/eng]; the page names Wikipedia as source, which looks odd and should be checked [U] | not stated | Not needed given morphy |
| Wiktextract / kaikki.org | es, en | Code MIT [P, https://github.com/tatuylonen/wiktextract]. Data: kaikki.org states no licence, asks for citation of Ylonen (LREC 2022) [P, https://kaikki.org/dictionary/rawdata.html]; the underlying text is CC BY-SA 4.0/GFDL [P, https://en.wiktionary.org/wiki/Wiktionary:Copyrights] | Spanish edition (es.wiktionary extract) 1.1 GB JSONL, "work in progress" [P]; English-edition Spanish entries also exist | Treat extracted data as CC BY-SA. Largest coverage, heaviest to process |
| FreeLing es dictionary | es | Not stated on the data page; see COPYING; code AGPL [P/S above] | n/a | Unclear; avoid until COPYING is read |
| Rule-based generator in Talos (plural -s/-es, accents, verb conjugation) | es, en | MIT (own code) | tiny | No data licence problem; recall depends on rules; can be validated against UniMorph in tests (not shipped) |

Recommendation for morphology: English from morphy-style rules plus the WordNet exception lists (which ship under the WordNet licence already attached to the backbone). Spanish from a small rule-based generator for nouns, adjectives, regular verbs, and a share-alike pack of UniMorph forms (or Wiktionary extracts) as an optional second pack, because those are CC BY-SA and would force share-alike onto whatever they are merged into (see §3). The core pack should not contain them.

### 2.5 Options for Spanish glosses

| Option | Licence of the result | Coverage | Fit |
|---|---|---|---|
| Leave empty; say "no definition in Spanish" (current design, KNOWLEDGE.md §7) | n/a | honest | works today |
| Hand-write for a core subset | Talos' own (MIT/CC0 as chosen) | hundreds to a few thousand | highest quality; costs authoring time; best for the "everyday concepts" seed |
| Wikidata descriptions (es) | CC0 [P] | only for concepts that map to a Wikidata item; short descriptions, not dictionary glosses; the WordNet-to-Wikidata mapping property was not verified [U] | good for entities later, weak for common-word synsets |
| Wiktionary (es) definitions | CC BY-SA 4.0 + GFDL [P] | broad, but word-centric: the sense -> synset alignment has to be guessed | possible as a separate optional share-alike pack; contaminates any pack it is merged into |
| Machine translation of the OEWN gloss | derivative of CC BY 4.0 text: attribution to OEWN/Princeton is enough; MT engine terms may add constraints [U] | full | cheap, but quality unvetted; must be stored as machine-generated, lower confidence, never shown as the "official" gloss without a mark |
| MCR's own machine-translated glosses | CC BY 3.0 for non-English data per the package LICENSE [P], but whether MT output from an external API (MyMemory) is covered by that statement was not checked [U] | partial (see §2.3) | include with provenance flag |

## 3. Licence compatibility with a pack distributed alongside (not inside) an MIT binary

The pack is a separate file; the MIT licence of the code does not extend to it (ADR 0006). The question is only what each source requires of the pack itself.

- **WordNet License** (Princeton): redistribution of modified databases is allowed; the copyright notice and disclaimer must be on all copies; no use of Princeton's name in advertising or publicity. Compatible. No share-alike.
- **CC BY 4.0 (OEWN)**: attribution, licence reference, indication of changes, disclaimer; no share-alike; adaptation allowed. Compatible. A SQLite database built from it is an adaptation; say so in `meta`.
- **CC BY 3.0 (MCR Spanish)**: same family: attribution, licence link, indicate changes; no share-alike. Compatible with the above. Because the MCR package keeps the English synset and relation data under the WordNet licence, use the Spanish data only, taking English from OEWN (also avoids carrying a second copy).
- **Combined pack**: all three are attribution-only, so the pack can be offered under "CC BY 4.0 for the OEWN-derived parts and CC BY 3.0 for the MCR-derived parts, with the WordNet notice retained", without choosing a single outbound licence. A single outbound CC BY 4.0 label on the whole file would arguably over-claim for the MCR rows; the safer wording is to carry each source's notice and say the pack as a whole is distributed under the most restrictive of these (attribution to all). This is a reading, not a ruling [U].
- **CC BY-SA 4.0/3.0 (Wiktionary, UniMorph, kaikki data)**: share-alike applies to adaptations. A pack that merges that data would itself have to be CC BY-SA. Compatible with distributing next to an MIT binary (a separate file) but they should live in their own optional pack so the core pack stays attribution-only.
- **CC0 (Wikidata)**: no obligations; fine anywhere.
- **BabelNet**: not compatible with open redistribution of a derived database.
- **FreeLing data / dictionaries**: unknown per file; AGPL code is irrelevant to a data pack but the dictionaries' own licence is [U].

## 4. Recommendation

**Backbone.** Open English WordNet 2025 (core, or 2025+ if proper nouns are wanted) for English concepts, English lemmas, glosses and relations (hypernym, meronym, antonym), plus the MCR 3.0 Spanish wordnet (CC BY 3.0) for Spanish lexemes, joined to OEWN through the WordNet 3.0 offsets -> CILI (`ili-map-pwn30.tab`) -> OEWN ILI link. Concept ids: use the CILI id (`i…`) as the stable global key where present, falling back to the OEWN synset id. Rationale: OEWN is the maintained, openly licensed English side; MCR is the only substantial open Spanish side; both are reachable through the ILI. If the join loses too many Spanish synsets, the fallback is Princeton WordNet 3.0 (permissive WordNet licence) as the English side: the alignment is then exact, at the price of an unmaintained English wordnet with fewer synsets (117,659 vs 120,564) and fewer relations (the 3.1 figure of 378,203 vs 419,226 for OEWN).

**Glosses.** English gloss from OEWN. Spanish gloss only where MCR has one (marked with its source); otherwise empty, and Talos says so. Hand-write Spanish glosses for the seed/core concepts the eval battery needs (own licence). Optionally an add-on pack of Wiktionary glosses (CC BY-SA) and one of machine-translated glosses (marked as such). Do not merge share-alike material into the core pack.

**Notices the core pack should carry (in `meta` and in a `NOTICE`/README beside the file):**

```
This knowledge pack contains data derived from:

- Open English WordNet (2025 edition), Copyright 2025 the Open English WordNet
  contributors, licensed under CC BY 4.0 (https://creativecommons.org/licenses/by/4.0/).
  Open English WordNet is derived from Princeton WordNet.
  https://en-word.net/
- Princeton WordNet 3.0, Copyright 2006 by Princeton University. All rights reserved.
  WordNet is provided "AS IS" with no warranties; Princeton University's name may not
  be used in advertising or publicity pertaining to distribution of this data.
  (WordNet License, https://wordnet.princeton.edu/license-and-commercial-use)
- Multilingual Central Repository 3.0 (Spanish wordnet), Gonzalez-Agirre, Laparra and
  Rigau, GWC 2012, licensed under CC BY 3.0 (https://creativecommons.org/licenses/by/3.0/).
  https://adimen.ehu.eus/web/MCR

The data was converted to a SQLite schema, merged by concept, and reduced to the
fields Talos uses; no endorsement by the original authors is implied.
```

(The Open English WordNet copyright line, the exact Princeton notice wording and the MCR attribution wording must be copied from each project's own LICENSE file when the pack is built; I could read the grant text only through SPDX and mirrors, and the EHU server was not reachable.)

## 5. Verification status

**Verified from a primary source [P]**
- OEWN: licence (CC BY 4.0), 2025/2025+ editions dated 2025-12-31, counts (README), formats and file sizes (downloads page), fork status, PR-based maintenance, CILI mapping statement, 13 contributors.
- CILI repository contents: mapping files for PWN 3.0 and 3.1.
- MCR 3.0 package LICENSE (via OMW copy): CC BY 3.0 for non-English data, WordNet licence for English data; OMW `index.toml` entries for `omw-es`, `omw-en`, `omw-en31`; OMW 1 table: Spanish 38,512 synsets, 36,681 words, CC BY 3.0; OMW 1 lacks Spanish definitions; MCR release announcement (Nov 2016, ILI on WordNet 3.0); MCR list statement on the CC BY 3.0 licence and the dropped WN 1.6 version.
- `wn` docs: `omw-es:1.4` needs `omw-en:1.4` (WordNet 3.0); OEWN synsets carry ILI ids; relations across lexicons found through ILI.
- SPDX WordNet licence summary; Wikidata CC0; Wiktionary CC BY-SA 4.0 + GFDL; BabelNet "Non-Commercial" licence summary; UniMorph `spa` licence and size; wiktextract code MIT; kaikki.org gives no data licence statement; FreeLing data page statements.

**Second-hand [S]**
- WordNet 3.0 counts (man page mirror) and morphy rules; WordNet licence text via Debian/Ubuntu man pages; OSI approval of the WordNet licence; `omw-es:1.4` = 38,512 synsets (third-party `wn` README); OMW Spanish data being the 2012 edition (arXiv summary); 37,876 lexical entries in a UPF LMF; Spanish gloss figures of ~15% and ~10% (see §2.3); FreeLing code AGPL; MCR machine-translated glosses via MyMemory; Wikidata lexeme totals.

**Unverified [U]**
- Any exact count of Spanish synsets with a Spanish gloss in MCR 3.0 or in the OMW packaging; the claim "119,074 / 20,960 / 17.6%".
- Spanish lemma/sense-pair counts for MCR 3.0.
- The Princeton-hosted WordNet licence text and any WordNet release after 3.1 (site returned 403).
- How many MCR synsets map onto OEWN 2025 synsets via CILI (merges/splits since 3.0).
- Whether `wn-data-spa.tab` carries definition lines, and what share are original vs machine-translated.
- Licence of FreeLing's Spanish morphological dictionary; UniMorph `eng` source statement; Spanish Wiktionary-derived wordnet in OMW's extended set; Wikidata property for WordNet synsets; Wikidata Spanish lexeme count.
- MCR relations: whether the Spanish synsets inherit all PWN 3.0 relations (hypernym, meronym, antonym) through the ILI, or have their own; MCR antonyms are lexical (sense-level) in WordNet, so the importer must carry sense-level antonym links.
- The right outbound licence wording for a pack mixing CC BY 4.0, CC BY 3.0 and the WordNet licence (a reading, not a ruling).

## 6. Remaining work to confirm the unverified items

1. When adimen.ehu.eus is reachable, read README-2016.txt and the LICENSE there; download the MCR 3.0 package once at build time and count rows in `wei_spa-30_synset` with a non-empty gloss (and `wei_spa-30_variant` for lemma/sense pairs). Same count on `wn-data-spa.tab` from OMW to compare 2012 vs 2016 data.
2. Build the join: PWN 3.0 offset -> CILI id (`ili-map-pwn30.tab`) -> OEWN 2025 synset. Report the share of MCR Spanish synsets that find an OEWN synset, and whether OEWN 2025 uses the same ILI ids as in 2024.
3. Count relations by type (hypernym, meronym, antonym) in the joined result; check antonym handling at sense level.
4. Read the 2024 LRE paper (Springer) for the Spanish gloss table, and the GWC'12 paper's "number of glosses of each WN" table.
5. Read Princeton's own licence page from a normal browser and copy the exact notice; read OEWN's `LICENSE.md` full text and copyright line; check for a WordNet release after 3.1.
6. Read FreeLing's `COPYING` for the Spanish dictionary; read the UniMorph and kaikki.org licence statements for the exact data release used; decide whether to ship Spanish forms at all in the first pack.
7. If a Wikidata-based gloss fill is wanted, check whether a property linking items to WordNet synsets exists, and the coverage of Spanish descriptions for those items.
8. Have someone qualified review the combined NOTICE text before the pack is published.
