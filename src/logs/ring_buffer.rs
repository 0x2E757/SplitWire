use std::collections::VecDeque;
use std::fmt;
use std::sync::Arc;
use std::time::SystemTime;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Info,
    Warn,
    Error,
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(match self {
            Level::Info => "INFO",
            Level::Warn => "WARN",
            Level::Error => "ERROR",
        })
    }
}

/// One line of the log: what happened, where (`source`, such as `routes` or `dns`) and for which
/// profile (none for a global line).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogEntry {
    pub seq: u64,
    pub time: SystemTime,
    pub level: Level,
    pub profile: Option<Arc<str>>,
    pub source: &'static str,
    pub message: String,
}

impl LogEntry {
    /// Bytes charged against the buffer's bound: the texts plus the fixed fields.
    fn cost(&self) -> usize {
        self.message.len() + self.profile.as_ref().map_or(0, |p| p.len()) + 32
    }
}

/// Log entries bounded by their total size; the oldest are dropped first. Sequence numbers start at 1.
pub struct LogBuffer {
    entries: VecDeque<LogEntry>,
    bytes: usize,
    max_bytes: usize,
    next_seq: u64,
}

impl LogBuffer {
    pub fn new(max_bytes: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            bytes: 0,
            max_bytes,
            next_seq: 1,
        }
    }

    /// Stores `entry` under the next sequence number and returns that number.
    pub fn push(&mut self, mut entry: LogEntry) -> u64 {
        entry.seq = self.next_seq;
        self.next_seq += 1;
        self.bytes += entry.cost();
        self.entries.push_back(entry);
        while self.bytes > self.max_bytes {
            match self.entries.pop_front() {
                Some(old) => self.bytes -= old.cost(),
                None => break,
            }
        }
        self.next_seq - 1
    }

    /// Up to `limit` entries with a sequence number above `after`, oldest first.
    pub fn since(&self, after: u64, limit: usize) -> Vec<LogEntry> {
        let first_seq = self.entries.front().map_or(self.next_seq, |e| e.seq);
        let skip = after.saturating_sub(first_seq - 1) as usize;
        self.entries
            .iter()
            .skip(skip)
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn last_seq(&self) -> u64 {
        self.next_seq - 1
    }

    pub fn bytes(&self) -> usize {
        self.bytes
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;

    fn push(buffer: &mut LogBuffer, message: &str) -> u64 {
        buffer.push(LogEntry {
            seq: 0,
            time: UNIX_EPOCH,
            level: Level::Info,
            profile: None,
            source: "test",
            message: message.to_owned(),
        })
    }

    #[test]
    fn numbers_entries_from_one() {
        let mut buffer = LogBuffer::new(1_000);
        assert_eq!(push(&mut buffer, "a"), 1);
        assert_eq!(push(&mut buffer, "b"), 2);
        assert_eq!(buffer.last_seq(), 2);
        assert_eq!(buffer.since(1, 10)[0].seq, 2);
    }

    #[test]
    fn stays_within_its_byte_bound_dropping_the_oldest() {
        let mut buffer = LogBuffer::new(400);
        for i in 0..100 {
            push(&mut buffer, &format!("message {i:02} {}", "x".repeat(40)));
        }
        assert!(buffer.bytes() <= 400);
        assert_eq!(buffer.len(), 400 / (51 + 32));
        let kept = buffer.since(0, usize::MAX);
        assert_eq!(kept.last().unwrap().seq, 100);
        assert_eq!(kept.first().unwrap().seq, 100 - kept.len() as u64 + 1);
    }

    #[test]
    fn the_profile_tag_counts_against_the_bound() {
        let mut buffer = LogBuffer::new(1_000);
        buffer.push(LogEntry {
            seq: 0,
            time: UNIX_EPOCH,
            level: Level::Warn,
            profile: Some("AcmeCorp".into()),
            source: "routes",
            message: "m".into(),
        });
        assert_eq!(buffer.bytes(), 1 + 8 + 32);
    }

    #[test]
    fn reads_incrementally_by_sequence() {
        let mut buffer = LogBuffer::new(10_000);
        for i in 0..10 {
            push(&mut buffer, &i.to_string());
        }
        let first = buffer.since(0, 4);
        assert_eq!(
            first.iter().map(|e| e.seq).collect::<Vec<_>>(),
            [1, 2, 3, 4]
        );
        let next = buffer.since(first.last().unwrap().seq, 100);
        assert_eq!(next.first().unwrap().seq, 5);
        assert_eq!(next.len(), 6);
        assert!(buffer.since(10, 100).is_empty());
        assert!(buffer.since(500, 100).is_empty());
    }

    #[test]
    fn a_reader_behind_the_evicted_entries_gets_the_oldest_kept() {
        let mut buffer = LogBuffer::new(3 * 33);
        for i in 0..10 {
            push(&mut buffer, &i.to_string());
        }
        let entries = buffer.since(2, 100);
        assert_eq!(
            entries.iter().map(|e| e.seq).collect::<Vec<_>>(),
            [8, 9, 10]
        );
    }
}
