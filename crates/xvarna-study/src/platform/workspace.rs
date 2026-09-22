//! Transactional disk workspace with exact optimizer checkpoint/resume semantics.

use super::{
    EvaluationRecord, MetricField, PlatformError, STUDY_PLATFORM_SCHEMA_VERSION,
    SensitivityOptions, StudyAnalysis, StudyLedger, StudyManifest, VariantRecord,
    analyze_platform_study, hash_serializable, study_manifest_schema,
};
use crate::{Candidate, Evaluation, MultiObjectiveOptimizer, OptimizerCheckpoint};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const MANIFEST_FILE: &str = "manifest.json";
const SCHEMA_FILE: &str = "study-manifest.schema.json";
const LEDGER_FILE: &str = "ledger.json";
const CHECKPOINT_FILE: &str = "checkpoint.json";
const TRANSACTION_FILE: &str = "transaction.json";

/// Lifecycle state of a persisted external-evaluation batch.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BatchStatus {
    /// Candidates exist and await complete evaluation rows.
    Pending,
    /// Every candidate was committed exactly once.
    Completed,
}

/// Rich external result row supplied when committing a pending batch.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BatchEvaluation {
    /// Pending candidate/variant ID.
    pub variant_id: u64,
    /// One scalar/vector row per manifest objective.
    pub objective_values: Vec<Vec<f64>>,
    /// Residual constraints, feasible when all values are non-positive.
    #[serde(default)]
    pub constraint_residuals: Vec<f64>,
    /// Optional indexed/spatial fields for scenario delta maps.
    #[serde(default)]
    pub metric_fields: Vec<MetricField>,
    /// External evaluation runtime.
    #[serde(default)]
    pub elapsed_microseconds: u64,
    /// Upstream scene/analysis identity.
    #[serde(default)]
    pub provenance_hash: String,
}

/// Durable hand-off between the optimizer and any external evaluator.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BatchEnvelope {
    /// Platform schema version.
    pub schema_version: String,
    /// Monotonic batch ID.
    pub batch_id: u64,
    /// Candidate generation.
    pub generation: u64,
    /// Pending or completed.
    pub status: BatchStatus,
    /// Creation time as Unix seconds UTC.
    pub created_unix_seconds_utc: u64,
    /// Completion time when committed.
    #[serde(default)]
    pub completed_unix_seconds_utc: Option<u64>,
    /// Exact pending candidates.
    pub candidates: Vec<Candidate>,
    /// Completed rich results; empty while pending.
    #[serde(default)]
    pub evaluations: Vec<BatchEvaluation>,
    /// Stable envelope identity excluding this field.
    pub content_hash: String,
}

impl BatchEnvelope {
    fn refresh_hash(&mut self) {
        let mut clone = self.clone();
        clone.content_hash.clear();
        self.content_hash = hash_serializable(b"XVARNA_STUDY_BATCH_V1\0", &clone);
    }

    fn validate_hash(&self) -> Result<(), PlatformError> {
        let mut clone = self.clone();
        clone.content_hash.clear();
        (hash_serializable(b"XVARNA_STUDY_BATCH_V1\0", &clone) == self.content_hash)
            .then_some(())
            .ok_or(PlatformError::CorruptWorkspace)
    }
}

/// Complete workspace checkpoint including exact native optimizer state.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceCheckpoint {
    /// Platform schema version.
    pub schema_version: String,
    /// Monotonic transaction revision.
    pub revision: u64,
    /// Active pending batch, if any.
    #[serde(default)]
    pub active_batch_id: Option<u64>,
    /// Exact optimizer state including PRNG and pending candidates.
    pub optimizer: OptimizerCheckpoint,
    /// Ledger identity this checkpoint references.
    pub ledger_hash: String,
    /// Stable checkpoint identity excluding this field.
    pub content_hash: String,
}

impl WorkspaceCheckpoint {
    fn refresh_hash(&mut self) {
        let mut clone = self.clone();
        clone.content_hash.clear();
        self.content_hash = hash_serializable(b"XVARNA_STUDY_WORKSPACE_CHECKPOINT_V1\0", &clone);
    }

    fn validate(&self, ledger: &StudyLedger) -> Result<(), PlatformError> {
        let mut clone = self.clone();
        clone.content_hash.clear();
        if self.schema_version != STUDY_PLATFORM_SCHEMA_VERSION
            || self.ledger_hash != ledger.content_hash
            || hash_serializable(b"XVARNA_STUDY_WORKSPACE_CHECKPOINT_V1\0", &clone)
                != self.content_hash
        {
            return Err(PlatformError::CorruptWorkspace);
        }
        MultiObjectiveOptimizer::from_checkpoint(self.optimizer.clone())?;
        Ok(())
    }
}

/// Compact state returned by open/begin/commit for UI and automation.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeState {
    /// Workspace revision.
    pub revision: u64,
    /// Completed optimizer generations.
    pub generation: u64,
    /// Accepted evaluation count.
    pub evaluation_count: u64,
    /// Pending batch ID, if evaluation must resume.
    pub active_batch_id: Option<u64>,
    /// Exact pending candidates.
    pub pending_candidates: Vec<Candidate>,
    /// Registered variant count including pending rows.
    pub variant_count: usize,
    /// Completed evaluation count.
    pub completed_count: usize,
    /// Ledger identity.
    pub ledger_hash: String,
    /// Checkpoint identity.
    pub checkpoint_hash: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceTransaction {
    ledger: StudyLedger,
    checkpoint: WorkspaceCheckpoint,
    batch: BatchEnvelope,
}

/// Open transactional Study directory.
#[derive(Debug)]
pub struct StudyWorkspace {
    root: PathBuf,
    manifest: StudyManifest,
    ledger: StudyLedger,
    checkpoint: WorkspaceCheckpoint,
}

impl StudyWorkspace {
    /// Initializes a new directory with manifest, schema, empty ledger, and resumable optimizer.
    pub fn create(root: impl AsRef<Path>, manifest: StudyManifest) -> Result<Self, PlatformError> {
        manifest.validate()?;
        let root = root.as_ref().to_path_buf();
        if root.join(MANIFEST_FILE).exists() {
            return Err(PlatformError::WorkspaceExists);
        }
        fs::create_dir_all(root.join("batches"))?;
        fs::create_dir_all(root.join("report"))?;
        let problem = manifest.problem()?;
        let optimizer = MultiObjectiveOptimizer::new(problem, manifest.optimizer)?.checkpoint();
        let ledger = StudyLedger::empty();
        let mut checkpoint = WorkspaceCheckpoint {
            schema_version: STUDY_PLATFORM_SCHEMA_VERSION.to_owned(),
            revision: 0,
            active_batch_id: None,
            optimizer,
            ledger_hash: ledger.content_hash.clone(),
            content_hash: String::new(),
        };
        checkpoint.refresh_hash();
        atomic_json(&root.join(SCHEMA_FILE), &study_manifest_schema())?;
        atomic_json(&root.join(MANIFEST_FILE), &manifest)?;
        atomic_json(&root.join(LEDGER_FILE), &ledger)?;
        atomic_json(&root.join(CHECKPOINT_FILE), &checkpoint)?;
        Ok(Self {
            root,
            manifest,
            ledger,
            checkpoint,
        })
    }

    /// Opens a workspace, completing an interrupted journaled transaction if necessary.
    pub fn open(root: impl AsRef<Path>) -> Result<Self, PlatformError> {
        let root = root.as_ref().to_path_buf();
        if !root.join(MANIFEST_FILE).is_file() {
            return Err(PlatformError::WorkspaceMissing);
        }
        recover_transaction(&root)?;
        let manifest: StudyManifest = read_json_recover(&root.join(MANIFEST_FILE))?;
        manifest.validate()?;
        let ledger: StudyLedger = read_json_recover(&root.join(LEDGER_FILE))?;
        ledger.validate(&manifest)?;
        let checkpoint: WorkspaceCheckpoint = read_json_recover(&root.join(CHECKPOINT_FILE))?;
        checkpoint.validate(&ledger)?;
        let workspace = Self {
            root,
            manifest,
            ledger,
            checkpoint,
        };
        workspace.validate_active_batch()?;
        Ok(workspace)
    }

    /// Workspace root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Immutable versioned manifest.
    #[must_use]
    pub const fn manifest(&self) -> &StudyManifest {
        &self.manifest
    }

    /// Hash-protected current ledger.
    #[must_use]
    pub const fn ledger(&self) -> &StudyLedger {
        &self.ledger
    }

    /// Current resume state without mutating disk or optimizer state.
    #[must_use]
    pub fn resume_state(&self) -> ResumeState {
        ResumeState {
            revision: self.checkpoint.revision,
            generation: self.checkpoint.optimizer.generation,
            evaluation_count: self.checkpoint.optimizer.evaluation_count,
            active_batch_id: self.checkpoint.active_batch_id,
            pending_candidates: self.checkpoint.optimizer.pending.clone(),
            variant_count: self.ledger.variants.len(),
            completed_count: self.ledger.evaluations.len(),
            ledger_hash: self.ledger.content_hash.clone(),
            checkpoint_hash: self.checkpoint.content_hash.clone(),
        }
    }

    /// Returns the active batch or creates and transactionally checkpoints the next one.
    pub fn begin_batch(&mut self) -> Result<BatchEnvelope, PlatformError> {
        if let Some(batch_id) = self.checkpoint.active_batch_id {
            return self.read_batch(batch_id);
        }
        let mut optimizer =
            MultiObjectiveOptimizer::from_checkpoint(self.checkpoint.optimizer.clone())?;
        let candidates = optimizer.ask()?;
        let batch_id = self.checkpoint.revision.saturating_add(1);
        let generation = candidates.first().map_or(0, |value| value.generation);
        let mut batch = BatchEnvelope {
            schema_version: STUDY_PLATFORM_SCHEMA_VERSION.to_owned(),
            batch_id,
            generation,
            status: BatchStatus::Pending,
            created_unix_seconds_utc: unix_seconds(),
            completed_unix_seconds_utc: None,
            candidates: candidates.clone(),
            evaluations: Vec::new(),
            content_hash: String::new(),
        };
        batch.refresh_hash();
        let existing_ids = self
            .ledger
            .variants
            .iter()
            .map(|value| value.variant_id)
            .collect::<BTreeSet<_>>();
        if candidates
            .iter()
            .any(|candidate| existing_ids.contains(&candidate.candidate_id))
        {
            return Err(PlatformError::InvalidBatchState);
        }
        let mut ledger = self.ledger.clone();
        ledger.revision = ledger.revision.saturating_add(1);
        ledger
            .variants
            .extend(candidates.iter().map(|candidate| VariantRecord {
                variant_id: candidate.candidate_id,
                generation: candidate.generation,
                label: format!("Variant {}", candidate.candidate_id),
                parameters: candidate.values.clone(),
                parent_variant_id: None,
                asset_uri: None,
                tags: BTreeMap::new(),
            }));
        ledger.refresh_hash();
        let mut checkpoint = WorkspaceCheckpoint {
            schema_version: STUDY_PLATFORM_SCHEMA_VERSION.to_owned(),
            revision: self.checkpoint.revision.saturating_add(1),
            active_batch_id: Some(batch_id),
            optimizer: optimizer.checkpoint(),
            ledger_hash: ledger.content_hash.clone(),
            content_hash: String::new(),
        };
        checkpoint.refresh_hash();
        self.commit_transaction(WorkspaceTransaction {
            ledger,
            checkpoint,
            batch: batch.clone(),
        })?;
        Ok(batch)
    }

    /// Commits one complete evaluation per pending candidate exactly once.
    pub fn commit_batch(
        &mut self,
        batch_id: u64,
        evaluations: Vec<BatchEvaluation>,
    ) -> Result<ResumeState, PlatformError> {
        let mut batch = self.read_batch(batch_id)?;
        if batch.status == BatchStatus::Completed {
            return (batch.evaluations == evaluations)
                .then(|| self.resume_state())
                .ok_or(PlatformError::InvalidBatchState);
        }
        if self.checkpoint.active_batch_id != Some(batch_id)
            || evaluations.len() != batch.candidates.len()
        {
            return Err(PlatformError::InvalidBatchState);
        }
        let expected_ids = batch
            .candidates
            .iter()
            .map(|value| value.candidate_id)
            .collect::<BTreeSet<_>>();
        let actual_ids = evaluations
            .iter()
            .map(|value| value.variant_id)
            .collect::<BTreeSet<_>>();
        if expected_ids != actual_ids || actual_ids.len() != evaluations.len() {
            return Err(PlatformError::InvalidBatchState);
        }
        let evaluation_map = evaluations
            .iter()
            .map(|value| (value.variant_id, value))
            .collect::<BTreeMap<_, _>>();
        let core_evaluations = batch
            .candidates
            .iter()
            .map(|candidate| {
                let value = evaluation_map
                    .get(&candidate.candidate_id)
                    .ok_or(PlatformError::InvalidBatchState)?;
                let record = EvaluationRecord {
                    variant_id: value.variant_id,
                    objective_values: value.objective_values.clone(),
                    constraint_residuals: value.constraint_residuals.clone(),
                    metric_fields: value.metric_fields.clone(),
                    elapsed_microseconds: value.elapsed_microseconds,
                    provenance_hash: value.provenance_hash.clone(),
                };
                Ok(Evaluation {
                    candidate_id: value.variant_id,
                    objectives: record.flatten_objectives(&self.manifest)?,
                    constraint_residuals: value.constraint_residuals.clone(),
                })
            })
            .collect::<Result<Vec<_>, PlatformError>>()?;
        let mut optimizer =
            MultiObjectiveOptimizer::from_checkpoint(self.checkpoint.optimizer.clone())?;
        optimizer.tell(&core_evaluations)?;
        let completed_ids = self
            .ledger
            .evaluations
            .iter()
            .map(|value| value.variant_id)
            .collect::<BTreeSet<_>>();
        if actual_ids.iter().any(|id| completed_ids.contains(id)) {
            return Err(PlatformError::InvalidBatchState);
        }
        let mut ledger = self.ledger.clone();
        ledger.revision = ledger.revision.saturating_add(1);
        for candidate in &batch.candidates {
            let value = evaluation_map[&candidate.candidate_id];
            ledger.evaluations.push(EvaluationRecord {
                variant_id: value.variant_id,
                objective_values: value.objective_values.clone(),
                constraint_residuals: value.constraint_residuals.clone(),
                metric_fields: value.metric_fields.clone(),
                elapsed_microseconds: value.elapsed_microseconds,
                provenance_hash: value.provenance_hash.clone(),
            });
        }
        ledger.refresh_hash();
        ledger.validate(&self.manifest)?;
        let mut checkpoint = WorkspaceCheckpoint {
            schema_version: STUDY_PLATFORM_SCHEMA_VERSION.to_owned(),
            revision: self.checkpoint.revision.saturating_add(1),
            active_batch_id: None,
            optimizer: optimizer.checkpoint(),
            ledger_hash: ledger.content_hash.clone(),
            content_hash: String::new(),
        };
        checkpoint.refresh_hash();
        batch.status = BatchStatus::Completed;
        batch.completed_unix_seconds_utc = Some(unix_seconds());
        batch.evaluations = evaluations;
        batch.refresh_hash();
        self.commit_transaction(WorkspaceTransaction {
            ledger,
            checkpoint,
            batch,
        })?;
        Ok(self.resume_state())
    }

    /// Runs the complete derived analysis for every completed ledger row.
    pub fn analyze(&self, options: SensitivityOptions) -> Result<StudyAnalysis, PlatformError> {
        analyze_platform_study(&self.manifest, &self.ledger, options)
    }

    fn validate_active_batch(&self) -> Result<(), PlatformError> {
        match self.checkpoint.active_batch_id {
            Some(batch_id) => {
                let batch = self.read_batch(batch_id)?;
                if batch.status != BatchStatus::Pending
                    || batch.candidates != self.checkpoint.optimizer.pending
                {
                    return Err(PlatformError::CorruptWorkspace);
                }
            }
            None if !self.checkpoint.optimizer.pending.is_empty() => {
                return Err(PlatformError::CorruptWorkspace);
            }
            None => {}
        }
        Ok(())
    }

    fn read_batch(&self, batch_id: u64) -> Result<BatchEnvelope, PlatformError> {
        let batch: BatchEnvelope = read_json_recover(&batch_path(&self.root, batch_id))?;
        batch.validate_hash()?;
        if batch.schema_version != STUDY_PLATFORM_SCHEMA_VERSION || batch.batch_id != batch_id {
            return Err(PlatformError::CorruptWorkspace);
        }
        Ok(batch)
    }

    fn commit_transaction(
        &mut self,
        transaction: WorkspaceTransaction,
    ) -> Result<(), PlatformError> {
        validate_transaction(&self.manifest, &transaction)?;
        atomic_json(&self.root.join(TRANSACTION_FILE), &transaction)?;
        apply_transaction(&self.root, &transaction, &self.manifest)?;
        remove_if_exists(&self.root.join(TRANSACTION_FILE))?;
        self.ledger = transaction.ledger;
        self.checkpoint = transaction.checkpoint;
        Ok(())
    }
}

fn recover_transaction(root: &Path) -> Result<(), PlatformError> {
    let path = root.join(TRANSACTION_FILE);
    if path.is_file() || backup_path(&path).is_file() {
        let transaction: WorkspaceTransaction = read_json_recover(&path)?;
        let manifest: StudyManifest = read_json_recover(&root.join(MANIFEST_FILE))?;
        manifest.validate()?;
        apply_transaction(root, &transaction, &manifest)?;
        remove_if_exists(&path)?;
        remove_if_exists(&backup_path(&path))?;
    }
    Ok(())
}

fn apply_transaction(
    root: &Path,
    transaction: &WorkspaceTransaction,
    manifest: &StudyManifest,
) -> Result<(), PlatformError> {
    validate_transaction(manifest, transaction)?;
    atomic_json(&root.join(LEDGER_FILE), &transaction.ledger)?;
    atomic_json(&root.join(CHECKPOINT_FILE), &transaction.checkpoint)?;
    atomic_json(
        &batch_path(root, transaction.batch.batch_id),
        &transaction.batch,
    )?;
    Ok(())
}

fn validate_transaction(
    manifest: &StudyManifest,
    transaction: &WorkspaceTransaction,
) -> Result<(), PlatformError> {
    transaction.batch.validate_hash()?;
    transaction.ledger.validate(manifest)?;
    transaction.checkpoint.validate(&transaction.ledger)?;
    if transaction.ledger.revision != transaction.checkpoint.revision {
        return Err(PlatformError::CorruptWorkspace);
    }
    match transaction.batch.status {
        BatchStatus::Pending
            if transaction.checkpoint.active_batch_id == Some(transaction.batch.batch_id)
                && transaction.checkpoint.optimizer.pending == transaction.batch.candidates
                && transaction.batch.evaluations.is_empty() =>
        {
            Ok(())
        }
        BatchStatus::Completed
            if transaction.checkpoint.active_batch_id.is_none()
                && transaction.checkpoint.optimizer.pending.is_empty()
                && transaction.batch.evaluations.len() == transaction.batch.candidates.len() =>
        {
            Ok(())
        }
        BatchStatus::Pending | BatchStatus::Completed => Err(PlatformError::CorruptWorkspace),
    }
}

fn batch_path(root: &Path, batch_id: u64) -> PathBuf {
    root.join("batches")
        .join(format!("batch-{batch_id:08}.json"))
}

fn atomic_json<T: Serialize>(path: &Path, value: &T) -> Result<(), PlatformError> {
    let bytes = serde_json::to_vec_pretty(value)?;
    let temporary = temporary_path(path);
    let backup = backup_path(path);
    remove_if_exists(&temporary)?;
    let mut file = File::create(&temporary)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    if path.exists() {
        remove_if_exists(&backup)?;
        fs::rename(path, &backup)?;
    }
    if let Err(error) = fs::rename(&temporary, path) {
        if backup.exists() && !path.exists() {
            let _ = fs::rename(&backup, path);
        }
        return Err(PlatformError::Io(error));
    }
    remove_if_exists(&backup)?;
    Ok(())
}

fn read_json_recover<T: DeserializeOwned>(path: &Path) -> Result<T, PlatformError> {
    for candidate in [path.to_path_buf(), backup_path(path)] {
        if let Ok(bytes) = fs::read(&candidate)
            && let Ok(value) = serde_json::from_slice(&bytes)
        {
            return Ok(value);
        }
    }
    Err(PlatformError::CorruptWorkspace)
}

fn remove_if_exists(path: &Path) -> Result<(), PlatformError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(PlatformError::Io(error)),
    }
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .map_or_else(|| "xvarna".into(), std::ffi::OsStr::to_os_string);
    name.push(format!(".tmp-{}", std::process::id()));
    path.with_file_name(name)
}

fn backup_path(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .map_or_else(|| "xvarna".into(), std::ffi::OsStr::to_os_string);
    name.push(".previous");
    path.with_file_name(name)
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |value| value.as_secs())
}
