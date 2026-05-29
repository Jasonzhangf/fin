use serde::{Deserialize, Serialize};

/// Append-only message log with structural enforcement.
///
/// Invariants (modeled after Deepseek-reasonix AppendOnlyLog):
/// - `append()` is the only way to add entries
/// - `compact()` is the only legal mutation path, sets `compacted = true`
/// - After compact, further `append()` calls are still valid
/// - In debug builds, assertion checks monotonic growth of entries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppendOnlyMessageLog {
    entries: Vec<String>,
    compacted: bool,
    #[serde(default)]
    append_count: usize,
    #[serde(default)]
    compact_count: usize,
}

impl Default for AppendOnlyMessageLog {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            compacted: false,
            append_count: 0,
            compact_count: 0,
        }
    }
}

impl AppendOnlyMessageLog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn append(&mut self, message: String) {
        assert!(!message.is_empty(), "cannot append empty message");
        self.entries.push(message);
        self.append_count += 1;
    }

    pub fn extend(&mut self, messages: impl IntoIterator<Item = String>) {
        for msg in messages {
            self.append(msg);
        }
    }

    /// The only legal mutation path. Replaces all entries with a compacted summary.
    /// Sets `compacted = true`; subsequent `append()` calls are still valid.
    pub fn compact(&mut self, replacement: Vec<String>) {
        let old_len = self.entries.len();
        self.entries = replacement;
        self.compacted = true;
        self.compact_count += 1;
        debug_assert!(
            self.entries.len() <= old_len,
            "compact must not increase entry count: {old_len} -> {}",
            self.entries.len()
        );
    }

    pub fn entries(&self) -> &[String] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_compacted(&self) -> bool {
        self.compacted
    }

    pub fn append_count(&self) -> usize {
        self.append_count
    }

    pub fn compact_count(&self) -> usize {
        self.compact_count
    }
}
