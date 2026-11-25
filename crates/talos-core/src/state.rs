//! Per-conversation dialog state, owned by the engine (DESIGN.md section 7).
//!
//! Each conversation has its own lock, so turns of one conversation are
//! serialised and turns of different conversations run independently. The
//! map of conversations has a lock of its own, held only to find or create
//! an entry. Memory is bounded: at most a configured number of
//! conversations, and a conversation idle for longer than the configured
//! expiry (measured with the turns' injected `now`, never the system clock)
//! starts afresh. When the cap is reached the least recently seen
//! conversation goes; between two seen in the same second, the one used
//! less recently.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::language::Hold;
use crate::skill::{Expectation, Message};
use crate::value::Slots;
use crate::{ConvId, IntentId, Lang, Status, Timestamp};

/// What the engine remembers about one conversation between turns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationState {
    turns: u32,
    lang: Option<(Lang, Hold)>,
    last_intent: Option<IntentId>,
    /// The open question waiting for the next message, if any
    /// (docs/spec/conversation.md section 3). At most one: asking again
    /// replaces it.
    open: Option<Open>,
    /// What was said lately, for the reflector's variants.
    recent: Recent,
    /// What the dictionary skills remember (docs/spec/knowledge.md 2.2 and
    /// 2.8); never written to disk.
    memory: Memory,
}

/// The senses chosen, the concepts talked about and the topic of a
/// conversation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Memory {
    /// Word key → the concept chosen for it.
    senses: BTreeMap<String, String>,
    /// (turn, concept), oldest first; only the last few turns are kept.
    mentions: Vec<(u32, String)>,
    /// The word of the last dictionary answer.
    topic: Option<String>,
}

/// How many turns back the recent context reaches (docs/spec/knowledge.md
/// 2.2, step 3).
const RECENT_TURNS: u32 = 3;

/// How many chosen senses a conversation keeps; past it, the first in key
/// order makes room (a bound on memory, not a policy anyone relies on).
const MAX_SENSES: usize = 64;

impl Memory {
    const NEW: Memory = Memory {
        senses: BTreeMap::new(),
        mentions: Vec::new(),
        topic: None,
    };
}

/// An open question: what was asked, by whom, and what the request had so
/// far.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Open {
    /// The intent being completed.
    pub(crate) intent: IntentId,
    /// What answers it.
    pub(crate) expectation: Expectation,
    /// The request's slots when it asked (internal ones included), without
    /// the open one.
    pub(crate) slots: Slots,
    /// The question, to ask again in the language of a later turn.
    pub(crate) question: Message,
    /// The turn that opened it.
    pub(crate) turn: u32,
    /// When it was opened, in Unix seconds of the turn's `now`.
    pub(crate) at: i64,
    /// How many times the question has been put: 1, or 2 after a failed
    /// answer. It is never put a third time.
    pub(crate) asked: u8,
    /// Whether the request was the classifier's guess: its answer is
    /// handled as a guess too (the handler still owes a confirmation).
    pub(crate) guessed: bool,
}

/// What Talos said lately in a conversation: enough for the reflector to
/// avoid repeating itself (docs/spec/reflector.md 6).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Recent {
    /// The previous turn's reply text, whatever answered it.
    pub(crate) last_reply: Option<String>,
    /// Consecutive deflections that ended in a question, up to the last
    /// turn.
    pub(crate) questions_in_row: u8,
    /// The variant each reflector rule (by language and position) used
    /// last.
    variants: BTreeMap<(Lang, u16), usize>,
}

impl Recent {
    const NEW: Recent = Recent {
        last_reply: None,
        questions_in_row: 0,
        variants: BTreeMap::new(),
    };

    /// The variant a rule used last, if it has been used.
    pub(crate) fn variant(&self, lang: Lang, rule: u16) -> Option<usize> {
        self.variants.get(&(lang, rule)).copied()
    }

    pub(crate) fn set_variant(&mut self, lang: Lang, rule: u16, variant: usize) {
        self.variants.insert((lang, rule), variant);
    }
}

impl ConversationState {
    /// A conversation that has not started.
    pub const NEW: ConversationState = ConversationState {
        turns: 0,
        lang: None,
        last_intent: None,
        open: None,
        recent: Recent::NEW,
        memory: Memory::NEW,
    };

    /// The concept chosen for a word (its key) earlier in this
    /// conversation, by the user or by context.
    #[must_use]
    pub fn sense(&self, word: &str) -> Option<&str> {
        self.memory.senses.get(word).map(String::as_str)
    }

    /// The concepts talked about in the previous three turns, most recent
    /// first, without repeats.
    #[must_use]
    pub fn recent_concepts(&self) -> Vec<&str> {
        let from = self.turns.saturating_sub(RECENT_TURNS);
        let mut out: Vec<&str> = Vec::new();
        for (turn, concept) in self.memory.mentions.iter().rev() {
            if *turn >= from && *turn < self.turns && !out.contains(&concept.as_str()) {
                out.push(concept);
            }
        }
        out
    }

    /// The word the last dictionary answer was about: what a short
    /// follow-up refers to.
    #[must_use]
    pub fn topic(&self) -> Option<&str> {
        self.memory.topic.as_deref()
    }

    /// Applies what a handler asked to remember about this turn.
    pub(crate) fn apply_memos(&mut self, memos: Vec<crate::skill::Memo>) {
        use crate::skill::Memo;
        let turn = self.turns;
        for memo in memos {
            match memo {
                Memo::Sense(word, concept) => {
                    if self.memory.senses.len() >= MAX_SENSES
                        && !self.memory.senses.contains_key(&word)
                        && let Some(first) = self.memory.senses.keys().next().cloned()
                    {
                        self.memory.senses.remove(&first);
                    }
                    self.memory.senses.insert(word, concept);
                }
                Memo::Mention(concept) => self.memory.mentions.push((turn, concept)),
                Memo::Topic(word) => self.memory.topic = Some(word),
            }
        }
        let from = turn.saturating_sub(RECENT_TURNS);
        self.memory.mentions.retain(|(t, _)| *t >= from);
    }

    /// The number of the turn being answered, from 1; 0 before the first.
    #[must_use]
    pub const fn turn(&self) -> u32 {
        self.turns
    }

    /// The conversation's language so far, if one has been decided.
    #[must_use]
    pub fn lang(&self) -> Option<Lang> {
        self.lang.map(|(lang, _)| lang)
    }

    /// The language and how firmly it is held.
    #[must_use]
    pub const fn lang_hold(&self) -> Option<(Lang, Hold)> {
        self.lang
    }

    /// The recent topic: the last intent recognised in this conversation,
    /// whether or not it was answered.
    #[must_use]
    pub const fn last_intent(&self) -> Option<&IntentId> {
        self.last_intent.as_ref()
    }

    /// The open question, if any.
    #[must_use]
    pub fn expectation(&self) -> Option<&Expectation> {
        self.open.as_ref().map(|open| &open.expectation)
    }

    /// Takes the open question out, for the turn to answer, keep or drop.
    pub(crate) fn take_open(&mut self) -> Option<Open> {
        self.open.take()
    }

    /// Leaves a question open for the next turn (or none).
    pub(crate) fn set_open(&mut self, open: Option<Open>) {
        self.open = open;
    }

    /// Starts the next turn and returns its number.
    pub(crate) fn begin_turn(&mut self) -> u32 {
        self.turns = self.turns.saturating_add(1);
        self.turns
    }

    pub(crate) fn set_lang(&mut self, lang: Lang, hold: Hold) {
        self.lang = Some((lang, hold));
    }

    /// The previous turn's reply, if there was a turn.
    #[must_use]
    pub fn last_reply(&self) -> Option<&str> {
        self.recent.last_reply.as_deref()
    }

    pub(crate) const fn recent(&self) -> &Recent {
        &self.recent
    }

    pub(crate) fn recent_mut(&mut self) -> &mut Recent {
        &mut self.recent
    }

    /// Records the turn's reply: the next turn will not repeat it, and a
    /// run of deflections ending in a question is counted.
    pub(crate) fn remember_reply(&mut self, reply: &str, status: Status) {
        self.recent.questions_in_row = if status == Status::Deflected && reply.ends_with('?') {
            self.recent.questions_in_row.saturating_add(1)
        } else {
            0
        };
        self.recent.last_reply = Some(reply.to_owned());
    }

    pub(crate) fn set_last_intent(&mut self, intent: Option<IntentId>) {
        if intent.is_some() {
            self.last_intent = intent;
        }
    }
}

impl Default for ConversationState {
    fn default() -> Self {
        Self::NEW
    }
}

/// One conversation in the map.
#[derive(Debug)]
struct Entry {
    state: Arc<Mutex<ConversationState>>,
    /// The `now` of its latest turn, in Unix seconds.
    last_seen: i64,
    /// When it was last entered, counted in entries to the map: the order
    /// of use, which breaks ties between turns in the same second.
    used: u64,
}

/// The map and its use counter, under one lock.
#[derive(Debug, Default)]
struct Map {
    entries: HashMap<ConvId, Entry>,
    uses: u64,
}

/// Every live conversation.
#[derive(Debug)]
pub(crate) struct Conversations {
    map: Mutex<Map>,
    max: usize,
    idle_seconds: i64,
}

/// Recovers a lock's data after a panic elsewhere: the state is plain data,
/// valid after any partial turn.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Conversations {
    pub(crate) fn new(max: usize, idle_seconds: u32) -> Self {
        Self {
            map: Mutex::new(Map::default()),
            max: max.max(1),
            idle_seconds: i64::from(idle_seconds),
        }
    }

    /// The state of a conversation at `now`, created (or restarted, when it
    /// has been idle too long) if needed. Hold the returned lock for the
    /// whole turn.
    pub(crate) fn enter(&self, id: &ConvId, now: Timestamp) -> Arc<Mutex<ConversationState>> {
        let now = now.unix_seconds;
        let mut guard = lock(&self.map);
        let Map { entries, uses } = &mut *guard;
        *uses += 1;
        let used = *uses;
        if let Some(entry) = entries.get_mut(id) {
            if now.saturating_sub(entry.last_seen) > self.idle_seconds {
                entry.state = Arc::default();
            }
            entry.last_seen = entry.last_seen.max(now);
            entry.used = used;
            return Arc::clone(&entry.state);
        }
        if entries.len() >= self.max {
            let idle = self.idle_seconds;
            entries.retain(|_, e| now.saturating_sub(e.last_seen) <= idle);
        }
        while entries.len() >= self.max {
            // The least recently seen goes; in the same second, the least
            // recently used. Deterministic: the order of use is the order
            // of the turns.
            let Some(oldest) = entries
                .iter()
                .min_by_key(|(_, e)| (e.last_seen, e.used))
                .map(|(id, _)| id.clone())
            else {
                break;
            };
            entries.remove(&oldest);
        }
        let state = Arc::<Mutex<ConversationState>>::default();
        entries.insert(
            id.clone(),
            Entry {
                state: Arc::clone(&state),
                last_seen: now,
                used,
            },
        );
        state
    }

    /// Forgets a conversation; its next turn starts afresh.
    pub(crate) fn end(&self, id: &ConvId) {
        lock(&self.map).entries.remove(id);
    }

    /// How many conversations are held.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        lock(&self.map).entries.len()
    }
}

/// Locks one conversation's state for a turn.
pub(crate) fn lock_state(state: &Mutex<ConversationState>) -> MutexGuard<'_, ConversationState> {
    lock(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_unix(seconds)
    }

    fn turn(store: &Conversations, id: &str, now: i64) -> u32 {
        let state = store.enter(&ConvId::new(id), at(now));
        lock_state(&state).begin_turn()
    }

    #[test]
    fn counts_turns_per_conversation() {
        let store = Conversations::new(10, 60);
        assert_eq!(turn(&store, "a", 0), 1);
        assert_eq!(turn(&store, "a", 1), 2);
        assert_eq!(turn(&store, "b", 1), 1);
        assert_eq!(turn(&store, "a", 2), 3);
        store.end(&ConvId::new("a"));
        assert_eq!(turn(&store, "a", 3), 1);
    }

    #[test]
    fn an_idle_conversation_starts_afresh() {
        let store = Conversations::new(10, 60);
        assert_eq!(turn(&store, "a", 0), 1);
        assert_eq!(turn(&store, "a", 60), 2);
        assert_eq!(turn(&store, "a", 121), 1);
        // A clock that goes backwards does not expire anything.
        assert_eq!(turn(&store, "a", 100), 2);
    }

    #[test]
    fn the_number_of_conversations_is_bounded() {
        let store = Conversations::new(2, 1_000);
        turn(&store, "a", 0);
        turn(&store, "b", 1);
        turn(&store, "a", 2);
        turn(&store, "c", 3);
        assert_eq!(store.len(), 2);
        // "b" was the least recently seen.
        assert_eq!(turn(&store, "a", 4), 3);
        assert_eq!(turn(&store, "b", 5), 1);
        // Expired conversations go first.
        let store = Conversations::new(2, 10);
        turn(&store, "a", 0);
        turn(&store, "b", 100);
        turn(&store, "c", 101);
        assert_eq!(turn(&store, "b", 102), 2);
        // In the same second, the least recently used goes, whatever its id.
        let store = Conversations::new(2, 1_000);
        turn(&store, "b", 0);
        turn(&store, "a", 0);
        turn(&store, "b", 0);
        turn(&store, "c", 0);
        assert_eq!(turn(&store, "b", 0), 3);
        assert_eq!(turn(&store, "a", 0), 1);
    }
}
