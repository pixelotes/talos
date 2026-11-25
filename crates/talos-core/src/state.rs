//! Per-conversation dialog state, owned by the engine (DESIGN.md section 7).
//!
//! Each conversation has its own lock, so turns of one conversation are
//! serialised and turns of different conversations run independently. The
//! map of conversations has a lock of its own, held only to find or create
//! an entry. Memory is bounded: at most a configured number of
//! conversations, and a conversation idle for longer than the configured
//! expiry (measured with the turns' injected `now`, never the system clock)
//! starts afresh.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::language::Hold;
use crate::skill::Expectation;
use crate::{ConvId, IntentId, Lang, Timestamp};

/// What the engine remembers about one conversation between turns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationState {
    turns: u32,
    lang: Option<(Lang, Hold)>,
    last_intent: Option<IntentId>,
    /// An open question waiting for the next message. Always `None` in
    /// v0.1; open questions arrive in v0.2 (docs/spec/conversation.md
    /// section 3).
    expectation: Option<Expectation>,
}

impl ConversationState {
    /// A conversation that has not started.
    pub const NEW: ConversationState = ConversationState {
        turns: 0,
        lang: None,
        last_intent: None,
        expectation: None,
    };

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

    /// The open question, if any (never, in v0.1).
    #[must_use]
    pub const fn expectation(&self) -> Option<&Expectation> {
        self.expectation.as_ref()
    }

    /// Starts the next turn and returns its number.
    pub(crate) fn begin_turn(&mut self) -> u32 {
        self.turns = self.turns.saturating_add(1);
        self.turns
    }

    pub(crate) fn set_lang(&mut self, lang: Lang, hold: Hold) {
        self.lang = Some((lang, hold));
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
}

/// Every live conversation.
#[derive(Debug)]
pub(crate) struct Conversations {
    map: Mutex<HashMap<ConvId, Entry>>,
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
            map: Mutex::new(HashMap::new()),
            max: max.max(1),
            idle_seconds: i64::from(idle_seconds),
        }
    }

    /// The state of a conversation at `now`, created (or restarted, when it
    /// has been idle too long) if needed. Hold the returned lock for the
    /// whole turn.
    pub(crate) fn enter(&self, id: &ConvId, now: Timestamp) -> Arc<Mutex<ConversationState>> {
        let now = now.unix_seconds;
        let mut map = lock(&self.map);
        if let Some(entry) = map.get_mut(id) {
            if now.saturating_sub(entry.last_seen) > self.idle_seconds {
                entry.state = Arc::default();
            }
            entry.last_seen = entry.last_seen.max(now);
            return Arc::clone(&entry.state);
        }
        if map.len() >= self.max {
            let idle = self.idle_seconds;
            map.retain(|_, e| now.saturating_sub(e.last_seen) <= idle);
        }
        while map.len() >= self.max {
            // The least recently seen goes; ties by id, for determinism.
            let Some(oldest) = map
                .iter()
                .min_by(|(a_id, a), (b_id, b)| a.last_seen.cmp(&b.last_seen).then(a_id.cmp(b_id)))
                .map(|(id, _)| id.clone())
            else {
                break;
            };
            map.remove(&oldest);
        }
        let state = Arc::<Mutex<ConversationState>>::default();
        map.insert(
            id.clone(),
            Entry {
                state: Arc::clone(&state),
                last_seen: now,
            },
        );
        state
    }

    /// Forgets a conversation; its next turn starts afresh.
    pub(crate) fn end(&self, id: &ConvId) {
        lock(&self.map).remove(id);
    }

    /// How many conversations are held.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        lock(&self.map).len()
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
    }
}
