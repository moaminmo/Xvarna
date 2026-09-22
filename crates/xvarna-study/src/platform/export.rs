//! Standards-based Parquet/glTF export and self-contained interactive HTML reporting.

use super::{
    EvaluationRecord, PlatformError, RankedVariant, StudyAnalysis, StudyLedger, StudyManifest,
};
use arrow_array::{ArrayRef, BooleanArray, Float64Array, RecordBatch, StringArray, UInt64Array};
use arrow_schema::{DataType, Field, Schema};
use parquet::{arrow::ArrowWriter, basic::Compression, file::properties::WriterProperties};
use serde::Serialize;
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs::{self, File},
    path::{Path, PathBuf},
    sync::Arc,
};

/// Files produced by one reproducible report export.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReportArtifacts {
    /// Complete portable JSON data.
    pub data_json: PathBuf,
    /// Typed columnar variant table.
    pub variants_parquet: PathBuf,
    /// Embedded-buffer glTF objective-space point cloud.
    pub objective_space_gltf: PathBuf,
    /// Narrative and interactive self-contained report.
    pub html_report: PathBuf,
    /// Compact self-contained interactive viewer.
    pub standalone_viewer: PathBuf,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReportData<'a> {
    schema_version: &'static str,
    manifest: &'a StudyManifest,
    ledger: &'a StudyLedger,
    analysis: &'a StudyAnalysis,
    objective_columns: Vec<String>,
    generated_by: &'static str,
}

/// Writes JSON, real Apache Parquet, glTF 2.0, and two network-free HTML artifacts.
pub fn write_report_package(
    output_directory: impl AsRef<Path>,
    manifest: &StudyManifest,
    ledger: &StudyLedger,
    analysis: &StudyAnalysis,
) -> Result<ReportArtifacts, PlatformError> {
    ledger.validate(manifest)?;
    let output_directory = output_directory.as_ref();
    fs::create_dir_all(output_directory)?;
    let artifacts = ReportArtifacts {
        data_json: output_directory.join("study-data.json"),
        variants_parquet: output_directory.join("variants.parquet"),
        objective_space_gltf: output_directory.join("objective-space.gltf"),
        html_report: output_directory.join("report.html"),
        standalone_viewer: output_directory.join("viewer.html"),
    };
    let data = ReportData {
        schema_version: super::STUDY_PLATFORM_SCHEMA_VERSION,
        manifest,
        ledger,
        analysis,
        objective_columns: manifest.objective_column_names(),
        generated_by: "XVARNA VAHMAN 0.16.0",
    };
    let json_bytes = serde_json::to_vec_pretty(&data)?;
    fs::write(&artifacts.data_json, &json_bytes)?;
    write_parquet(&artifacts.variants_parquet, manifest, ledger, analysis)?;
    write_objective_gltf(&artifacts.objective_space_gltf, manifest, ledger, analysis)?;
    let embedded = safe_script_json(&serde_json::to_string(&data)?);
    fs::write(&artifacts.html_report, render_html(&embedded, false))?;
    fs::write(&artifacts.standalone_viewer, render_html(&embedded, true))?;
    Ok(artifacts)
}

#[allow(clippy::too_many_lines)]
fn write_parquet(
    path: &Path,
    manifest: &StudyManifest,
    ledger: &StudyLedger,
    analysis: &StudyAnalysis,
) -> Result<(), PlatformError> {
    let variants = ledger
        .variants
        .iter()
        .map(|value| (value.variant_id, value))
        .collect::<BTreeMap<_, _>>();
    let ranking = analysis
        .ranked_variants
        .iter()
        .map(|value| (value.variant_id, value))
        .collect::<BTreeMap<_, _>>();
    let rows = ledger
        .evaluations
        .iter()
        .map(|evaluation| {
            let variant = variants
                .get(&evaluation.variant_id)
                .ok_or(PlatformError::InvalidLedger)?;
            let rank = ranking
                .get(&evaluation.variant_id)
                .ok_or(PlatformError::InvalidLedger)?;
            Ok((*variant, evaluation, *rank))
        })
        .collect::<Result<Vec<_>, PlatformError>>()?;
    let mut fields = vec![
        Field::new("variant_id", DataType::UInt64, false),
        Field::new("generation", DataType::UInt64, false),
        Field::new("label", DataType::Utf8, false),
        Field::new("feasible", DataType::Boolean, false),
        Field::new("pareto_rank", DataType::UInt64, false),
        Field::new("constraint_violation", DataType::Float64, false),
        Field::new("crowding_distance", DataType::Float64, true),
        Field::new("provenance_hash", DataType::Utf8, false),
        Field::new("elapsed_microseconds", DataType::UInt64, false),
    ];
    let mut columns: Vec<ArrayRef> = vec![
        Arc::new(UInt64Array::from(
            rows.iter()
                .map(|(variant, _, _)| variant.variant_id)
                .collect::<Vec<_>>(),
        )),
        Arc::new(UInt64Array::from(
            rows.iter()
                .map(|(variant, _, _)| variant.generation)
                .collect::<Vec<_>>(),
        )),
        Arc::new(StringArray::from(
            rows.iter()
                .map(|(variant, _, _)| variant.label.clone())
                .collect::<Vec<_>>(),
        )),
        Arc::new(BooleanArray::from(
            rows.iter()
                .map(|(_, _, rank)| rank.feasible)
                .collect::<Vec<_>>(),
        )),
        Arc::new(UInt64Array::from(
            rows.iter()
                .map(|(_, _, rank)| u64::try_from(rank.pareto_rank).unwrap_or(u64::MAX))
                .collect::<Vec<_>>(),
        )),
        Arc::new(Float64Array::from(
            rows.iter()
                .map(|(_, _, rank)| rank.total_constraint_violation)
                .collect::<Vec<_>>(),
        )),
        Arc::new(Float64Array::from(
            rows.iter()
                .map(|(_, _, rank)| rank.crowding_distance)
                .collect::<Vec<_>>(),
        )),
        Arc::new(StringArray::from(
            rows.iter()
                .map(|(_, evaluation, _)| evaluation.provenance_hash.clone())
                .collect::<Vec<_>>(),
        )),
        Arc::new(UInt64Array::from(
            rows.iter()
                .map(|(_, evaluation, _)| evaluation.elapsed_microseconds)
                .collect::<Vec<_>>(),
        )),
    ];
    for (index, parameter) in manifest.parameters.iter().enumerate() {
        fields.push(Field::new(
            format!(
                "parameter_{}_{}",
                parameter.parameter_id,
                safe_column(&parameter.name)
            ),
            DataType::Float64,
            false,
        ));
        columns.push(Arc::new(Float64Array::from(
            rows.iter()
                .map(|(variant, _, _)| variant.parameters[index])
                .collect::<Vec<_>>(),
        )));
    }
    let objective_names = manifest.objective_column_names();
    for (index, name) in objective_names.iter().enumerate() {
        fields.push(Field::new(
            format!("objective_{index}_{}", safe_column(name)),
            DataType::Float64,
            false,
        ));
        columns.push(Arc::new(Float64Array::from(
            rows.iter()
                .map(|(_, evaluation, _)| flatten(evaluation)[index])
                .collect::<Vec<_>>(),
        )));
    }
    for (index, constraint) in manifest.constraints.iter().enumerate() {
        fields.push(Field::new(
            format!(
                "constraint_{}_{}",
                constraint.constraint_id,
                safe_column(&constraint.name)
            ),
            DataType::Float64,
            false,
        ));
        columns.push(Arc::new(Float64Array::from(
            rows.iter()
                .map(|(_, evaluation, _)| evaluation.constraint_residuals[index])
                .collect::<Vec<_>>(),
        )));
    }
    let schema = Arc::new(Schema::new(fields));
    let batch = RecordBatch::try_new(Arc::clone(&schema), columns)
        .map_err(|error| PlatformError::Parquet(error.to_string()))?;
    let properties = WriterProperties::builder()
        .set_compression(Compression::SNAPPY)
        .set_created_by("XVARNA VAHMAN 0.16.0".to_owned())
        .build();
    let file = File::create(path)?;
    let mut writer = ArrowWriter::try_new(file, schema, Some(properties))
        .map_err(|error| PlatformError::Parquet(error.to_string()))?;
    writer
        .write(&batch)
        .map_err(|error| PlatformError::Parquet(error.to_string()))?;
    writer
        .close()
        .map_err(|error| PlatformError::Parquet(error.to_string()))?;
    Ok(())
}

#[allow(clippy::cast_possible_truncation)]
fn write_objective_gltf(
    path: &Path,
    manifest: &StudyManifest,
    ledger: &StudyLedger,
    analysis: &StudyAnalysis,
) -> Result<(), PlatformError> {
    let ranking = analysis
        .ranked_variants
        .iter()
        .map(|value| (value.variant_id, value))
        .collect::<BTreeMap<_, _>>();
    let objective_rows = ledger.evaluations.iter().map(flatten).collect::<Vec<_>>();
    let dimensions = manifest.objective_column_names().len();
    let mut minima = vec![f64::INFINITY; dimensions];
    let mut maxima = vec![f64::NEG_INFINITY; dimensions];
    for row in &objective_rows {
        for (index, value) in row.iter().enumerate() {
            minima[index] = minima[index].min(*value);
            maxima[index] = maxima[index].max(*value);
        }
    }
    let mut bytes = Vec::with_capacity(objective_rows.len() * 16);
    let mut position_min = [f32::INFINITY; 3];
    let mut position_max = [f32::NEG_INFINITY; 3];
    for row in &objective_rows {
        for axis in 0..3 {
            let value = row
                .get(axis)
                .map_or(0.0, |value| normalize(*value, minima[axis], maxima[axis]))
                as f32;
            position_min[axis] = position_min[axis].min(value);
            position_max[axis] = position_max[axis].max(value);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    let position_bytes = bytes.len();
    for evaluation in &ledger.evaluations {
        let rank = ranking
            .get(&evaluation.variant_id)
            .ok_or(PlatformError::InvalidLedger)?;
        let colour = point_colour(rank);
        bytes.extend_from_slice(&colour);
    }
    let encoded = base64_encode(&bytes);
    let names = manifest.objective_column_names();
    let gltf = json!({
        "asset": { "version": "2.0", "generator": "XVARNA VAHMAN 0.16.0", "copyright": "Apache-2.0" },
        "scene": 0,
        "scenes": [{ "name": manifest.title, "nodes": [0] }],
        "nodes": [{ "name": "Evaluated variants in normalized objective space", "mesh": 0 }],
        "meshes": [{
            "name": "Pareto study points",
            "primitives": [{ "attributes": { "POSITION": 0, "COLOR_0": 1 }, "mode": 0 }],
            "extras": {
                "variantIds": ledger.evaluations.iter().map(|value| value.variant_id).collect::<Vec<_>>(),
                "objectiveColumns": names,
                "normalization": "per-axis min/max mapped to [-1,1]"
            }
        }],
        "buffers": [{ "byteLength": bytes.len(), "uri": format!("data:application/octet-stream;base64,{encoded}") }],
        "bufferViews": [
            { "buffer": 0, "byteOffset": 0, "byteLength": position_bytes, "byteStride": 12, "target": 34962 },
            { "buffer": 0, "byteOffset": position_bytes, "byteLength": ledger.evaluations.len() * 4, "byteStride": 4, "target": 34962 }
        ],
        "accessors": [
            { "bufferView": 0, "byteOffset": 0, "componentType": 5126, "count": ledger.evaluations.len(), "type": "VEC3", "min": position_min, "max": position_max },
            { "bufferView": 1, "byteOffset": 0, "componentType": 5121, "normalized": true, "count": ledger.evaluations.len(), "type": "VEC4" }
        ],
        "extras": { "xvarnaStudySchema": "0.16.0", "analysisHash": analysis.content_hash }
    });
    fs::write(path, serde_json::to_vec_pretty(&gltf)?)?;
    Ok(())
}

fn flatten(evaluation: &EvaluationRecord) -> Vec<f64> {
    evaluation
        .objective_values
        .iter()
        .flatten()
        .copied()
        .collect()
}

fn normalize(value: f64, minimum: f64, maximum: f64) -> f64 {
    let range = maximum - minimum;
    if range.abs() <= f64::EPSILON {
        0.0
    } else {
        ((value - minimum) / range).mul_add(2.0, -1.0)
    }
}

const fn point_colour(rank: &RankedVariant) -> [u8; 4] {
    if !rank.feasible {
        [239, 68, 68, 255]
    } else if rank.pareto_rank == 0 {
        [34, 197, 94, 255]
    } else {
        [56, 189, 248, 210]
    }
}

fn safe_column(value: &str) -> String {
    let mut output = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    while output.contains("__") {
        output = output.replace("__", "_");
    }
    output.trim_matches('_').to_owned()
}

fn safe_script_json(value: &str) -> String {
    value
        .replace('&', "\\u0026")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
}

#[allow(clippy::cast_possible_truncation)]
fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = u32::from(chunk[0]);
        let second = chunk.get(1).map_or(0, |value| u32::from(*value));
        let third = chunk.get(2).map_or(0, |value| u32::from(*value));
        let packed = (first << 16) | (second << 8) | third;
        output.push(char::from(TABLE[((packed >> 18) & 63) as usize]));
        output.push(char::from(TABLE[((packed >> 12) & 63) as usize]));
        output.push(if chunk.len() > 1 {
            char::from(TABLE[((packed >> 6) & 63) as usize])
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            char::from(TABLE[(packed & 63) as usize])
        } else {
            '='
        });
    }
    output
}

fn render_html(data: &str, compact: bool) -> String {
    HTML_TEMPLATE
        .replace("__XVARNA_DATA__", data)
        .replace("__XVARNA_MODE__", if compact { "viewer" } else { "report" })
        .replace(
            "__XVARNA_TITLE__",
            if compact {
                "XVARNA Study Viewer"
            } else {
                "XVARNA Study Report"
            },
        )
}

const HTML_TEMPLATE: &str = r##"<!doctype html>
<html lang="en" data-mode="__XVARNA_MODE__"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>__XVARNA_TITLE__</title><style>
:root{color-scheme:dark;--bg:#07111c;--panel:#0d1b2a;--line:#21364a;--ink:#e8f0f7;--muted:#92a7b8;--cyan:#38bdf8;--green:#22c55e;--red:#ef4444;--gold:#f59e0b}*{box-sizing:border-box}body{margin:0;background:radial-gradient(circle at 10% -10%,#17314b 0,transparent 35%),var(--bg);color:var(--ink);font:14px/1.5 Inter,Segoe UI,sans-serif}header,main{max-width:1380px;margin:auto;padding:24px}header{display:flex;justify-content:space-between;gap:20px;align-items:end;border-bottom:1px solid var(--line)}h1{margin:0;font-size:clamp(27px,4vw,48px);letter-spacing:-.04em}.eyebrow{color:var(--cyan);font-weight:700;letter-spacing:.16em;text-transform:uppercase}.muted{color:var(--muted)}.cards{display:grid;grid-template-columns:repeat(auto-fit,minmax(165px,1fr));gap:12px;margin:20px 0}.card,.panel{background:linear-gradient(145deg,rgba(18,38,57,.94),rgba(9,23,36,.94));border:1px solid var(--line);border-radius:14px;box-shadow:0 16px 42px #0005}.card{padding:16px}.card b{display:block;font-size:25px}.grid{display:grid;grid-template-columns:minmax(0,1.55fr) minmax(300px,.75fr);gap:16px}.panel{padding:18px;margin-bottom:16px}h2{margin:0 0 12px;font-size:18px}select,button{background:#10273a;color:var(--ink);border:1px solid #31506a;border-radius:7px;padding:7px 10px}svg{width:100%;min-height:420px;background:#071522;border-radius:9px}.axis{stroke:#49677d;stroke-width:1}.dot{cursor:pointer;stroke:#07111c;stroke-width:1.5;transition:r .15s}.dot:hover{r:8}table{width:100%;border-collapse:collapse;font-size:12px}th,td{text-align:left;padding:8px;border-bottom:1px solid #1c3347;white-space:nowrap}th{position:sticky;top:0;background:#0d1b2a;color:#a9c0d1}.table-wrap{max-height:430px;overflow:auto}.legend{display:flex;gap:14px;flex-wrap:wrap}.legend i{display:inline-block;width:9px;height:9px;border-radius:50%;margin-right:5px}.bar{height:8px;background:#193246;border-radius:5px;overflow:hidden}.bar span{display:block;height:100%;background:linear-gradient(90deg,var(--red),var(--gold),var(--green))}.links a{color:var(--cyan);margin-right:16px}footer{max-width:1380px;margin:auto;padding:10px 24px 40px;color:var(--muted)}[data-mode=viewer] .report-only{display:none}@media(max-width:900px){.grid{grid-template-columns:1fr}header{align-items:start;flex-direction:column}svg{min-height:330px}}
</style></head><body><header><div><div class="eyebrow">VAHMAN · XVARNA 0.16</div><h1 id="title"></h1><div id="description" class="muted"></div></div><div class="links"><a href="study-data.json">JSON</a><a href="variants.parquet">Parquet</a><a href="objective-space.gltf">glTF</a></div></header><main>
<section id="cards" class="cards"></section><section class="grid"><div><article class="panel"><h2>Objective space</h2><div><label>X <select id="xAxis"></select></label> &nbsp; <label>Y <select id="yAxis"></select></label></div><svg id="plot" viewBox="0 0 900 460" role="img" aria-label="Interactive Pareto objective scatter plot"></svg><div class="legend"><span><i style="background:#22c55e"></i>Pareto</span><span><i style="background:#38bdf8"></i>Feasible</span><span><i style="background:#ef4444"></i>Infeasible</span></div></article><article class="panel"><h2>Variants</h2><div class="table-wrap"><table id="variants"></table></div></article></div><aside><article class="panel"><h2>Statistical sensitivity</h2><div class="muted">Spearman ρ · bootstrap interval · permutation test · Holm correction</div><div id="sensitivity"></div></article><article class="panel"><h2>Scenario deltas</h2><div id="deltas"></div></article></aside></section>
<section class="panel report-only"><h2>Reproducibility and scope</h2><p id="scope"></p><p>The report embeds the complete manifest, ledger, ranking, uncertainty, comparison data and hashes. It has no CDN, font, analytics, network, or server dependency. Correlation indicates monotonic association, not causation; confidence and significance require a representative sample design.</p></section></main><footer id="footer"></footer>
<script>const D=__XVARNA_DATA__;const $=s=>document.querySelector(s),E=(t,c)=>{const e=document.createElement(t);if(c)e.className=c;return e};const esc=s=>String(s??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const flat=e=>e.objectiveValues.flat(),ranks=new Map(D.analysis.rankedVariants.map(r=>[r.variantId,r])),vars=new Map(D.ledger.variants.map(v=>[v.variantId,v]));$('#title').textContent=D.manifest.title;$('#description').textContent=D.manifest.description||D.manifest.studyId;const feasible=D.analysis.rankedVariants.filter(r=>r.feasible).length;
[['Variants',D.ledger.variants.length],['Evaluated',D.ledger.evaluations.length],['Feasible',feasible],['Pareto',D.analysis.paretoVariantIds.length],['Fronts',D.analysis.frontCount],['Revision',D.ledger.revision]].forEach(([k,v])=>{const c=E('div','card');c.innerHTML='<span class="muted">'+k+'</span><b>'+v+'</b>';$('#cards').append(c)});
D.objectiveColumns.forEach((n,i)=>{$('#xAxis').add(new Option(n,i));$('#yAxis').add(new Option(n,i))});$('#yAxis').selectedIndex=Math.min(1,D.objectiveColumns.length-1);function plot(){const xi=+$('#xAxis').value,yi=+$('#yAxis').value,rows=D.ledger.evaluations.map(e=>({e,v:flat(e)})),xs=rows.map(r=>r.v[xi]),ys=rows.map(r=>r.v[yi]),minx=Math.min(...xs),maxx=Math.max(...xs),miny=Math.min(...ys),maxy=Math.max(...ys),sx=x=>70+(x-minx)/(maxx-minx||1)*780,sy=y=>400-(y-miny)/(maxy-miny||1)*340,s=$('#plot');s.innerHTML='<line class="axis" x1="70" y1="400" x2="850" y2="400"/><line class="axis" x1="70" y1="60" x2="70" y2="400"/><text fill="#92a7b8" x="70" y="435">'+esc(D.objectiveColumns[xi])+'</text><text fill="#92a7b8" x="75" y="48">'+esc(D.objectiveColumns[yi])+'</text>';rows.forEach(o=>{const r=ranks.get(o.e.variantId),c=!r.feasible?'#ef4444':r.paretoRank===0?'#22c55e':'#38bdf8',n=document.createElementNS('http://www.w3.org/2000/svg','circle');n.setAttribute('class','dot');n.setAttribute('cx',sx(o.v[xi]));n.setAttribute('cy',sy(o.v[yi]));n.setAttribute('r',r.paretoRank===0?6:4.5);n.setAttribute('fill',c);const t=document.createElementNS('http://www.w3.org/2000/svg','title');t.textContent='Variant '+o.e.variantId+' · '+o.v[xi].toPrecision(5)+' · '+o.v[yi].toPrecision(5)+' · rank '+r.paretoRank;n.append(t);s.append(n)})}$('#xAxis').onchange=plot;$('#yAxis').onchange=plot;plot();
let head='<tr><th>ID</th><th>Label</th><th>Generation</th><th>Rank</th><th>Feasible</th>'+D.manifest.parameters.map(p=>'<th>'+esc(p.name)+'</th>').join('')+D.objectiveColumns.map(n=>'<th>'+esc(n)+'</th>').join('')+'</tr>';let body=D.ledger.evaluations.map(e=>{const v=vars.get(e.variantId),r=ranks.get(e.variantId);return '<tr><td>'+e.variantId+'</td><td>'+esc(v.label)+'</td><td>'+v.generation+'</td><td>'+r.paretoRank+'</td><td>'+(r.feasible?'yes':'no')+'</td>'+v.parameters.map(x=>'<td>'+Number(x).toPrecision(6)+'</td>').join('')+flat(e).map(x=>'<td>'+Number(x).toPrecision(6)+'</td>').join('')+'</tr>'}).join('');$('#variants').innerHTML=head+body;
$('#sensitivity').innerHTML=D.analysis.sensitivity.slice().sort((a,b)=>Math.abs(b.spearmanRho)-Math.abs(a.spearmanRho)).slice(0,24).map(s=>'<div style="margin:12px 0"><b>P'+(s.parameterIndex+1)+' → '+esc(s.objectiveName)+'</b><div class="muted">ρ '+s.spearmanRho.toFixed(3)+' · CI ['+s.confidenceLower.toFixed(3)+', '+s.confidenceUpper.toFixed(3)+'] · pHolm '+s.holmAdjustedPValue.toPrecision(3)+'</div><div class="bar"><span style="width:'+Math.abs(s.spearmanRho)*100+'%"></span></div></div>').join('')||'<p class="muted">No sensitivity rows.</p>';
$('#deltas').innerHTML=D.analysis.comparisons.map(c=>'<div style="margin:10px 0"><b>'+c.baselineVariantId+' → '+c.variantId+'</b><div class="muted">Objective Δ: '+c.objectiveDelta.flat().map(x=>Number(x).toPrecision(4)).join(', ')+'</div>'+c.deltaMaps.map(m=>'<div>'+esc(m.fieldId)+' mean '+m.mean.toPrecision(4)+' '+esc(m.unit)+' · RMS '+m.rootMeanSquare.toPrecision(4)+'</div>').join('')+'</div>').join('')||'<p class="muted">No baseline selected.</p>';$('#scope').textContent='Study '+D.manifest.studyId+' · engine '+D.manifest.engineVersion+' · analysis '+D.analysis.contentHash+' · ledger '+D.ledger.contentHash;$('#footer').textContent='Generated by '+D.generatedBy+' · schema '+D.schemaVersion+' · self-contained viewer';</script></body></html>"##;
