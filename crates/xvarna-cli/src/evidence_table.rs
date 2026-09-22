//! Typed Apache Parquet export for public benchmark evidence.

use arrow_array::{ArrayRef, BooleanArray, Float64Array, RecordBatch, StringArray, UInt64Array};
use arrow_schema::{DataType, Field, Schema};
use parquet::{arrow::ArrowWriter, basic::Compression, file::properties::WriterProperties};
use serde::Deserialize;
use std::{fs::File, path::Path, process::ExitCode, sync::Arc};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EvidenceDocument {
    schema_version: String,
    rows: Vec<EvidenceRow>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EvidenceRow {
    tier: String,
    requested_backend: String,
    backend: String,
    adapter: String,
    triangles: u64,
    sensors: u64,
    rays: u64,
    cold_milliseconds: f64,
    p50_milliseconds: f64,
    p95_milliseconds: f64,
    memory_bytes: u64,
    vram_bytes: u64,
    parity_accepted: bool,
    device_losses: u64,
    runtime_fallbacks: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DaylightEvidenceDocument {
    schema_version: String,
    radiance_version: String,
    case_id: String,
    rows: Vec<DaylightEvidenceRow>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DaylightEvidenceRow {
    sensor_id: u64,
    fast_lux: f64,
    reference_lux: f64,
    delta_lux: f64,
    absolute_error_lux: f64,
    percentage_error: f64,
}

/// Converts the strict benchmark evidence JSON contract to real Apache Parquet.
pub fn run_evidence_table(input: &str, output: &str) -> ExitCode {
    match write_evidence_table(Path::new(input), Path::new(output)) {
        Ok(rows) => {
            println!("Wrote {rows} benchmark rows to {output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(1)
        }
    }
}

/// Converts paired Fast/Reference daylight validation rows to Apache Parquet.
pub fn run_daylight_evidence_table(input: &str, output: &str) -> ExitCode {
    match write_daylight_evidence_table(Path::new(input), Path::new(output)) {
        Ok(rows) => {
            println!("Wrote {rows} daylight validation rows to {output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(1)
        }
    }
}

fn write_daylight_evidence_table(input: &Path, output: &Path) -> Result<usize, String> {
    let bytes = std::fs::read(input).map_err(|error| error.to_string())?;
    let document: DaylightEvidenceDocument =
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    if document.schema_version != "1.0.0"
        || document.radiance_version.is_empty()
        || document.case_id.is_empty()
        || document.rows.is_empty()
    {
        return Err("daylight evidence requires schema 1.0.0, provenance, and rows".to_owned());
    }
    if document.rows.iter().any(|row| {
        !row.fast_lux.is_finite()
            || !row.reference_lux.is_finite()
            || !row.delta_lux.is_finite()
            || !row.absolute_error_lux.is_finite()
            || !row.percentage_error.is_finite()
    }) {
        return Err("daylight evidence values must be finite".to_owned());
    }
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let schema = Arc::new(Schema::new(vec![
        Field::new("case_id", DataType::Utf8, false),
        Field::new("radiance_version", DataType::Utf8, false),
        Field::new("sensor_id", DataType::UInt64, false),
        Field::new("fast_lux", DataType::Float64, false),
        Field::new("reference_lux", DataType::Float64, false),
        Field::new("delta_lux", DataType::Float64, false),
        Field::new("absolute_error_lux", DataType::Float64, false),
        Field::new("percentage_error", DataType::Float64, false),
    ]));
    let row_count = document.rows.len();
    let columns: Vec<ArrayRef> = vec![
        Arc::new(StringArray::from_iter_values(std::iter::repeat_n(
            document.case_id.as_str(),
            row_count,
        ))),
        Arc::new(StringArray::from_iter_values(std::iter::repeat_n(
            document.radiance_version.as_str(),
            row_count,
        ))),
        Arc::new(UInt64Array::from_iter_values(
            document.rows.iter().map(|row| row.sensor_id),
        )),
        Arc::new(Float64Array::from_iter_values(
            document.rows.iter().map(|row| row.fast_lux),
        )),
        Arc::new(Float64Array::from_iter_values(
            document.rows.iter().map(|row| row.reference_lux),
        )),
        Arc::new(Float64Array::from_iter_values(
            document.rows.iter().map(|row| row.delta_lux),
        )),
        Arc::new(Float64Array::from_iter_values(
            document.rows.iter().map(|row| row.absolute_error_lux),
        )),
        Arc::new(Float64Array::from_iter_values(
            document.rows.iter().map(|row| row.percentage_error),
        )),
    ];
    let batch =
        RecordBatch::try_new(Arc::clone(&schema), columns).map_err(|error| error.to_string())?;
    let properties = WriterProperties::builder()
        .set_compression(Compression::SNAPPY)
        .set_created_by("XVARNA 1.0 daylight evidence".to_owned())
        .build();
    let file = File::create(output).map_err(|error| error.to_string())?;
    let mut writer =
        ArrowWriter::try_new(file, schema, Some(properties)).map_err(|error| error.to_string())?;
    writer.write(&batch).map_err(|error| error.to_string())?;
    writer.close().map_err(|error| error.to_string())?;
    Ok(row_count)
}

#[allow(clippy::too_many_lines)]
fn write_evidence_table(input: &Path, output: &Path) -> Result<usize, String> {
    let bytes = std::fs::read(input).map_err(|error| error.to_string())?;
    let document: EvidenceDocument =
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    if document.schema_version != "1.0.0" || document.rows.is_empty() {
        return Err("evidence schemaVersion must be 1.0.0 and rows must be non-empty".to_owned());
    }
    for row in &document.rows {
        if row.tier.is_empty()
            || row.requested_backend.is_empty()
            || row.backend.is_empty()
            || !row.cold_milliseconds.is_finite()
            || !row.p50_milliseconds.is_finite()
            || !row.p95_milliseconds.is_finite()
        {
            return Err("benchmark rows require names and finite timing values".to_owned());
        }
    }
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let fields = vec![
        Field::new("tier", DataType::Utf8, false),
        Field::new("requested_backend", DataType::Utf8, false),
        Field::new("backend", DataType::Utf8, false),
        Field::new("adapter", DataType::Utf8, false),
        Field::new("triangles", DataType::UInt64, false),
        Field::new("sensors", DataType::UInt64, false),
        Field::new("rays", DataType::UInt64, false),
        Field::new("cold_milliseconds", DataType::Float64, false),
        Field::new("p50_milliseconds", DataType::Float64, false),
        Field::new("p95_milliseconds", DataType::Float64, false),
        Field::new("memory_bytes", DataType::UInt64, false),
        Field::new("vram_bytes", DataType::UInt64, false),
        Field::new("parity_accepted", DataType::Boolean, false),
        Field::new("device_losses", DataType::UInt64, false),
        Field::new("runtime_fallbacks", DataType::UInt64, false),
    ];
    let columns: Vec<ArrayRef> = vec![
        Arc::new(StringArray::from_iter_values(
            document.rows.iter().map(|row| row.tier.as_str()),
        )),
        Arc::new(StringArray::from_iter_values(
            document
                .rows
                .iter()
                .map(|row| row.requested_backend.as_str()),
        )),
        Arc::new(StringArray::from_iter_values(
            document.rows.iter().map(|row| row.backend.as_str()),
        )),
        Arc::new(StringArray::from_iter_values(
            document.rows.iter().map(|row| row.adapter.as_str()),
        )),
        Arc::new(UInt64Array::from_iter_values(
            document.rows.iter().map(|row| row.triangles),
        )),
        Arc::new(UInt64Array::from_iter_values(
            document.rows.iter().map(|row| row.sensors),
        )),
        Arc::new(UInt64Array::from_iter_values(
            document.rows.iter().map(|row| row.rays),
        )),
        Arc::new(Float64Array::from_iter_values(
            document.rows.iter().map(|row| row.cold_milliseconds),
        )),
        Arc::new(Float64Array::from_iter_values(
            document.rows.iter().map(|row| row.p50_milliseconds),
        )),
        Arc::new(Float64Array::from_iter_values(
            document.rows.iter().map(|row| row.p95_milliseconds),
        )),
        Arc::new(UInt64Array::from_iter_values(
            document.rows.iter().map(|row| row.memory_bytes),
        )),
        Arc::new(UInt64Array::from_iter_values(
            document.rows.iter().map(|row| row.vram_bytes),
        )),
        Arc::new(BooleanArray::from(
            document
                .rows
                .iter()
                .map(|row| row.parity_accepted)
                .collect::<Vec<_>>(),
        )),
        Arc::new(UInt64Array::from_iter_values(
            document.rows.iter().map(|row| row.device_losses),
        )),
        Arc::new(UInt64Array::from_iter_values(
            document.rows.iter().map(|row| row.runtime_fallbacks),
        )),
    ];
    let schema = Arc::new(Schema::new(fields));
    let batch =
        RecordBatch::try_new(Arc::clone(&schema), columns).map_err(|error| error.to_string())?;
    let properties = WriterProperties::builder()
        .set_compression(Compression::SNAPPY)
        .set_created_by("XVARNA 1.0 evidence-table".to_owned())
        .build();
    let file = File::create(output).map_err(|error| error.to_string())?;
    let mut writer =
        ArrowWriter::try_new(file, schema, Some(properties)).map_err(|error| error.to_string())?;
    writer.write(&batch).map_err(|error| error.to_string())?;
    writer.close().map_err(|error| error.to_string())?;
    Ok(document.rows.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use parquet::file::reader::{FileReader, SerializedFileReader};

    #[test]
    fn strict_json_becomes_readable_parquet() {
        let root =
            std::env::temp_dir().join(format!("xvarna-evidence-table-{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("temporary directory");
        let input = root.join("rows.json");
        let output = root.join("rows.parquet");
        std::fs::write(&input, r#"{"schemaVersion":"1.0.0","rows":[{"tier":"S","requestedBackend":"cpu","backend":"CPU","adapter":"canonical","triangles":100352,"sensors":10000,"rays":4096,"coldMilliseconds":1.0,"p50Milliseconds":0.5,"p95Milliseconds":0.7,"memoryBytes":1,"vramBytes":0,"parityAccepted":true,"deviceLosses":0,"runtimeFallbacks":0}]}"#).expect("fixture");
        assert_eq!(write_evidence_table(&input, &output), Ok(1));
        let reader =
            SerializedFileReader::new(File::open(&output).expect("output")).expect("parquet");
        assert_eq!(reader.metadata().file_metadata().num_rows(), 1);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn daylight_json_becomes_readable_parquet() {
        let root = std::env::temp_dir().join(format!(
            "xvarna-daylight-evidence-table-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("temporary directory");
        let input = root.join("rows.json");
        let output = root.join("rows.parquet");
        std::fs::write(&input, r#"{"schemaVersion":"1.0.0","radianceVersion":"6.1a","caseId":"open-room","rows":[{"sensorId":1,"fastLux":100.0,"referenceLux":101.0,"deltaLux":-1.0,"absoluteErrorLux":1.0,"percentageError":0.9900990099}]}"#).expect("fixture");
        assert_eq!(write_daylight_evidence_table(&input, &output), Ok(1));
        let reader =
            SerializedFileReader::new(File::open(&output).expect("output")).expect("parquet");
        assert_eq!(reader.metadata().file_metadata().num_rows(), 1);
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
