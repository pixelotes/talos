//! Choosing a sense (docs/spec/knowledge.md 2.2; KNOWLEDGE.md section 5).
//!
//! The caller brings what the conversation knows ([`Cues`]: the sense
//! remembered, the turn's other words, the concepts of recent turns, the
//! relation asked for); the store brings glosses and claims. The first
//! step that decides, decides; when none does, the result carries what is
//! needed to ask "which one?": every sense named by its nearest class, or
//! by the first words of its gloss.

use std::collections::BTreeSet;

use super::query::{ConceptId, Sense, Word};
use super::store::Knowledge;
use crate::normalise::normalise;
use crate::{Error, Lang};

/// The relation a question asks about, for step 4 (structural fit): "is it
/// a kind of furniture?" asks for an `is_a` claim with the sense as
/// subject; "what kinds of X are there?" one with the sense as object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Relation<'a> {
    /// The predicate (`is_a`, `part_of`, `causes`).
    pub predicate: &'a str,
    /// Whether the sense is the claim's subject (else its object).
    pub as_subject: bool,
}

/// What the conversation offers for choosing among a word's senses.
#[derive(Debug, Clone, Copy)]
pub struct Cues<'a> {
    /// The conversation's language: labels are written in it.
    pub lang: Lang,
    /// Step 1: the concept chosen for this word earlier in the
    /// conversation, if one was.
    pub remembered: Option<&'a ConceptId>,
    /// Step 2: the words of the turn other than the word itself ("banco …
    /// dinero" gives "dinero").
    pub context: &'a str,
    /// Step 3: concepts discussed in the previous three turns.
    pub recent: &'a [ConceptId],
    /// Step 4: the relation the question asks for, if any.
    pub relation: Option<Relation<'a>>,
}

impl<'a> Cues<'a> {
    /// No cues: only the baseline rank can decide.
    #[must_use]
    pub const fn none(lang: Lang) -> Cues<'a> {
        Cues {
            lang,
            remembered: None,
            context: "",
            recent: &[],
            relation: None,
        }
    }
}

/// The step of 2.2 that decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// 1: the sense remembered in this conversation.
    Remembered,
    /// 2: words of this turn.
    Turn,
    /// 3: concepts of the previous turns.
    Recent,
    /// 4: only one sense can answer the relation asked.
    Structure,
    /// 5: one sense has a strictly lower rank than every other.
    Rank,
}

/// How a sense is named in a question or in "it has other senses".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Label {
    /// By its nearest class ("la entidad financiera", "the seat").
    Class {
        /// The class.
        concept: ConceptId,
        /// Its word, in the conversation's language when it has one.
        word: Word,
    },
    /// By the first words of its gloss, when it has no class.
    Gloss(String),
    /// Neither: only the concept is known.
    None,
}

/// One sense, as offered to the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SenseOption {
    /// The meaning.
    pub concept: ConceptId,
    /// How to name it.
    pub label: Label,
}

/// The result of choosing a sense.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SenseChoice {
    /// The chosen sense (an index into the senses given) and the step that
    /// chose it; `None` when Talos must ask (step 6).
    pub chosen: Option<(usize, Step)>,
    /// Every sense, in the order given, with its label: the options of the
    /// question, or the other senses to mention.
    pub options: Vec<SenseOption>,
}

/// How many words of a gloss name a sense that has no class.
const GLOSS_WORDS: usize = 4;

/// Words too common to count as context.
const STOP: &[&str] = &[
    // es
    "que",
    "qué",
    "una",
    "uno",
    "unos",
    "unas",
    "los",
    "las",
    "del",
    "con",
    "por",
    "para",
    "como",
    "cual",
    "cuál",
    "es",
    "son",
    "era",
    "fue",
    "ser",
    "esta",
    "este",
    "eso",
    "esto",
    "esa",
    "ese",
    "hay",
    "muy",
    "mas",
    "pero",
    "sus",
    "su",
    "lo",
    "le",
    "les",
    "al",
    "de",
    "la",
    "el",
    "en",
    "y",
    "o",
    "se",
    "me",
    "te",
    "mi",
    "tu",
    "significa",
    "quiere",
    "decir",
    "dime",
    "sobre",
    // en
    "the",
    "and",
    "for",
    "with",
    "what",
    "which",
    "who",
    "this",
    "that",
    "these",
    "those",
    "are",
    "was",
    "were",
    "is",
    "be",
    "been",
    "has",
    "have",
    "had",
    "does",
    "did",
    "its",
    "it",
    "of",
    "to",
    "in",
    "on",
    "at",
    "by",
    "an",
    "a",
    "or",
    "as",
    "from",
    "about",
    "mean",
    "means",
    "tell",
    "me",
    "you",
    "your",
    "my",
];

/// The content words of a text, in the match form.
fn content_words(text: &str) -> BTreeSet<String> {
    normalise(text)
        .folded
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 3 && !STOP.contains(w))
        .map(str::to_owned)
        .collect()
}

/// Whether two content words are the same word, allowing a plural `s`.
fn same_word(a: &str, b: &str) -> bool {
    a == b || a.strip_suffix('s') == Some(b) || b.strip_suffix('s') == Some(a)
}

/// The index of the only sense with the highest positive score.
fn unique_best(scores: &[usize]) -> Option<usize> {
    let best = *scores.iter().max()?;
    (best > 0 && scores.iter().filter(|s| **s == best).count() == 1)
        .then(|| scores.iter().position(|s| *s == best))
        .flatten()
}

impl Knowledge {
    /// The concepts within two held claims of `concept`, itself included.
    fn within_two(&self, concept: &ConceptId) -> Result<BTreeSet<ConceptId>, Error> {
        let mut out = BTreeSet::from([concept.clone()]);
        let first = self.neighbours(concept)?;
        for near in &first {
            out.extend(self.neighbours(near)?);
        }
        out.extend(first);
        Ok(out)
    }

    /// How a sense is named: its first class, in `lang` when that class has
    /// a word there; else the first words of its gloss in `lang`.
    ///
    /// # Errors
    ///
    /// [`Error::Knowledge`] when a database cannot be read.
    pub fn label(&self, concept: &ConceptId, lang: Lang) -> Result<Label, Error> {
        for class in self.objects(concept, "is_a")? {
            if let Some(word) = self.name(&class, lang)? {
                return Ok(Label::Class {
                    concept: class,
                    word,
                });
            }
        }
        if let Some(gloss) = self.gloss(concept, lang)? {
            let words: Vec<&str> = gloss.text.split_whitespace().collect();
            let mut text = words[..words.len().min(GLOSS_WORDS)].join(" ");
            if words.len() > GLOSS_WORDS {
                text.push('…');
            }
            return Ok(Label::Gloss(text));
        }
        Ok(Label::None)
    }

    /// Context scores of each sense: for each content word of `context`,
    /// one point when it occurs in the sense's gloss (either language), and
    /// one when it is a word for a concept within two links of the sense.
    fn context_scores(&self, senses: &[Sense], context: &str) -> Result<Vec<usize>, Error> {
        let words = content_words(context);
        let mut scores = vec![0; senses.len()];
        if words.is_empty() {
            return Ok(scores);
        }
        let mut named: Vec<BTreeSet<ConceptId>> = Vec::with_capacity(words.len());
        for word in &words {
            let mut concepts = BTreeSet::new();
            for lang in Lang::ALL {
                concepts.extend(self.senses_in(word, lang)?.into_iter().map(|s| s.concept));
            }
            named.push(concepts);
        }
        for (i, sense) in senses.iter().enumerate() {
            let mut gloss_words = BTreeSet::new();
            for lang in Lang::ALL {
                if let Some(gloss) = self.gloss(&sense.concept, lang)? {
                    gloss_words.extend(content_words(&gloss.text));
                }
            }
            let near = self.within_two(&sense.concept)?;
            for (word, concepts) in words.iter().zip(&named) {
                if gloss_words.iter().any(|g| same_word(g, word)) {
                    scores[i] += 1;
                }
                if concepts
                    .iter()
                    .any(|c| near.contains(c) && *c != sense.concept)
                {
                    scores[i] += 1;
                }
            }
        }
        Ok(scores)
    }

    /// Chooses among a word's senses (as [`Knowledge::lookup`] gives them)
    /// by the steps of docs/spec/knowledge.md 2.2, the first that decides
    /// deciding: the remembered sense, context in the turn, recent context,
    /// structural fit, a strict rank winner; otherwise ask. Every sense is
    /// returned with its label either way.
    ///
    /// # Errors
    ///
    /// [`Error::Knowledge`] when a database cannot be read.
    pub fn choose_sense(&self, senses: &[Sense], cues: &Cues<'_>) -> Result<SenseChoice, Error> {
        let mut options = Vec::with_capacity(senses.len());
        for sense in senses {
            options.push(SenseOption {
                concept: sense.concept.clone(),
                label: self.label(&sense.concept, cues.lang)?,
            });
        }
        let chosen = self.decide(senses, cues)?;
        Ok(SenseChoice { chosen, options })
    }

    fn decide(&self, senses: &[Sense], cues: &Cues<'_>) -> Result<Option<(usize, Step)>, Error> {
        if senses.len() == 1 {
            return Ok(Some((0, Step::Rank)));
        }
        if senses.is_empty() {
            return Ok(None);
        }
        if let Some(remembered) = cues.remembered
            && let Some(i) = senses.iter().position(|s| &s.concept == remembered)
        {
            return Ok(Some((i, Step::Remembered)));
        }
        if let Some(i) = unique_best(&self.context_scores(senses, cues.context)?) {
            return Ok(Some((i, Step::Turn)));
        }
        if !cues.recent.is_empty() {
            let mut scores = Vec::with_capacity(senses.len());
            for sense in senses {
                let near = self.within_two(&sense.concept)?;
                scores.push(cues.recent.iter().filter(|c| near.contains(c)).count());
            }
            if let Some(i) = unique_best(&scores) {
                return Ok(Some((i, Step::Recent)));
            }
        }
        if let Some(relation) = cues.relation {
            let mut fits = Vec::with_capacity(senses.len());
            for sense in senses {
                fits.push(usize::from(self.has_claim(
                    &sense.concept,
                    relation.predicate,
                    relation.as_subject,
                )?));
            }
            if fits.iter().sum::<usize>() == 1
                && let Some(i) = fits.iter().position(|f| *f == 1)
            {
                return Ok(Some((i, Step::Structure)));
            }
        }
        let lowest = senses.iter().map(|s| s.rank).min().unwrap_or(0);
        let at_lowest: Vec<usize> = (0..senses.len())
            .filter(|i| senses[*i].rank == lowest)
            .collect();
        Ok((at_lowest.len() == 1).then(|| (at_lowest[0], Step::Rank)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEED: &str = include_str!("../../../../knowledge/seed.toml");

    fn choose(k: &Knowledge, word: &str, lang: Lang, cues: &Cues<'_>) -> SenseChoice {
        let senses = k.lookup(word, lang).unwrap().unwrap().senses;
        k.choose_sense(&senses, cues).unwrap()
    }

    fn chosen(k: &Knowledge, word: &str, lang: Lang, cues: &Cues<'_>) -> Option<(String, Step)> {
        let choice = choose(k, word, lang, cues);
        choice
            .chosen
            .map(|(i, step)| (choice.options[i].concept.0.clone(), step))
    }

    #[test]
    fn a_tie_at_the_top_asks_with_each_sense_named() {
        let k = Knowledge::from_source(SEED).unwrap();
        let choice = choose(&k, "banco", Lang::Es, &Cues::none(Lang::Es));
        assert_eq!(choice.chosen, None);
        let labels: Vec<String> = choice
            .options
            .iter()
            .map(|o| match &o.label {
                Label::Class { word, .. } => word.lemma.clone(),
                Label::Gloss(g) => g.clone(),
                Label::None => String::new(),
            })
            .collect();
        assert_eq!(labels, ["entidad financiera", "asiento", "grupo"]);
        let cold = choose(&k, "cold", Lang::En, &Cues::none(Lang::En));
        assert_eq!(cold.chosen, None);
        assert_eq!(
            cold.options[0].label,
            Label::Gloss("a low temperature".to_owned())
        );
        assert_eq!(
            cold.options[1].label,
            Label::Gloss("a mild infection of…".to_owned())
        );
    }

    #[test]
    fn the_first_step_that_decides_decides() {
        let k = Knowledge::from_source(SEED).unwrap();
        let es = Cues::none(Lang::Es);
        let en = Cues::none(Lang::En);
        // 5: a strict rank winner.
        assert_eq!(
            chosen(&k, "neck", Lang::En, &en),
            Some(("core:neck_body".to_owned(), Step::Rank))
        );
        assert_eq!(
            chosen(&k, "cuerda", Lang::Es, &es),
            Some(("core:rope".to_owned(), Step::Rank))
        );
        // 2: words of the turn, in a gloss or naming a concept two links away.
        let turn = |context| Cues { context, ..es };
        assert_eq!(
            chosen(&k, "banco", Lang::Es, &turn("¿dónde guardo el dinero?")),
            Some(("core:bank".to_owned(), Step::Turn))
        );
        assert_eq!(
            chosen(&k, "cuerda", Lang::Es, &turn("de la guitarra")),
            Some(("core:string".to_owned(), Step::Turn))
        );
        assert_eq!(
            chosen(
                &k,
                "bank",
                Lang::En,
                &Cues {
                    context: "of the river",
                    ..en
                }
            ),
            Some(("core:riverbank".to_owned(), Step::Turn))
        );
        // 1: the remembered sense wins over the turn.
        let bench = ConceptId::new("core:bench");
        assert_eq!(
            chosen(
                &k,
                "banco",
                Lang::Es,
                &Cues {
                    remembered: Some(&bench),
                    context: "dinero",
                    ..es
                }
            ),
            Some(("core:bench".to_owned(), Step::Remembered))
        );
        // 3: recent concepts.
        let recent = [ConceptId::new("core:furniture")];
        assert_eq!(
            chosen(
                &k,
                "banco",
                Lang::Es,
                &Cues {
                    recent: &recent,
                    ..es
                }
            ),
            Some(("core:bench".to_owned(), Step::Recent))
        );
        // 4: only one sense can answer the relation asked.
        let part_of = Relation {
            predicate: "part_of",
            as_subject: true,
        };
        assert_eq!(
            chosen(
                &k,
                "bank",
                Lang::En,
                &Cues {
                    relation: Some(part_of),
                    ..en
                }
            ),
            Some(("core:riverbank".to_owned(), Step::Structure))
        );
        // Stop words decide nothing.
        assert_eq!(chosen(&k, "banco", Lang::Es, &turn("¿qué es un?")), None);
    }
}
