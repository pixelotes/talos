//! Person reflection: the user's first person becomes the second, so an
//! echo reads as said *to* the user (docs/spec/reflector.md section 3).
//!
//! Reflection is one-directional and conservative. Every word of a
//! fragment is either mapped by a table, copied as written, or **blocks**
//! the echo; a blocked echo means the shape's neutral reply is used. The
//! rule of thumb is the spec's: when in doubt, do not echo. In Spanish the
//! person lives in the verb, so a verb is reflected only when the tables
//! know its form *and* its position says it is a verb with the user as
//! subject; anything that might be a first-person verb the tables do not
//! know blocks.

use std::collections::{HashMap, HashSet};

use super::script::{RawReflect, RawVerbs};
use crate::Lang;
use crate::normalise::fold;

/// The most words an echo may have (reflector.md 3.1).
pub(crate) const MAX_ECHO_WORDS: usize = 12;

/// What reflecting a fragment gave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Reflection {
    /// The echo, or why there is none.
    pub(crate) echo: Result<String, String>,
    /// The fragment speaks in the first person (a pronoun, a possessive or
    /// a verb form of the first person, known or likely).
    pub(crate) first: bool,
    /// The fragment has a second-person word: it is about Talos.
    pub(crate) second: bool,
}

/// How a Spanish verb form relates to its persons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Only the first person singular has it ("tengo", "llegué").
    Own,
    /// The first and the third person share it ("tenía", "fuera"):
    /// reflected only after an explicit "yo".
    Shared,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Entry {
    Form {
        second: String,
        kind: Kind,
        /// A verb like *gustar*: after "me", a shared form has the thing
        /// as subject and is copied ("me gustaría" → "te gustaría").
        dative: bool,
    },
    /// Two verbs (or two persons) give different readings: never echoed.
    Ambiguous,
}

/// The compiled person tables of one language.
#[derive(Debug)]
pub(crate) struct Tables {
    lang: Lang,
    words: HashMap<String, String>,
    phrases: Vec<(Vec<String>, Vec<String>)>,
    block: HashSet<String>,
    block_phrases: Vec<Vec<String>>,
    drop_phrases: Vec<Vec<String>>,
    offensive: HashSet<String>,
    determiners: HashSet<String>,
    prepositions: HashSet<String>,
    verb_context: HashSet<String>,
    auxiliaries: HashSet<String>,
    plain: HashSet<String>,
    verbs: HashMap<String, Entry>,
    /// Second-person verb forms, folded: a fragment with one is about
    /// Talos.
    second_forms: HashSet<String>,
    after_i: HashMap<String, String>,
    bad_after_i: HashSet<String>,
}

fn key(word: &str) -> String {
    fold(&word.to_lowercase().replace('\u{2019}', "'"))
}

fn set(words: &[String]) -> HashSet<String> {
    words.iter().map(|w| key(w)).collect()
}

fn phrase(text: &str) -> Vec<String> {
    text.split_whitespace().map(key).collect()
}

/// Replaces the last occurrence of `from` in `stem`.
fn change_last(stem: &str, from: char, to: &str) -> Option<String> {
    let at = stem.rfind(from)?;
    Some(format!(
        "{}{to}{}",
        &stem[..at],
        &stem[at + from.len_utf8()..]
    ))
}

/// Spelling of an *-ar* stem before *e*: c → qu, g → gu, z → c.
fn before_e(stem: &str) -> String {
    if let Some(s) = stem.strip_suffix('c') {
        format!("{s}qu")
    } else if let Some(s) = stem.strip_suffix('g') {
        format!("{s}gu")
    } else if let Some(s) = stem.strip_suffix('z') {
        format!("{s}c")
    } else {
        stem.to_owned()
    }
}

/// A generated form: first person, second person, and whether only the
/// first person has it.
type Form = (String, String, Kind);

/// The forms of a regular verb written as `infinitive/flag/flag`, and
/// whether it is a verb like *gustar*.
#[expect(
    clippy::too_many_lines,
    reason = "one conjugation table, tense by tense, reads best in one place"
)]
fn conjugate(entry: &str) -> Result<(Vec<Form>, bool), String> {
    let mut parts = entry.split('/');
    let infinitive = parts.next().unwrap_or_default().trim().to_owned();
    let flags: Vec<&str> = parts.map(str::trim).collect();
    let (stem, ar) = if let Some(stem) = infinitive.strip_suffix("ar") {
        (stem.to_owned(), true)
    } else if let Some(stem) = infinitive
        .strip_suffix("er")
        .or_else(|| infinitive.strip_suffix("ir"))
    {
        (stem.to_owned(), false)
    } else {
        return Err(format!("verb `{entry}`: not an infinitive"));
    };
    if stem.is_empty() {
        return Err(format!("verb `{entry}`: no stem"));
    }
    let ir = infinitive.ends_with("ir");
    let mut present = stem.clone();
    let mut zc = false;
    let mut y = false;
    let mut dative = false;
    for flag in &flags {
        let changed = match *flag {
            "ie" => change_last(&present, 'e', "ie"),
            "ue" => change_last(&present, 'o', "ue").or_else(|| change_last(&present, 'u', "ue")),
            "i" => change_last(&present, 'e', "i"),
            "í" => change_last(&present, 'i', "í"),
            "ú" => change_last(&present, 'u', "ú"),
            "zc" => {
                zc = true;
                Some(present.clone())
            }
            "y" => {
                y = true;
                Some(present.clone())
            }
            "dat" => {
                dative = true;
                Some(present.clone())
            }
            other => return Err(format!("verb `{entry}`: unknown flag `{other}`")),
        };
        present = changed.ok_or_else(|| format!("verb `{entry}`: flag `{flag}` does not apply"))?;
    }
    // The first person's stem in the present, with its spelling.
    let first_stem = if zc {
        format!("{}zc", present.strip_suffix('c').unwrap_or(&present))
    } else if y {
        format!("{present}y")
    } else if !ar && present.ends_with('g') {
        format!("{}j", &present[..present.len() - 1])
    } else if ir && present.ends_with("gu") {
        present[..present.len() - 1].to_owned()
    } else {
        present.clone()
    };
    let second_present = if y {
        format!("{present}yes")
    } else if ar {
        format!("{present}as")
    } else {
        format!("{present}es")
    };
    let mut forms = vec![(format!("{first_stem}o"), second_present, Kind::Own)];
    // Present subjunctive, shared with the third person.
    let subjunctive = if ar {
        format!("{}e", before_e(&first_stem))
    } else {
        format!("{first_stem}a")
    };
    forms.push((subjunctive.clone(), format!("{subjunctive}s"), Kind::Shared));
    // Preterite; stems ending in a vowel ("leer") stress differently and
    // are listed as irregular instead.
    if ar {
        forms.push((
            format!("{}é", before_e(&stem)),
            format!("{stem}aste"),
            Kind::Own,
        ));
    } else if !stem.ends_with(['a', 'e', 'i', 'o', 'u']) {
        forms.push((format!("{stem}í"), format!("{stem}iste"), Kind::Own));
    }
    // Future, imperfect, conditional, imperfect subjunctive.
    forms.push((
        format!("{infinitive}é"),
        format!("{infinitive}ás"),
        Kind::Own,
    ));
    let imperfect = if ar { "aba" } else { "ía" };
    forms.push((
        format!("{stem}{imperfect}"),
        format!("{stem}{imperfect}s"),
        Kind::Shared,
    ));
    forms.push((
        format!("{infinitive}ía"),
        format!("{infinitive}ías"),
        Kind::Shared,
    ));
    for ending in if ar { ["ara", "ase"] } else { ["iera", "iese"] } {
        forms.push((
            format!("{stem}{ending}"),
            format!("{stem}{ending}s"),
            Kind::Shared,
        ));
    }
    Ok((forms, dative))
}

impl Tables {
    /// Compiles a script's tables.
    ///
    /// # Errors
    ///
    /// A message naming a malformed verb entry.
    pub(crate) fn compile(lang: Lang, raw: &RawReflect) -> Result<Self, String> {
        let mut tables = Self {
            lang,
            words: raw.words.iter().map(|(k, v)| (key(k), v.clone())).collect(),
            phrases: raw
                .phrases
                .iter()
                .map(|(k, v)| (phrase(k), v.split_whitespace().map(str::to_owned).collect()))
                .collect(),
            block: set(&raw.block),
            block_phrases: raw.block_phrases.iter().map(|p| phrase(p)).collect(),
            drop_phrases: raw.drop_phrases.iter().map(|p| phrase(p)).collect(),
            offensive: set(&raw.offensive),
            determiners: set(&raw.determiners),
            prepositions: set(&raw.prepositions),
            verb_context: set(&raw.verb_context),
            auxiliaries: set(&raw.auxiliaries),
            // Matched with accents: "se" is plain, "sé" is a verb.
            plain: raw.plain.iter().map(|w| w.to_lowercase()).collect(),
            verbs: HashMap::new(),
            second_forms: HashSet::new(),
            after_i: raw
                .after_i
                .iter()
                .map(|(k, v)| (key(k), v.clone()))
                .collect(),
            bad_after_i: set(&raw.bad_after_i),
        };
        // Longest phrases first, so "yo diría que" wins over "diría que".
        tables
            .phrases
            .sort_by_key(|(from, _)| std::cmp::Reverse(from.len()));
        tables.add_verbs(&raw.verbs)?;
        Ok(tables)
    }

    fn add_verbs(&mut self, verbs: &RawVerbs) -> Result<(), String> {
        for entry in &verbs.regular {
            let (forms, dative) = conjugate(entry)?;
            for (first, second, kind) in forms {
                self.add_form(&first, &second, kind, dative);
            }
        }
        for (first, second) in &verbs.irregular {
            self.add_form(first, second, Kind::Own, false);
        }
        for (first, second) in &verbs.shared {
            self.add_form(first, second, Kind::Shared, false);
        }
        Ok(())
    }

    /// Records a form under its spelling and its folded spelling; a key
    /// that two different readings claim becomes ambiguous.
    fn add_form(&mut self, first: &str, second: &str, kind: Kind, dative: bool) {
        self.second_forms.insert(key(second));
        let entry = Entry::Form {
            second: second.to_owned(),
            kind,
            dative,
        };
        let lower = first.to_lowercase();
        let folded = fold(&lower);
        for k in [lower, folded] {
            match self.verbs.get(&k) {
                None => {
                    self.verbs.insert(k, entry.clone());
                }
                Some(existing) if *existing == entry => {}
                Some(Entry::Form {
                    second: s,
                    kind: existing_kind,
                    dative: d,
                }) if *s == second && *d == dative => {
                    // Same reading from two tenses: the stricter kind wins.
                    let kind = if *existing_kind == Kind::Shared || kind == Kind::Shared {
                        Kind::Shared
                    } else {
                        Kind::Own
                    };
                    self.verbs.insert(
                        k,
                        Entry::Form {
                            second: second.to_owned(),
                            kind,
                            dative,
                        },
                    );
                }
                Some(_) => {
                    self.verbs.insert(k, Entry::Ambiguous);
                }
            }
        }
    }

    /// Reflects a fragment. `before` is the word that precedes it in the
    /// sentence, if any: it decides whether the first word is a verb.
    pub(crate) fn reflect(&self, fragment: &str, before: Option<&str>) -> Reflection {
        let fragment = fragment.trim_matches(|c: char| {
            c.is_whitespace() || matches!(c, ',' | ';' | ':' | '.' | '…' | '-')
        });
        let mut tokens = tokenize(fragment, self.lang);
        self.drop_fillers(&mut tokens);
        let mut out = Reflection {
            echo: Ok(String::new()),
            first: false,
            second: false,
        };
        if let Err(why) = content_allowed(fragment, &tokens) {
            out.echo = Err(why);
            // Still look at the persons, for rules that need to know.
            let _ = self.walk(&tokens, before, &mut out);
            return out;
        }
        out.echo = self.walk(&tokens, before, &mut out);
        out
    }

    /// Whether a sentence has any first- or second-person word.
    pub(crate) fn has_person(&self, sentence: &str) -> bool {
        let r = self.reflect(sentence, None);
        r.first || r.second
    }

    fn drop_fillers(&self, tokens: &mut Vec<Token>) {
        for filler in &self.drop_phrases {
            while let Some(at) = find_phrase(tokens, filler) {
                let words = word_positions(tokens);
                let start = words[at];
                let end = words[at + filler.len() - 1];
                // Take one separator with it.
                let end = if end + 1 < tokens.len() { end + 1 } else { end };
                tokens.drain(start..=end);
            }
        }
    }

    fn walk(
        &self,
        tokens: &[Token],
        before: Option<&str>,
        out: &mut Reflection,
    ) -> Result<String, String> {
        for blocked in &self.block_phrases {
            if find_phrase(tokens, blocked).is_some() {
                return Err(format!("fixed phrase `{}`", blocked.join(" ")));
            }
        }
        let words: Vec<usize> = word_positions(tokens);
        if words.len() > MAX_ECHO_WORDS {
            return Err(format!("{} words, more than {MAX_ECHO_WORDS}", words.len()));
        }
        let mut result: Result<(), String> = Ok(());
        let mut text: Vec<String> = tokens.iter().map(|t| t.text.clone()).collect();
        let mut w = 0;
        while w < words.len() {
            // A phrase whose reading the group settles.
            if let Some((len, replacement)) = self.phrase_at(tokens, &words, w) {
                for (k, word) in replacement.iter().enumerate() {
                    let at = words[w + k];
                    text[at] = match_case(&tokens[at].text, word);
                }
                out.first = true;
                w += len;
                continue;
            }
            let at = words[w];
            let prev = if w == 0 {
                before.map(key)
            } else {
                Some(tokens[words[w - 1]].key.clone())
            };
            let next = words.get(w + 1).map(|&n| tokens[n].key.as_str());
            let step = match self.lang {
                Lang::Es => self.word_es(&tokens[at], prev.as_deref(), next, out),
                Lang::En => {
                    let prev_raw = if w == 0 {
                        None
                    } else {
                        Some(tokens[words[w - 1]].key.as_str())
                    };
                    self.word_en(tokens, &words, w, prev_raw, out)
                }
            };
            match step {
                Ok(Some(mapped)) => text[at] = match_case(&tokens[at].text, &mapped),
                Ok(None) => {}
                Err(why) => {
                    if result.is_ok() {
                        result = Err(why);
                    }
                }
            }
            w += 1;
        }
        result.map(|()| text.concat())
    }

    fn phrase_at(&self, tokens: &[Token], words: &[usize], w: usize) -> Option<(usize, &[String])> {
        self.phrases.iter().find_map(|(from, to)| {
            let fits = from.len() <= words.len() - w
                && from
                    .iter()
                    .enumerate()
                    .all(|(k, f)| tokens[words[w + k]].key == *f);
            (fits && from.len() == to.len()).then_some((from.len(), to.as_slice()))
        })
    }

    /// One Spanish word: `Ok(Some)` to replace it, `Ok(None)` to copy it,
    /// `Err` to block the echo.
    fn word_es(
        &self,
        token: &Token,
        prev: Option<&str>,
        next: Option<&str>,
        out: &mut Reflection,
    ) -> Result<Option<String>, String> {
        let k = token.key.as_str();
        let lower = token.text.to_lowercase();
        if self.offensive.contains(k) {
            return Err(format!("offensive word `{}`", token.text));
        }
        if self.plain.contains(&lower) {
            return Ok(None);
        }
        if self.block.contains(k) {
            out.second |= self.is_second_es(k);
            return Err(format!("blocked word `{}`", token.text));
        }
        if k == "mi" {
            out.first = true;
            // "mí" (or "mi" after a preposition with no noun after it) is
            // the pronoun; "mi" before a noun is the possessive.
            let pronoun = lower == "mí"
                || (prev.is_some_and(|p| self.prepositions.contains(p))
                    && next.is_none_or(|n| !self.is_noun_start(n)));
            return Ok(Some(if pronoun { "ti" } else { "tu" }.to_owned()));
        }
        if let Some(mapped) = self.words.get(k) {
            out.first = true;
            return Ok(Some(mapped.clone()));
        }
        let is_determiner = prev.is_some_and(|p| self.determiners.contains(p));
        let is_verb_context = prev.is_none_or(|p| self.verb_context.contains(p));
        let after_aux = prev.is_some_and(|p| self.auxiliaries.contains(p));
        let entry = self.verbs.get(&lower).or_else(|| self.verbs.get(k));
        if entry.is_some() && (after_aux || (is_determiner && !is_verb_context)) {
            // A participle, or a noun ("el trabajo", "mi viaje").
            return Ok(None);
        }
        match entry {
            // Two readings, or a form the third person shares: a problem
            // only where a verb with the user as subject could stand. After
            // a noun or an adjective ("en pleno monte", "mi madre
            // trabajaba") the subject, if any, is that word.
            Some(Entry::Ambiguous) if !is_verb_context => return Ok(None),
            Some(Entry::Ambiguous) => {
                out.first = true;
                return Err(format!("`{}` has two readings", token.text));
            }
            Some(Entry::Form {
                second,
                kind,
                dative,
            }) => match kind {
                Kind::Own => {
                    out.first = true;
                    if is_verb_context && !is_determiner {
                        return Ok(Some(second.clone()));
                    }
                    return Err(format!("`{}` may or may not be a verb", token.text));
                }
                Kind::Shared => {
                    if prev == Some("yo") {
                        out.first = true;
                        return Ok(Some(second.clone()));
                    }
                    if (prev == Some("me") && *dative) || !is_verb_context {
                        return Ok(None);
                    }
                    out.first |= prev == Some("me");
                    return Err(format!(
                        "`{}` is first or third person; the sentence does not say which",
                        token.text
                    ));
                }
            },
            None => {}
        }
        if self.second_forms.contains(k) && !is_determiner {
            out.second = true;
            return Err(format!("`{}` is a second-person verb", token.text));
        }
        if is_enclitic_group(&lower) {
            out.first |= k.ends_with("me") || k.contains("nos");
            return Err(format!("enclitic group `{}`", token.text));
        }
        // After "lo"/"la" (a clitic or an article), only a present or
        // preterite ending is suspicious ("lo arreglo"); "la lotería" is a
        // noun.
        let suspicious = if is_determiner {
            lower.chars().count() > 2
                && (lower.ends_with('o') || lower.ends_with('é') || lower.ends_with('í'))
        } else {
            looks_like_first_person_verb(&lower)
        };
        if is_verb_context && suspicious {
            out.first = true;
            return Err(format!(
                "`{}` may be a verb the tables do not know",
                token.text
            ));
        }
        if k.len() > 4
            && (k.ends_with("amos") || k.ends_with("emos") || k.ends_with("imos"))
            && !is_determiner
        {
            return Err(format!("`{}` may be first person plural", token.text));
        }
        Ok(None)
    }

    fn is_second_es(&self, k: &str) -> bool {
        matches!(
            k,
            "tu" | "tus"
                | "te"
                | "ti"
                | "contigo"
                | "tuyo"
                | "tuya"
                | "tuyos"
                | "tuyas"
                | "usted"
                | "ustedes"
                | "vosotros"
                | "vosotras"
                | "os"
                | "vuestro"
                | "vuestra"
                | "vuestros"
                | "vuestras"
        ) || self.second_forms.contains(k)
    }

    /// Whether a word can open a noun phrase after "mi" ("mi madre").
    fn is_noun_start(&self, word: &str) -> bool {
        !(self.verb_context.contains(word)
            || self.prepositions.contains(word)
            || self.words.contains_key(word)
            || matches!(word, "me" | "mi" | "no" | "y" | "o"))
    }

    /// One English word.
    fn word_en(
        &self,
        tokens: &[Token],
        words: &[usize],
        w: usize,
        prev: Option<&str>,
        out: &mut Reflection,
    ) -> Result<Option<String>, String> {
        let token = &tokens[words[w]];
        let k = token.key.as_str();
        let next = words.get(w + 1).map(|&n| tokens[n].key.as_str());
        if self.offensive.contains(k) {
            return Err(format!("offensive word `{}`", token.text));
        }
        if self.block.contains(k) {
            out.second |= k.starts_with('y') || k == "u" || k == "ur";
            return Err(format!("blocked word `{}`", token.text));
        }
        if k == "i" {
            out.first = true;
            if let Some(n) = next
                && self.bad_after_i.contains(n)
            {
                return Err(format!("`I {n}` is outside the tables"));
            }
            return Ok(Some("you".to_owned()));
        }
        if let Some(mapped) = self.after_i.get(k) {
            if prev == Some("i") || next == Some("i") {
                return Ok(Some(mapped.clone()));
            }
            // "was" with another subject stays; but not when an "I"
            // elsewhere may be its subject ("I really was late").
            let has_i = words.iter().any(|&p| tokens[p].key == "i");
            if k == "am" || has_i {
                return Err(format!("`{}` with no clear subject", token.text));
            }
            return Ok(None);
        }
        if let Some(mapped) = self.words.get(k) {
            out.first = true;
            return Ok(Some(mapped.clone()));
        }
        Ok(None)
    }
}

/// Words, and what lies between them.
#[derive(Debug, Clone)]
struct Token {
    text: String,
    /// Lower case, no accents; empty for a separator.
    key: String,
}

impl Token {
    fn is_word(&self) -> bool {
        !self.key.is_empty()
    }
}

fn tokenize(text: &str, lang: Lang) -> Vec<Token> {
    let in_word = |c: char, prev_alnum: bool, next: Option<char>| {
        c.is_alphanumeric()
            || (lang == Lang::En
                && matches!(c, '\'' | '\u{2019}')
                && prev_alnum
                && next.is_some_and(char::is_alphanumeric))
    };
    let chars: Vec<char> = text.chars().collect();
    let mut tokens: Vec<Token> = Vec::new();
    let mut current = String::new();
    let mut word = false;
    for (i, &c) in chars.iter().enumerate() {
        let prev_alnum = i > 0 && chars[i - 1].is_alphanumeric();
        let is = in_word(c, prev_alnum, chars.get(i + 1).copied());
        if !current.is_empty() && is != word {
            tokens.push(make_token(std::mem::take(&mut current), word));
        }
        word = is;
        current.push(c);
    }
    if !current.is_empty() {
        tokens.push(make_token(current, word));
    }
    tokens
}

fn make_token(text: String, word: bool) -> Token {
    let key = if word { key(&text) } else { String::new() };
    Token { text, key }
}

fn word_positions(tokens: &[Token]) -> Vec<usize> {
    tokens
        .iter()
        .enumerate()
        .filter(|(_, t)| t.is_word())
        .map(|(i, _)| i)
        .collect()
}

/// The word index where a phrase starts, if it occurs.
fn find_phrase(tokens: &[Token], phrase: &[String]) -> Option<usize> {
    if phrase.is_empty() {
        return None;
    }
    let words = word_positions(tokens);
    (0..words.len()).find(|&w| {
        phrase.len() <= words.len() - w
            && phrase
                .iter()
                .enumerate()
                .all(|(k, p)| tokens[words[w + k]].key == *p)
    })
}

/// Content that is never echoed (reflector.md 3.1): quoted speech, long
/// numbers, e-mail addresses, URLs, anything that looks like code.
fn content_allowed(fragment: &str, tokens: &[Token]) -> Result<(), String> {
    if fragment.contains(['"', '«', '»', '“', '”', '„']) {
        return Err("quoted speech".to_owned());
    }
    if fragment.contains('@') || fragment.contains("http") || fragment.contains("www.") {
        return Err("an address".to_owned());
    }
    if fragment.contains([
        '{', '}', '<', '>', '=', '_', '\\', '|', '#', '$', '%', '^', '*', '~', '`', '/',
    ]) {
        return Err("looks like code".to_owned());
    }
    for token in tokens.iter().filter(|t| t.is_word()) {
        let digits = token.text.chars().filter(char::is_ascii_digit).count();
        if digits > 4 {
            return Err("a long number".to_owned());
        }
        if digits > 0 && digits < token.text.chars().count() {
            return Err("letters and digits mixed".to_owned());
        }
        if token.text.starts_with('\'') {
            return Err("quoted speech".to_owned());
        }
    }
    Ok(())
}

/// A Spanish infinitive, gerund or imperative carrying a first- or
/// second-person clitic, or two clitics ("apuntarme", "comprármelo",
/// "dímelo", "regalárselo"): the echo would need the accent moved.
fn is_enclitic_group(word: &str) -> bool {
    const CLITICS: [&str; 10] = [
        "me", "te", "se", "nos", "os", "lo", "la", "los", "las", "le",
    ];
    let accented = word.contains(['á', 'é', 'í', 'ó', 'ú']);
    let folded = fold(word);
    let ends_clitic = CLITICS.iter().any(|c| folded.ends_with(c));
    if accented && ends_clitic && folded.len() > 4 {
        return true;
    }
    for base in ["ar", "er", "ir", "ando", "iendo", "yendo"] {
        for clitic in ["me", "te", "nos", "os"] {
            let suffix = format!("{base}{clitic}");
            if let Some(stem) = folded.strip_suffix(&suffix) {
                // Not "duerme", "convierte", "firme": no infinitive there.
                if stem.len() >= 2 && !stem.ends_with(['i', 'u']) {
                    return true;
                }
            }
        }
        for first in ["me", "te", "se", "nos"] {
            for second in ["lo", "la", "los", "las", "le", "les"] {
                if folded.ends_with(&format!("{base}{first}{second}")) {
                    return true;
                }
            }
        }
    }
    false
}

/// A word that could be a first-person (or first/third shared) Spanish
/// verb form: present in *-o*, *-oy*; preterite or future with a final
/// stressed vowel; imperfect, conditional and subjunctive endings.
fn looks_like_first_person_verb(word: &str) -> bool {
    if word.chars().count() < 3 {
        return false;
    }
    let folded = fold(word);
    folded.ends_with('o')
        || folded.ends_with("oy")
        || word.ends_with('é')
        || (word.ends_with('í') && word != "sí")
        || folded.ends_with("aba")
        || word.ends_with("ía")
        || folded.ends_with("iera")
        || folded.ends_with("iese")
        || folded.ends_with("ase")
}

/// The replacement, capitalised like the word it replaces.
fn match_case(original: &str, replacement: &str) -> String {
    let pronoun_i = original == "I" || original.starts_with("I'");
    if original.chars().next().is_some_and(char::is_uppercase) && !pronoun_i {
        let mut chars = replacement.chars();
        chars
            .next()
            .map_or_else(String::new, |c| c.to_uppercase().chain(chars).collect())
    } else {
        replacement.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn es() -> Tables {
        let raw: RawReflect = toml::from_str(
            r#"
words = { yo = "tú", me = "te", mis = "tus", conmigo = "contigo", "mío" = "tuyo" }
phrases = { "creo que" = "crees que" }
block = ["tú", "tu", "te", "ti", "nosotros", "siento", "talos"]
determiners = ["el", "la", "lo", "un", "una", "mi", "su", "del", "al"]
prepositions = ["a", "de", "con", "para", "por", "en"]
verb_context = ["yo", "me", "que", "no", "ya", "la", "lo", "y", "ayer"]
auxiliaries = ["he", "has", "ha"]
plain = ["suerte", "qué"]
offensive = ["imbécil"]
[verbs]
regular = ["trabajar", "llegar", "pensar/ie", "coger", "exagerar", "quedar", "gustar/dat", "tocar/dat", "soñar/ue", "vivir"]
irregular = { tengo = "tienes", estuve = "estuviste", he = "has", soy = "eres" }
shared = { fuera = "fueras" }
"#,
        )
        .unwrap();
        Tables::compile(Lang::Es, &raw).unwrap()
    }

    fn echo(t: &Tables, text: &str) -> Option<String> {
        t.reflect(text, None).echo.ok()
    }

    #[test]
    fn regular_verbs_conjugate() {
        let (forms, dative) = conjugate("pensar/ie").unwrap();
        assert!(!dative);
        assert!(forms.contains(&("pienso".to_owned(), "piensas".to_owned(), Kind::Own)));
        assert!(forms.contains(&("pensé".to_owned(), "pensaste".to_owned(), Kind::Own)));
        assert!(forms.contains(&("pensaba".to_owned(), "pensabas".to_owned(), Kind::Shared)));
        let (forms, _) = conjugate("coger").unwrap();
        assert!(forms.contains(&("cojo".to_owned(), "coges".to_owned(), Kind::Own)));
        let (forms, _) = conjugate("llegar").unwrap();
        assert!(forms.contains(&("llegué".to_owned(), "llegaste".to_owned(), Kind::Own)));
        assert!(forms.contains(&("llegue".to_owned(), "llegues".to_owned(), Kind::Shared)));
        let (forms, _) = conjugate("enviar/í").unwrap();
        assert!(forms.contains(&("envío".to_owned(), "envías".to_owned(), Kind::Own)));
        let (forms, _) = conjugate("conocer/zc").unwrap();
        assert!(forms.contains(&("conozco".to_owned(), "conoces".to_owned(), Kind::Own)));
        assert!(conjugate("casa").is_err());
        assert!(conjugate("pensar/xx").is_err());
    }

    #[test]
    fn spanish_persons_swap() {
        let t = es();
        assert_eq!(
            echo(&t, "me cojo el día libre").unwrap(),
            "te coges el día libre"
        );
        assert_eq!(
            echo(&t, "mi jefe dice que tengo que venir").unwrap(),
            "tu jefe dice que tienes que venir"
        );
        assert_eq!(
            echo(&t, "mi novio dice que exagero").unwrap(),
            "tu novio dice que exageras"
        );
        assert_eq!(
            t.reflect("estuve en el Prado", Some("ayer")).echo.unwrap(),
            "estuviste en el Prado"
        );
        assert_eq!(echo(&t, "para mi").unwrap(), "para ti");
        assert_eq!(echo(&t, "a mi me pasa").unwrap(), "a ti te pasa");
        assert_eq!(echo(&t, "Mis amigos").unwrap(), "Tus amigos");
        assert_eq!(echo(&t, "creo que llueve").unwrap(), "crees que llueve");
        // A noun after a determiner, a participle after an auxiliary.
        assert_eq!(echo(&t, "mi trabajo").unwrap(), "tu trabajo");
        // Copied as written.
        assert_eq!(echo(&t, "qué suerte").unwrap(), "qué suerte");
        // "me" with a verb like gustar keeps the shared form.
        assert_eq!(
            echo(&t, "me tocara la lotería").unwrap(),
            "te tocara la lotería"
        );
        assert_eq!(echo(&t, "yo fuera rico").unwrap(), "tú fueras rico");
    }

    #[test]
    fn spanish_echo_is_blocked_when_in_doubt() {
        let t = es();
        for text in [
            "me siento fatal",        // two readings
            "tengo sueño",            // a noun or a verb
            "voy a comprármelo",      // enclitic group
            "apuntarme a la EOI",     // first-person enclitic
            "eres lento",             // second person (a known form)
            "tú sabrás",              // second person
            "fuera tarde",            // first or third person
            "llegue tarde",           // accent missing: preterite or subjunctive
            "nosotros vamos",         // first person plural
            "hablo con mi madre",     // a verb the tables do not know
            "me dijo «no vuelvas»",   // quoted speech
            "mi número es 600123456", // a long number
            "eres imbécil",           // never echoed
            "uno dos tres cuatro cinco seis siete ocho nueve diez once doce trece",
        ] {
            assert!(
                echo(&t, text).is_none(),
                "{text}: {:?}",
                t.reflect(text, None)
            );
        }
    }

    #[test]
    fn persons_are_reported() {
        let t = es();
        assert!(t.has_person("me gusta"));
        assert!(t.has_person("tú sabrás"));
        assert!(!t.has_person("el agua sabe a cloro"));
    }

    fn en() -> Tables {
        let raw: RawReflect = toml::from_str(
            r#"
words = { me = "you", my = "your", mine = "yours", myself = "yourself", "i'm" = "you're", "i've" = "you've", "i'd" = "you'd", "i'll" = "you'll" }
block = ["you", "your", "you're", "we", "us", "our"]
block_phrases = ["let me", "excuse me"]
drop_phrases = ["you know"]
after_i = { am = "are", was = "were", "wasn't" = "weren't" }
bad_after_i = ["were", "is", "has"]
"#,
        )
        .unwrap();
        Tables::compile(Lang::En, &raw).unwrap()
    }

    #[test]
    fn english_persons_swap() {
        let t = en();
        assert_eq!(
            echo(&t, "I was sure I'd locked the door").unwrap(),
            "you were sure you'd locked the door"
        );
        assert_eq!(
            echo(&t, "me and my brother").unwrap(),
            "you and your brother"
        );
        assert_eq!(
            echo(&t, "the train was late").unwrap(),
            "the train was late"
        );
        assert_eq!(echo(&t, "am I late").unwrap(), "are you late");
        assert_eq!(echo(&t, "it's, you know, mine").unwrap(), "it's, yours");
        for text in [
            "I were that tired",
            "let me think",
            "you're wrong",
            "we went",
            "I really was late",
        ] {
            assert!(echo(&t, text).is_none(), "{text}");
        }
    }
}
