//! Bounded memory/disk cache with deterministic keys and corruption recovery.

#![forbid(unsafe_code)]

use std::{
    collections::HashMap,
    fmt, fs, io,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
    time::SystemTime,
};

const MAGIC: &[u8; 8] = b"XVCACHE\0";
const SCHEMA_VERSION: u32 = 1;
const HEADER_BYTES: usize = 8 + 4 + 4 + 8 + 32 + 32;

/// Exact 256-bit identity used for memory and disk lookup.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CacheKey([u8; 32]);

impl CacheKey {
    /// Creates a key from an already-computed digest.
    #[must_use]
    pub const fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    /// Hashes a namespace and ordered byte slices into one stable key.
    #[must_use]
    pub fn from_parts(namespace: &[u8], parts: &[&[u8]]) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"XVARNA_CACHE_KEY_V1\0");
        hasher.update(&(namespace.len() as u64).to_le_bytes());
        hasher.update(namespace);
        for part in parts {
            hasher.update(&(part.len() as u64).to_le_bytes());
            hasher.update(part);
        }
        Self(*hasher.finalize().as_bytes())
    }

    /// Returns the raw key bytes.
    #[must_use]
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }

    /// Lowercase filename-safe hexadecimal representation.
    #[must_use]
    pub fn to_hex(self) -> String {
        let mut output = String::with_capacity(64);
        for byte in self.0 {
            use fmt::Write as _;
            let _ = write!(output, "{byte:02x}");
        }
        output
    }
}

/// Runtime cache policy. A zero budget disables that tier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheConfig {
    /// Directory containing versioned `.xvc` entries.
    pub directory: PathBuf,
    /// Maximum payload bytes retained in process memory.
    pub memory_budget_bytes: u64,
    /// Maximum total bytes retained on disk.
    pub disk_budget_bytes: u64,
}

impl CacheConfig {
    /// Validates and constructs one policy.
    pub fn try_new(
        directory: PathBuf,
        memory_budget_bytes: u64,
        disk_budget_bytes: u64,
    ) -> Result<Self, CacheError> {
        if directory.as_os_str().is_empty() {
            return Err(CacheError::InvalidConfiguration(
                "cache directory cannot be empty".to_owned(),
            ));
        }
        Ok(Self {
            directory,
            memory_budget_bytes,
            disk_budget_bytes,
        })
    }
}

/// Observable cumulative cache state.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CacheStats {
    /// Requests served without filesystem access.
    pub memory_hit_count: u64,
    /// Requests restored from a validated disk entry.
    pub disk_hit_count: u64,
    /// Requests for which no valid entry existed.
    pub miss_count: u64,
    /// Entries written successfully to at least one tier.
    pub write_count: u64,
    /// Entries removed to enforce a memory or disk budget.
    pub eviction_count: u64,
    /// Invalid entries removed during recovery.
    pub corruption_count: u64,
    /// Current in-process payload bytes.
    pub memory_bytes: u64,
    /// Current validated/known disk bytes.
    pub disk_bytes: u64,
    /// Current in-process entry count.
    pub memory_entry_count: u64,
    /// Current disk entry count.
    pub disk_entry_count: u64,
    /// Latest concise operation or recovery diagnostic.
    pub last_event: String,
}

/// Origin of a successful cache lookup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheHitTier {
    /// The payload was cloned from the bounded in-process cache.
    Memory,
    /// The payload was checksummed and read from disk.
    Disk,
}

/// Validated payload and its lookup tier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheHit {
    /// Exact cached bytes.
    pub payload: Vec<u8>,
    /// Tier that served the lookup.
    pub tier: CacheHitTier,
}

/// Cache configuration or filesystem failure.
#[derive(Debug)]
pub enum CacheError {
    /// The requested policy is invalid.
    InvalidConfiguration(String),
    /// A filesystem operation failed.
    Io(io::Error),
    /// The cache state mutex was poisoned.
    StatePoisoned,
    /// Payload length cannot be represented by the cache schema.
    PayloadTooLarge,
}

impl fmt::Display for CacheError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration(message) => formatter.write_str(message),
            Self::Io(error) => write!(formatter, "cache I/O failed: {error}"),
            Self::StatePoisoned => formatter.write_str("cache state lock was poisoned"),
            Self::PayloadTooLarge => formatter.write_str("cache payload length exceeds u64"),
        }
    }
}

impl std::error::Error for CacheError {}

impl From<io::Error> for CacheError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Clone)]
struct MemoryEntry {
    payload: Vec<u8>,
    last_access: u64,
}

#[derive(Default)]
struct State {
    entries: HashMap<CacheKey, MemoryEntry>,
    memory_bytes: u64,
    access_clock: u64,
    stats: CacheStats,
}

/// Thread-safe cache store with independent memory and disk budgets.
pub struct CacheStore {
    config: CacheConfig,
    state: Mutex<State>,
}

impl CacheStore {
    /// Opens a cache and performs an initial disk inventory.
    pub fn open(config: CacheConfig) -> Result<Self, CacheError> {
        if config.disk_budget_bytes > 0 {
            fs::create_dir_all(&config.directory)?;
        }
        let (disk_bytes, disk_entries) = inventory(&config.directory)?;
        let mut state = State::default();
        state.stats.disk_bytes = disk_bytes;
        state.stats.disk_entry_count = disk_entries;
        "cache opened".clone_into(&mut state.stats.last_event);
        let store = Self {
            config,
            state: Mutex::new(state),
        };
        store.evict_disk_to_budget()?;
        Ok(store)
    }

    /// Returns the active immutable policy.
    #[must_use]
    pub const fn config(&self) -> &CacheConfig {
        &self.config
    }

    /// Looks up and validates one payload. Invalid disk entries are removed and counted as misses.
    pub fn get(&self, key: CacheKey) -> Result<Option<CacheHit>, CacheError> {
        {
            let mut state = self.lock_state()?;
            state.access_clock = state.access_clock.saturating_add(1);
            let access = state.access_clock;
            if let Some(entry) = state.entries.get_mut(&key) {
                entry.last_access = access;
                let payload = entry.payload.clone();
                state.stats.memory_hit_count = state.stats.memory_hit_count.saturating_add(1);
                state.stats.last_event = format!("memory hit {}", key.to_hex());
                return Ok(Some(CacheHit {
                    payload,
                    tier: CacheHitTier::Memory,
                }));
            }
        }

        if self.config.disk_budget_bytes == 0 {
            self.record_miss(key, "disk tier disabled")?;
            return Ok(None);
        }
        let path = self.entry_path(key);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.record_miss(key, "entry absent")?;
                return Ok(None);
            }
            Err(error) => return Err(error.into()),
        };
        let payload = if let Some(payload) = decode_entry(key, &bytes) {
            payload.to_vec()
        } else {
            let _ = fs::remove_file(&path);
            let mut state = self.lock_state()?;
            state.stats.corruption_count = state.stats.corruption_count.saturating_add(1);
            state.stats.miss_count = state.stats.miss_count.saturating_add(1);
            state.stats.disk_bytes = state
                .stats
                .disk_bytes
                .saturating_sub(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
            state.stats.disk_entry_count = state.stats.disk_entry_count.saturating_sub(1);
            state.stats.last_event = format!("recovered corrupt entry {}", key.to_hex());
            return Ok(None);
        };
        {
            let mut state = self.lock_state()?;
            state.stats.disk_hit_count = state.stats.disk_hit_count.saturating_add(1);
            state.stats.last_event = format!("disk hit {}", key.to_hex());
            insert_memory(&self.config, &mut state, key, payload.clone());
        }
        Ok(Some(CacheHit {
            payload,
            tier: CacheHitTier::Disk,
        }))
    }

    /// Inserts one payload, using an atomic temporary-file rename for the disk tier.
    pub fn put(&self, key: CacheKey, payload: Vec<u8>) -> Result<(), CacheError> {
        let encoded = encode_entry(key, &payload)?;
        let mut wrote = if self.config.disk_budget_bytes > 0 {
            fs::create_dir_all(&self.config.directory)?;
            let destination = self.entry_path(key);
            let temporary =
                self.config
                    .directory
                    .join(format!("{}.{}.tmp", key.to_hex(), std::process::id()));
            fs::write(&temporary, &encoded)?;
            if destination.exists() {
                fs::remove_file(&destination)?;
            }
            fs::rename(&temporary, &destination)?;
            true
        } else {
            false
        };
        {
            let mut state = self.lock_state()?;
            if self.config.memory_budget_bytes > 0 {
                insert_memory(&self.config, &mut state, key, payload);
                wrote = true;
            }
            if wrote {
                state.stats.write_count = state.stats.write_count.saturating_add(1);
                state.stats.last_event = format!("stored {}", key.to_hex());
            }
        }
        self.refresh_disk_inventory()?;
        self.evict_disk_to_budget()?;
        Ok(())
    }

    /// Removes a semantically invalid entry from both tiers and records recovery.
    pub fn recover_corrupt(&self, key: CacheKey, detail: &str) -> Result<(), CacheError> {
        let path = self.entry_path(key);
        let _ = fs::remove_file(path);
        let mut state = self.lock_state()?;
        if let Some(entry) = state.entries.remove(&key) {
            state.memory_bytes = state
                .memory_bytes
                .saturating_sub(u64::try_from(entry.payload.len()).unwrap_or(u64::MAX));
        }
        state.stats.corruption_count = state.stats.corruption_count.saturating_add(1);
        state.stats.last_event = format!("recovered {}: {detail}", key.to_hex());
        state.stats.memory_bytes = state.memory_bytes;
        state.stats.memory_entry_count = u64::try_from(state.entries.len()).unwrap_or(u64::MAX);
        drop(state);
        self.refresh_disk_inventory()
    }

    /// Clears only versioned XVARNA cache entries inside the configured directory.
    pub fn clear(&self) -> Result<(), CacheError> {
        if self.config.directory.exists() {
            for entry in fs::read_dir(&self.config.directory)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().is_some_and(|value| value == "xvc") {
                    fs::remove_file(path)?;
                }
            }
        }
        let mut state = self.lock_state()?;
        state.entries.clear();
        state.memory_bytes = 0;
        state.stats.memory_bytes = 0;
        state.stats.disk_bytes = 0;
        state.stats.memory_entry_count = 0;
        state.stats.disk_entry_count = 0;
        "cache cleared".clone_into(&mut state.stats.last_event);
        drop(state);
        Ok(())
    }

    /// Returns a consistent observable snapshot.
    pub fn stats(&self) -> Result<CacheStats, CacheError> {
        let state = self.lock_state()?;
        let mut snapshot = state.stats.clone();
        snapshot.memory_bytes = state.memory_bytes;
        snapshot.memory_entry_count = u64::try_from(state.entries.len()).unwrap_or(u64::MAX);
        drop(state);
        Ok(snapshot)
    }

    fn entry_path(&self, key: CacheKey) -> PathBuf {
        self.config.directory.join(format!("{}.xvc", key.to_hex()))
    }

    fn lock_state(&self) -> Result<MutexGuard<'_, State>, CacheError> {
        self.state.lock().map_err(|_| CacheError::StatePoisoned)
    }

    fn record_miss(&self, key: CacheKey, detail: &str) -> Result<(), CacheError> {
        let mut state = self.lock_state()?;
        state.stats.miss_count = state.stats.miss_count.saturating_add(1);
        state.stats.last_event = format!("miss {}: {detail}", key.to_hex());
        Ok(())
    }

    fn refresh_disk_inventory(&self) -> Result<(), CacheError> {
        let (bytes, entries) = inventory(&self.config.directory)?;
        let mut state = self.lock_state()?;
        state.stats.disk_bytes = bytes;
        state.stats.disk_entry_count = entries;
        drop(state);
        Ok(())
    }

    fn evict_disk_to_budget(&self) -> Result<(), CacheError> {
        if self.config.disk_budget_bytes == 0 || !self.config.directory.exists() {
            return Ok(());
        }
        let mut files = Vec::new();
        for entry in fs::read_dir(&self.config.directory)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|value| value == "xvc") {
                let metadata = entry.metadata()?;
                files.push((
                    path,
                    metadata.len(),
                    metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                ));
            }
        }
        files.sort_by_key(|(_, _, modified)| *modified);
        let mut total = files.iter().map(|(_, size, _)| *size).sum::<u64>();
        let mut evicted = 0_u64;
        for (path, size, _) in files {
            if total <= self.config.disk_budget_bytes {
                break;
            }
            fs::remove_file(path)?;
            total = total.saturating_sub(size);
            evicted = evicted.saturating_add(1);
        }
        self.refresh_disk_inventory()?;
        if evicted > 0 {
            let mut state = self.lock_state()?;
            state.stats.eviction_count = state.stats.eviction_count.saturating_add(evicted);
            state.stats.last_event = format!("evicted {evicted} disk entrie(s)");
        }
        Ok(())
    }
}

fn insert_memory(config: &CacheConfig, state: &mut State, key: CacheKey, payload: Vec<u8>) {
    let payload_bytes = u64::try_from(payload.len()).unwrap_or(u64::MAX);
    if config.memory_budget_bytes == 0 || payload_bytes > config.memory_budget_bytes {
        return;
    }
    if let Some(previous) = state.entries.remove(&key) {
        state.memory_bytes = state
            .memory_bytes
            .saturating_sub(u64::try_from(previous.payload.len()).unwrap_or(u64::MAX));
    }
    while state.memory_bytes.saturating_add(payload_bytes) > config.memory_budget_bytes {
        let Some(oldest) = state
            .entries
            .iter()
            .min_by_key(|(_, entry)| entry.last_access)
            .map(|(key, _)| *key)
        else {
            break;
        };
        if let Some(removed) = state.entries.remove(&oldest) {
            state.memory_bytes = state
                .memory_bytes
                .saturating_sub(u64::try_from(removed.payload.len()).unwrap_or(u64::MAX));
            state.stats.eviction_count = state.stats.eviction_count.saturating_add(1);
        }
    }
    state.access_clock = state.access_clock.saturating_add(1);
    state.entries.insert(
        key,
        MemoryEntry {
            payload,
            last_access: state.access_clock,
        },
    );
    state.memory_bytes = state.memory_bytes.saturating_add(payload_bytes);
    state.stats.memory_bytes = state.memory_bytes;
    state.stats.memory_entry_count = u64::try_from(state.entries.len()).unwrap_or(u64::MAX);
}

fn encode_entry(key: CacheKey, payload: &[u8]) -> Result<Vec<u8>, CacheError> {
    let payload_length = u64::try_from(payload.len()).map_err(|_| CacheError::PayloadTooLarge)?;
    let checksum = blake3::hash(payload);
    let mut output = Vec::with_capacity(HEADER_BYTES.saturating_add(payload.len()));
    output.extend_from_slice(MAGIC);
    output.extend_from_slice(&SCHEMA_VERSION.to_le_bytes());
    output.extend_from_slice(&0_u32.to_le_bytes());
    output.extend_from_slice(&payload_length.to_le_bytes());
    output.extend_from_slice(&key.bytes());
    output.extend_from_slice(checksum.as_bytes());
    output.extend_from_slice(payload);
    Ok(output)
}

fn decode_entry(key: CacheKey, bytes: &[u8]) -> Option<&[u8]> {
    if bytes.len() < HEADER_BYTES || &bytes[..8] != MAGIC {
        return None;
    }
    let schema = u32::from_le_bytes(bytes[8..12].try_into().ok()?);
    let flags = u32::from_le_bytes(bytes[12..16].try_into().ok()?);
    let payload_length =
        usize::try_from(u64::from_le_bytes(bytes[16..24].try_into().ok()?)).ok()?;
    if schema != SCHEMA_VERSION || flags != 0 || bytes[24..56] != key.bytes() {
        return None;
    }
    let expected = HEADER_BYTES.checked_add(payload_length)?;
    if bytes.len() != expected {
        return None;
    }
    let payload = &bytes[HEADER_BYTES..];
    let checksum = blake3::hash(payload);
    (bytes[56..88] == checksum.as_bytes()[..]).then_some(payload)
}

fn inventory(directory: &Path) -> Result<(u64, u64), CacheError> {
    if !directory.exists() {
        return Ok((0, 0));
    }
    let mut bytes = 0_u64;
    let mut entries = 0_u64;
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.path().extension().is_some_and(|value| value == "xvc") {
            bytes = bytes.saturating_add(entry.metadata()?.len());
            entries = entries.saturating_add(1);
        }
    }
    Ok((bytes, entries))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    fn directory(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "xvarna-cache-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_nanos()
        ))
    }

    #[test]
    fn memory_and_disk_hits_are_observable() {
        let path = directory("hit");
        let config = CacheConfig::try_new(path.clone(), 1024, 4096).expect("config valid");
        let store = CacheStore::open(config.clone()).expect("cache opens");
        let key = CacheKey::from_parts(b"test", &[b"one"]);
        store.put(key, b"payload".to_vec()).expect("put succeeds");
        assert_eq!(
            store.get(key).expect("get succeeds").expect("hit").tier,
            CacheHitTier::Memory
        );
        drop(store);
        let reopened = CacheStore::open(config).expect("cache reopens");
        let hit = reopened.get(key).expect("get succeeds").expect("disk hit");
        assert_eq!(hit.tier, CacheHitTier::Disk);
        assert_eq!(hit.payload, b"payload");
        let stats = reopened.stats().expect("stats available");
        assert_eq!(stats.disk_hit_count, 1);
        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn checksum_corruption_is_removed_and_counted() {
        let path = directory("corrupt");
        let store =
            CacheStore::open(CacheConfig::try_new(path.clone(), 0, 4096).expect("config valid"))
                .expect("cache opens");
        let key = CacheKey::from_parts(b"test", &[b"two"]);
        store.put(key, b"payload".to_vec()).expect("put succeeds");
        let file = path.join(format!("{}.xvc", key.to_hex()));
        let mut bytes = fs::read(&file).expect("entry readable");
        *bytes.last_mut().expect("payload exists") ^= 0xff;
        fs::write(&file, bytes).expect("corruption written");
        assert!(store.get(key).expect("recovery succeeds").is_none());
        assert!(!file.exists());
        assert_eq!(store.stats().expect("stats available").corruption_count, 1);
        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn memory_and_disk_lru_enforce_hard_budgets() {
        let path = directory("budget");
        let store =
            CacheStore::open(CacheConfig::try_new(path.clone(), 8, 210).expect("config valid"))
                .expect("cache opens");
        for index in 0_u8..5 {
            let key = CacheKey::from_parts(b"budget", &[&[index]]);
            store.put(key, vec![index; 8]).expect("put succeeds");
        }
        let stats = store.stats().expect("stats available");
        assert!(stats.memory_bytes <= 8);
        assert!(stats.disk_bytes <= 210);
        assert!(stats.eviction_count > 0);
        let _ = fs::remove_dir_all(path);
    }
}
