//! Conversation identity (docs/API.md section 2.3): an explicit key, the
//! `user` field, or a fingerprint of the transcript. The dialog state itself
//! is the engine's, bounded there; this module only maps a request to the
//! engine's conversation id, and for fingerprints keeps the bounded table
//! from "the transcript so far" to the conversation it belongs to.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use talos_core::{ConvId, Timestamp};

/// Keys and `user` values longer than this are replaced by their hash, so
/// that a conversation id is never larger than a few hundred bytes.
const MAX_ID_BYTES: usize = 128;

/// FNV-1a, 128 bits: stable across releases and platforms (`std`'s hasher
/// is not), and wide enough that two transcripts never meet by chance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Fingerprint(u128);

impl Fingerprint {
    const OFFSET: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
    const PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;

    /// The fingerprint of an empty transcript.
    pub(crate) const EMPTY: Fingerprint = Fingerprint(Self::OFFSET);

    fn bytes(self, bytes: &[u8]) -> Self {
        let mut hash = self.0;
        for &b in bytes {
            hash ^= u128::from(b);
            hash = hash.wrapping_mul(Self::PRIME);
        }
        Self(hash)
    }

    /// The fingerprint of the transcript with one more message. Only user
    /// and assistant messages count; their text is trimmed, so that a
    /// client that strips a trailing newline still matches.
    pub(crate) fn then(self, speaker: Speaker, text: &str) -> Self {
        let text = text.trim();
        let tag = match speaker {
            Speaker::User => b'u',
            Speaker::Assistant => b'a',
        };
        self.bytes(&[tag])
            .bytes(&(text.len() as u64).to_le_bytes())
            .bytes(text.as_bytes())
    }

    fn hex(self) -> String {
        format!("{:032x}", self.0)
    }
}

/// Who said a message that counts towards the fingerprint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Speaker {
    User,
    Assistant,
}

/// How a request names its conversation, in order of preference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Identity {
    /// `metadata.conversation` or the `X-Talos-Conversation` header.
    Key(String),
    /// The `user` field of the `OpenAI` request.
    User(String),
    /// The fingerprint of every user and assistant message before the last
    /// user message.
    Transcript(Fingerprint),
}

/// A caller-given id, bounded in size.
fn bounded(prefix: &str, id: &str) -> ConvId {
    if id.len() <= MAX_ID_BYTES {
        ConvId::new(format!("{prefix}:{id}"))
    } else {
        let hash = Fingerprint::EMPTY.bytes(id.as_bytes()).hex();
        ConvId::new(format!("{prefix}#{hash}"))
    }
}

/// One transcript the table knows.
#[derive(Debug)]
struct Alias {
    /// The conversation number: the engine's id is `transcript:N`.
    conversation: u64,
    /// The `now` of its latest use, in Unix seconds.
    last_seen: i64,
    /// The order of its latest use.
    used: u64,
}

#[derive(Debug, Default)]
struct Table {
    aliases: HashMap<Fingerprint, Alias>,
    uses: u64,
    conversations: u64,
}

/// Transcript fingerprints to conversations: at most `max` entries; one
/// idle for longer than `idle_seconds` is forgotten; when full, the least
/// recently used goes. The same bounds as the engine's dialog state.
#[derive(Debug)]
pub(crate) struct Transcripts {
    table: Mutex<Table>,
    max: usize,
    idle_seconds: i64,
}

impl Transcripts {
    pub(crate) fn new(max: usize, idle_seconds: u32) -> Self {
        Self {
            table: Mutex::new(Table::default()),
            max: max.max(1),
            idle_seconds: i64::from(idle_seconds),
        }
    }

    /// The engine's conversation for an identity. For a transcript not
    /// known (the first turn, a history edited by the client, a restart,
    /// an eviction), a new conversation.
    pub(crate) fn resolve(&self, identity: &Identity, now: Timestamp) -> ConvId {
        let prefix = match identity {
            Identity::Key(key) => return bounded("conversation", key),
            Identity::User(user) => return bounded("user", user),
            Identity::Transcript(prefix) => *prefix,
        };
        let now = now.unix_seconds;
        let mut table = self.table.lock().unwrap_or_else(PoisonError::into_inner);
        table.uses += 1;
        let used = table.uses;
        let idle = self.idle_seconds;
        let known = if prefix == Fingerprint::EMPTY {
            None
        } else {
            match table.aliases.get_mut(&prefix) {
                Some(alias) if now.saturating_sub(alias.last_seen) <= idle => {
                    alias.last_seen = alias.last_seen.max(now);
                    alias.used = used;
                    Some(alias.conversation)
                }
                _ => None,
            }
        };
        let n = known.unwrap_or_else(|| {
            table.conversations += 1;
            table.conversations
        });
        transcript_id(n)
    }

    /// Records that the transcript `next` (the request's, with the reply
    /// appended) belongs to `conversation`, so that the client's next
    /// request finds it.
    pub(crate) fn remember(&self, next: Fingerprint, conversation: &ConvId, now: Timestamp) {
        let Some(n) = conversation
            .as_str()
            .strip_prefix("transcript:")
            .and_then(|n| n.parse().ok())
        else {
            return;
        };
        let now = now.unix_seconds;
        let mut guard = self.table.lock().unwrap_or_else(PoisonError::into_inner);
        let table = &mut *guard;
        table.uses += 1;
        let used = table.uses;
        if !table.aliases.contains_key(&next) && table.aliases.len() >= self.max {
            let idle = self.idle_seconds;
            table
                .aliases
                .retain(|_, a| now.saturating_sub(a.last_seen) <= idle);
            while table.aliases.len() >= self.max {
                let Some(oldest) = table
                    .aliases
                    .iter()
                    .min_by_key(|(_, a)| (a.last_seen, a.used))
                    .map(|(f, _)| *f)
                else {
                    break;
                };
                table.aliases.remove(&oldest);
            }
        }
        table.aliases.insert(
            next,
            Alias {
                conversation: n,
                last_seen: now,
                used,
            },
        );
    }

    /// How many transcripts are known.
    #[cfg(test)]
    fn len(&self) -> usize {
        self.table
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .aliases
            .len()
    }
}

fn transcript_id(n: u64) -> ConvId {
    ConvId::new(format!("transcript:{n}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_unix(seconds)
    }

    fn fp(messages: &[&str]) -> Fingerprint {
        messages
            .iter()
            .enumerate()
            .fold(Fingerprint::EMPTY, |f, (i, m)| {
                let speaker = if i % 2 == 0 {
                    Speaker::User
                } else {
                    Speaker::Assistant
                };
                f.then(speaker, m)
            })
    }

    #[test]
    fn fingerprints_tell_transcripts_apart() {
        assert_eq!(fp(&["hola", "¡Hola!"]), fp(&[" hola\n", "¡Hola! "]));
        assert_ne!(fp(&["hola", "¡Hola!"]), fp(&["hola", "¡Hola!", ""]));
        assert_ne!(fp(&["ab", "c"]), fp(&["a", "bc"]));
        assert_ne!(
            Fingerprint::EMPTY.then(Speaker::User, "x"),
            Fingerprint::EMPTY.then(Speaker::Assistant, "x")
        );
    }

    #[test]
    fn explicit_ids_are_namespaced_and_bounded() {
        let t = Transcripts::new(4, 60);
        assert_eq!(
            t.resolve(&Identity::Key("a".into()), at(0)),
            ConvId::new("conversation:a")
        );
        assert_eq!(
            t.resolve(&Identity::User("a".into()), at(0)),
            ConvId::new("user:a")
        );
        let long = t.resolve(&Identity::User("x".repeat(1_000)), at(0));
        assert!(long.as_str().starts_with("user#") && long.as_str().len() < 64);
    }

    #[test]
    fn a_transcript_continues_its_conversation() {
        let t = Transcripts::new(4, 60);
        let first = t.resolve(&Identity::Transcript(Fingerprint::EMPTY), at(0));
        assert_eq!(first, ConvId::new("transcript:1"));
        t.remember(fp(&["hola", "¡Hola!"]), &first, at(0));
        let again = t.resolve(&Identity::Transcript(fp(&["hola", "¡Hola!"])), at(10));
        assert_eq!(again, first);
        // An edited history is a new conversation.
        let other = t.resolve(&Identity::Transcript(fp(&["hola", "¡Hey!"])), at(10));
        assert_eq!(other, ConvId::new("transcript:2"));
        // So is an idle one.
        let late = t.resolve(&Identity::Transcript(fp(&["hola", "¡Hola!"])), at(71));
        assert_eq!(late, ConvId::new("transcript:3"));
    }

    #[test]
    fn the_table_is_bounded_least_recently_used_first() {
        let t = Transcripts::new(2, 1_000);
        let a = transcript_id(1);
        let b = transcript_id(2);
        let c = transcript_id(3);
        t.remember(fp(&["a"]), &a, at(0));
        t.remember(fp(&["b"]), &b, at(0));
        // Using "a" makes "b" the least recently used.
        assert_eq!(t.resolve(&Identity::Transcript(fp(&["a"])), at(0)), a);
        t.remember(fp(&["c"]), &c, at(0));
        assert_eq!(t.len(), 2);
        assert_eq!(t.resolve(&Identity::Transcript(fp(&["a"])), at(0)), a);
        assert_eq!(t.resolve(&Identity::Transcript(fp(&["c"])), at(0)), c);
        assert_ne!(t.resolve(&Identity::Transcript(fp(&["b"])), at(0)), b);
    }
}
