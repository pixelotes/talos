# Source data committed to the repository

## `wikidata-p8814-es.tsv`

The Spanish description of every Wikidata item that names a WordNet 3.1
synset (property P8814, "WordNet 3.1 Synset ID"). `talos-pack wordnet` reads
it to give a Spanish gloss to the Spanish senses the MCR wordnet has none for
(source `wikidata` in the vocabulary pack). Not a pack and not built by
`talos-pack build`.

- **Taken:** 2026-11-26, from https://query.wikidata.org/sparql.
- **Query:**
  `SELECT ?i ?s ?d WHERE { ?i wdt:P8814 ?s . ?i schema:description ?d FILTER(lang(?d)="es") }`
- **Format:** a header line `?i	?s	?d`, then one row per line, three
  tab-separated columns: the item (`<http://www.wikidata.org/entity/Q1013>`),
  the synset id (`"08978603-n"`), the description (`"país de África del Sur"@es`,
  with `\"` and the other backslash escapes of an RDF literal). 28,703 rows,
  sorted bytewise.
- **Licence:** CC0 1.0. Wikidata's structured data is dedicated to the public
  domain (https://www.wikidata.org/wiki/Wikidata:Licensing). The descriptions
  are short labels written by Wikidata contributors, not dictionary
  definitions. No attribution is required; the pack's notice names the source
  anyway.
- **Taken in slices.** The whole query times out and the endpoint then ends
  the response mid-stream, after rows that look complete: a first export
  stopped at 17,657 rows that way. This one ran the query once per two-digit
  prefix of the synset id, adding `FILTER(STRSTARTS(?s, "00"))` and so on up
  to `"15"`, checked that every line of every slice is a row, and merged the
  slices. The builder refuses a file with any line that is not a row, so a
  damaged export is never read as data.

### Refreshing it

Wikidata changes every day, so the pack is built from this snapshot, not from
the live service; the same snapshot and sources give the same pack bytes. To
refresh, run the query in slices as described above (or take the rows from a
Wikidata dump), save the result as
tab-separated values with the same header and columns, replace the file, and
update the date above. Check that the export has no line the builder refuses
(`cargo run --release -p talos-pack -- wordnet DIR` stops at the first one), then
build the vocabulary pack and run
`cargo test -p talos-pack --test vocabulary`; its ranges may need to move with
the new counts. `--wikidata FILE` builds from another snapshot without
replacing this one; `--no-wikidata` builds the pack without Wikidata.

The WordNet 3.1 map that joins the synset ids to concept ids
(`ili-map-pwn31.tab`) is not kept here: it is downloaded with the other
sources and checked by its SHA-256 (docs/DEVELOPMENT.md, "The vocabulary pack").
