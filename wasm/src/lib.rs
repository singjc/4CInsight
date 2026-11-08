use serde::Serialize;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use wasm_bindgen::prelude::*;

// Host-side unit tests compile shared longitudinal.rs outside wasm32. The
// production wasm32 build excludes native orchestration, but the host parser
// still needs the small selection shape referenced by that excluded workflow.
#[cfg(not(target_arch = "wasm32"))]
mod analysis {
    #[allow(dead_code)]
    pub struct CohortModalitySelection {
        pub gene: bool,
        pub transcript: bool,
        pub bcr: bool,
        pub bcr_isotype: bool,
        pub tcr_alpha: bool,
        pub tcr_beta: bool,
        pub immune: bool,
        pub pathway: bool,
    }
}

#[allow(dead_code)]
#[path = "../../src/cohort.rs"]
mod cohort;
#[allow(dead_code)]
#[path = "../../src/corrected.rs"]
mod corrected;
#[allow(dead_code)]
#[path = "../../src/hit_structure_core.rs"]
mod hit_structure_core;
#[path = "../../src/input_conventions.rs"]
mod input_conventions;
#[allow(dead_code)]
#[path = "../../src/integrative_distance.rs"]
mod integrative_distance;
#[allow(dead_code)]
#[path = "../../src/joint_structure_reference.rs"]
mod joint_structure_reference;
#[path = "../../src/longitudinal.rs"]
mod longitudinal;
#[allow(dead_code)]
#[path = "../../src/metadata.rs"]
mod metadata;
#[path = "../../src/portable_compare.rs"]
mod portable_compare;
#[path = "../../src/portable_hit_structure.rs"]
mod portable_hit_structure;
#[allow(dead_code)]
#[path = "../../src/portable_io.rs"]
mod portable_io;
#[path = "../../src/portable_longitudinal.rs"]
mod portable_longitudinal;
#[path = "../../src/portable_merge.rs"]
mod portable_merge;

use hit_structure_core::SignificantHitStructureConfig;
use input_conventions::{FilenameInputKind, classify_input_filename};
use portable_compare::{PortableExpressionComparisonConfig, run_portable_expression_comparison};
use portable_hit_structure::run_portable_expression_hit_structure;
use portable_io::{
    PortableExpressionKind, PortableExpressionMetric, inspect_metadata_bytes,
    metadata_column_values_bytes, parse_metadata_bytes, summarize_expression_bytes,
};
use portable_longitudinal::{PortableLongitudinalConfig, run_portable_expression_longitudinal};

#[derive(Serialize)]
struct EngineInfo<'a> {
    product: &'a str,
    package: &'a str,
    version: &'a str,
    execution: &'a str,
    capabilities: [&'a str; 12],
}

#[derive(Serialize)]
struct FilenameClassification<'a> {
    recognized: bool,
    kind: Option<&'a str>,
    label: Option<&'a str>,
    #[serde(rename = "sampleId")]
    sample_id: Option<String>,
}

fn kind_slug(kind: FilenameInputKind) -> &'static str {
    match kind {
        FilenameInputKind::GeneExpression => "gene-expression",
        FilenameInputKind::TranscriptExpression => "transcript-expression",
        FilenameInputKind::Bcr => "bcr",
        FilenameInputKind::BcrIsotype => "bcr-isotype",
        FilenameInputKind::TcrAlpha => "tcr-alpha",
        FilenameInputKind::TcrBeta => "tcr-beta",
        FilenameInputKind::ImmuneInfiltrate => "immune-infiltrate",
        FilenameInputKind::MergedGeneExpression => "merged-gene-expression",
        FilenameInputKind::MergedTranscriptExpression => "merged-transcript-expression",
        FilenameInputKind::MetadataCandidate => "metadata-candidate",
    }
}

/// I2A: deterministic, label-free multi-view distance fusion.
#[wasm_bindgen]
pub fn fuse_integrative_distances_json(input_json: &str) -> Result<String, JsValue> {
    integrative_distance::fuse_distances_json(input_json).map_err(js_error)
}

/// Independent Rust reference for joint PCoA and hierarchical clustering.
/// Used to test numerical agreement with the browser worker. No labels enter.
#[wasm_bindgen]
pub fn reference_joint_structure_json(input_json: &str) -> Result<String, JsValue> {
    joint_structure_reference::reference_joint_structure_json(input_json).map_err(js_error)
}

// I2B: a separate unsupervised byte session for each modality. It reuses the
// native W4 kernels and does not access case/control labels or adjusted tests.
struct I2bSession {
    modality: cohort::CohortModality,
    spec: corrected::CorrectedModelSpec,
    repertoire: Option<corrected::PortableRepertoireAccumulator>,
    maps: Vec<(String, HashMap<String, f64>)>,
    source_ids: HashSet<String>,
    feature_samples: Vec<serde_json::Value>,
}
thread_local! {
    static I2B_SESSION: RefCell<Option<I2bSession>> = const { RefCell::new(None) };
}
fn i2b_kind_name(modality: cohort::CohortModality) -> &'static str {
    match modality {
        cohort::CohortModality::Bcr => "bcr",
        cohort::CohortModality::TcrAlpha => "tcrAlpha",
        cohort::CohortModality::TcrBeta => "tcrBeta",
        cohort::CohortModality::BcrIsotype => "isotype",
        cohort::CohortModality::ImmuneInfiltrate => "immune",
        _ => "unknown",
    }
}
fn i2b_output(
    modality: &str,
    raw: corrected::UnsupervisedDistanceMatrix,
    feature_samples: Vec<serde_json::Value>,
) -> Result<String, JsValue> {
    if raw.sample_ids.len() < 3
        || raw.distances.len() != raw.sample_ids.len()
        || raw
            .distances
            .iter()
            .any(|r| r.len() != raw.sample_ids.len())
    {
        return Err(js_error(
            "I2B needs at least three samples and a square distance matrix",
        ));
    }
    let mut pairs = Vec::new();
    for i in 0..raw.sample_ids.len() {
        for j in (i + 1)..raw.sample_ids.len() {
            let distance = raw.distances[i][j];
            if !distance.is_finite() || distance < 0.0 {
                return Err(js_error("I2B produced an invalid unsupervised distance"));
            }
            pairs.push(serde_json::json!({"a":raw.sample_ids[i],"b":raw.sample_ids[j],"distance":distance}));
        }
    }
    serde_json::to_string(&serde_json::json!({
        "modality":modality,"distance_label":raw.distance_label,
        "pairs":pairs,"samples":raw.sample_ids.len(),"warnings":raw.warnings,
        "source":"direct-local-workbooks", "model":"unsupervised-native-w4-distance-kernel",
        "feature_samples":feature_samples,
    }))
    .map_err(js_error)
}

/// Begin one I2B modality. Settings match the native W4 distance methods.
#[wasm_bindgen]
pub fn i2b_begin_json(modality: &str, options: &str) -> Result<String, JsValue> {
    install_m2_repertoire_panic_hook();
    let kind = match modality {
        "bcr" => cohort::CohortModality::Bcr,
        "tcrAlpha" => cohort::CohortModality::TcrAlpha,
        "tcrBeta" => cohort::CohortModality::TcrBeta,
        "isotype" => cohort::CohortModality::BcrIsotype,
        "immune" => cohort::CohortModality::ImmuneInfiltrate,
        _ => {
            return Err(js_error(
                "I2B supports BCR, TCR alpha, TCR beta, isotypes and immune",
            ));
        }
    };
    let options: serde_json::Value = serde_json::from_str(options).map_err(js_error)?;
    let mut spec = corrected::CorrectedModelSpec::default();
    if let Some(v) = options.get("ambiguousPolicy").and_then(|v| v.as_str()) {
        spec.ambiguous_policy = match v {
            "exclude" => corrected::AmbiguousCallPolicy::Exclude,
            "split" => corrected::AmbiguousCallPolicy::Split,
            "first" => corrected::AmbiguousCallPolicy::First,
            "family" => corrected::AmbiguousCallPolicy::Family,
            _ => return Err(js_error("Unknown ambiguity policy")),
        };
    }
    if let Some(v) = options.get("determinedOnly") {
        spec.exclude_undetermined_isotype = v
            .as_bool()
            .ok_or_else(|| js_error("determinedOnly must be a boolean"))?;
    }
    if let Some(v) = options.get("clrPseudocount") {
        spec.clr_pseudocount = v
            .as_f64()
            .filter(|x| x.is_finite() && *x > 0.0)
            .ok_or_else(|| js_error("Invalid clrPseudocount"))?;
    }
    if let Some(v) = options.get("compositionMinMean") {
        spec.composition_min_mean = v
            .as_f64()
            .filter(|x| x.is_finite() && *x >= 0.0)
            .ok_or_else(|| js_error("Invalid compositionMinMean"))?;
    }
    if let Some(v) = options.get("compositionMinPrevalence") {
        spec.composition_min_prevalence = v
            .as_f64()
            .filter(|x| x.is_finite() && (0.0..=1.0).contains(x))
            .ok_or_else(|| js_error("Invalid compositionMinPrevalence"))?;
    }
    let repertoire = if matches!(
        kind,
        cohort::CohortModality::Bcr
            | cohort::CohortModality::TcrAlpha
            | cohort::CohortModality::TcrBeta
    ) {
        Some(
            corrected::PortableRepertoireAccumulator::new(kind, spec.ambiguous_policy)
                .map_err(js_error)?,
        )
    } else {
        None
    };
    I2B_SESSION.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_some() {
            return Err(js_error("I2B session already active"));
        }
        *slot = Some(I2bSession {
            modality: kind,
            spec,
            repertoire,
            maps: Vec::new(),
            source_ids: HashSet::new(),
            feature_samples: Vec::new(),
        });
        Ok("{\"status\":\"ready\"}".to_string())
    })
}

/// Decode only one local sample workbook. The RefCell is not borrowed while
/// the XLSX parser runs, so a recoverable parsing error cannot poison the session.
#[wasm_bindgen]
pub fn i2b_add_sample_json(
    file_name: &str,
    sample_id: &str,
    bytes: &[u8],
) -> Result<String, JsValue> {
    let mut session = I2B_SESSION
        .with(|cell| cell.borrow_mut().take())
        .ok_or_else(|| js_error("No I2B session is active"))?;
    let outcome = (|| -> Result<String, JsValue> {
        if sample_id.is_empty()
            || sample_id.trim() != sample_id
            || bytes.is_empty()
            || file_name.is_empty()
        {
            return Err(js_error(
                "I2B needs a nonempty exact sample ID and one local source file",
            ));
        }
        if session.source_ids.contains(sample_id) {
            return Err(js_error(format!("Duplicate I2B source ID: {sample_id}")));
        }
        let (features, complete_families): (Vec<serde_json::Value>, Vec<&str>) =
            if let Some(repertoire) = session.repertoire.as_mut() {
                repertoire
                    .add_bytes(file_name, bytes, sample_id, "Eligible")
                    .map_err(js_error)?;
                let native = repertoire.last_sample_feature_values().map_err(js_error)?;
                // Mark a composition family complete only if this source actually
                // measured that family. Never turn a missing column into zeros.
                let families = ["V gene fraction", "D gene fraction", "J gene fraction", "Isotype fraction"]
                    .into_iter()
                    .filter(|family| native.iter().any(|(label, _, _, _)| label.as_str() == *family))
                    .collect::<Vec<_>>();
                (native.into_iter().map(|(family, name, value, unit)|
                    serde_json::json!({"family":family,"name":name,"value":value,"unit":unit})
                ).collect(), families)
            } else if session.modality == cohort::CohortModality::BcrIsotype {
                let map = corrected::parse_isotype_bytes(bytes, session.spec.exclude_undetermined_isotype)
                    .map_err(js_error)?;
                let total: f64 = map.values().filter(|v| v.is_finite() && **v > 0.0).sum();
                let values = if total > 0.0 { map.iter().filter_map(|(name, value)| {
                    if value.is_finite() && *value >= 0.0 {
                        Some(serde_json::json!({"family":"Isotype fraction","name":name,"value":value / total,"unit":"fraction"}))
                    } else { None }
                }).collect() } else { Vec::new() };
                session.maps.push((sample_id.to_string(), map));
                (values, vec!["Isotype fraction"])
            } else {
                let map = corrected::parse_immune_bytes(bytes).map_err(js_error)?;
                let values = map.iter().filter_map(|(name, value)| {
                    if value.is_finite() {
                        Some(serde_json::json!({"family":"Immune signature score","name":name,"value":value,"unit":"native score"}))
                    } else { None }
                }).collect();
                session.maps.push((sample_id.to_string(), map));
                (values, Vec::new())
            };
        session.feature_samples.push(serde_json::json!({
            "sample_id":sample_id,"features":features,"complete_families":complete_families,
            "source":"native-corrected-parser"
        }));
        session.source_ids.insert(sample_id.to_string());
        serde_json::to_string(
            &serde_json::json!({"status":"added","samples":session.source_ids.len()}),
        )
        .map_err(js_error)
    })();
    I2B_SESSION.with(|cell| {
        let mut slot = cell.borrow_mut();
        debug_assert!(slot.is_none());
        *slot = Some(session);
    });
    outcome
}

#[wasm_bindgen]
pub fn i2b_finish_json() -> Result<String, JsValue> {
    let session = I2B_SESSION
        .with(|cell| cell.borrow_mut().take())
        .ok_or_else(|| js_error("No I2B session is active"))?;
    let feature_samples = session.feature_samples;
    let raw = match session.modality {
        cohort::CohortModality::Bcr
        | cohort::CohortModality::TcrAlpha
        | cohort::CohortModality::TcrBeta => session
            .repertoire
            .ok_or_else(|| js_error("I2B repertoire collector is missing"))?
            .finish_unsupervised()
            .map_err(js_error)?,
        cohort::CohortModality::BcrIsotype => corrected::unsupervised_isotype_distance_from_maps(
            session.maps,
            &session.spec,
            Vec::new(),
        )
        .map_err(js_error)?,
        cohort::CohortModality::ImmuneInfiltrate => {
            corrected::unsupervised_immune_distance_from_maps(session.maps, Vec::new())
                .map_err(js_error)?
        }
        _ => return Err(js_error("Unsupported I2B modality")),
    };
    i2b_output(i2b_kind_name(session.modality), raw, feature_samples)
}
#[wasm_bindgen]
pub fn i2b_abort() {
    I2B_SESSION.with(|cell| *cell.borrow_mut() = None);
}

/// Gene/transcript distance uses the same two-pass native W4 reader kernel.
/// The browser receives an explicit I1-eligible ID set, never group labels.
#[wasm_bindgen]
pub fn i2b_expression_distance_json(
    modality: &str,
    merged_bytes: &[u8],
    sample_ids_json: &str,
    options_json: &str,
) -> Result<String, JsValue> {
    let ids: Vec<String> = serde_json::from_str(sample_ids_json).map_err(js_error)?;
    let eligible: HashSet<String> = ids
        .iter()
        .map(|id| cohort::normalize_sample_id(id))
        .collect();
    if eligible.len() != ids.len()
        || eligible.len() < 3
        || ids.iter().any(|id| id.is_empty() || id.trim() != id)
    {
        return Err(js_error(
            "I2B requires three distinct, exact, aligned sample IDs",
        ));
    }
    let options: serde_json::Value = serde_json::from_str(options_json).map_err(js_error)?;
    let mut spec = corrected::CorrectedModelSpec::default();
    if let Some(x) = options.get("minCount") {
        spec.min_count = x
            .as_f64()
            .filter(|n| n.is_finite() && *n >= 0.0)
            .ok_or_else(|| js_error("Invalid minimum count"))?;
    }
    if let Some(x) = options.get("minFractionSamples") {
        spec.min_fraction_samples = x
            .as_f64()
            .filter(|n| n.is_finite() && *n >= 0.0 && *n <= 1.0)
            .ok_or_else(|| js_error("Invalid minimum prevalence"))?;
    }
    let (source_metric, feature_label) = match modality {
        "gene" => ("Raw Count", "Gene"),
        "transcript" => ("Total Reads", "Transcript"),
        _ => {
            return Err(js_error(
                "I2B expression distance supports gene and transcript only",
            ));
        }
    };
    let raw = corrected::run_unsupervised_expression_distance_bytes(
        merged_bytes,
        &eligible,
        source_metric,
        feature_label,
        &spec,
        |_, _, _| {},
    )
    .map_err(js_error)?;
    i2b_output(modality, raw, Vec::new())
}

fn js_error(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}

/// A single, explicit multi-omics session stays inside the science Web Worker.
/// File bytes are decoded one workbook at a time and released after parsing.
struct MultiomicsSession {
    modality: cohort::CohortModality,
    prepared: portable_compare::PreparedPortableComparison,
    repertoire: Option<corrected::PortableRepertoireAccumulator>,
    maps: Vec<(String, String, HashMap<String, f64>)>,
    used_ids: HashSet<String>,
    skipped: usize,
}

thread_local! {
    static MULTIOMICS_SESSION: RefCell<Option<MultiomicsSession>> = const { RefCell::new(None) };
}

fn parse_multiomics_kind(value: &str) -> Result<cohort::CohortModality, String> {
    match value {
        "bcr" => Ok(cohort::CohortModality::Bcr),
        "tcr-alpha" => Ok(cohort::CohortModality::TcrAlpha),
        "tcr-beta" => Ok(cohort::CohortModality::TcrBeta),
        "bcr-isotype" => Ok(cohort::CohortModality::BcrIsotype),
        "immune-infiltrate" => Ok(cohort::CohortModality::ImmuneInfiltrate),
        _ => Err(format!("Unsupported multi-omics modality: {value}")),
    }
}

/// Start a corrected modality run with the SAME metadata/group/covariate
/// conventions used by the corrected expression workflow.
// M2/R4b: Record Rust panic details in the browser console.
// This does not read host system information, use the network, or change science.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
unsafe extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = error)]
    fn m2_repertoire_console_error(message: &str);
}

#[cfg(target_arch = "wasm32")]
fn install_m2_repertoire_panic_hook() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        std::panic::set_hook(Box::new(|info| unsafe {
            m2_repertoire_console_error(&format!("4C Insight WASM panic: {info}"));
        }));
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn install_m2_repertoire_panic_hook() {}

#[wasm_bindgen]
pub fn multiomics_begin_json(
    metadata_file_name: &str,
    metadata_bytes: &[u8],
    comparison_config_json: &str,
    modality: &str,
    analysis_options_json: &str,
) -> Result<String, JsValue> {
    install_m2_repertoire_panic_hook();
    let modality = parse_multiomics_kind(modality).map_err(js_error)?;
    let config: portable_compare::PortableExpressionComparisonConfig =
        serde_json::from_str(comparison_config_json).map_err(js_error)?;
    let mut prepared =
        portable_compare::prepare_portable_comparison(metadata_file_name, metadata_bytes, &config)
            .map_err(js_error)?;
    let options: serde_json::Value =
        serde_json::from_str(analysis_options_json).map_err(js_error)?;
    // Bounded configurable options; defaults are the established native ones.
    if let Some(permutations) = options.get("permutations").and_then(|v| v.as_u64()) {
        prepared.spec.permutations = permutations.clamp(1, 9999) as usize;
    }
    if let Some(reps) = options.get("rarefactionReps").and_then(|v| v.as_u64()) {
        prepared.spec.rarefaction_reps = reps.clamp(1, 32) as usize;
    }
    if let Some(cap) = options.get("rarefactionCap").and_then(|v| v.as_u64()) {
        prepared.spec.rarefaction_cap = cap.clamp(100, 1_000_000) as usize;
    }
    if let Some(value) = options.get("determinedOnly").and_then(|v| v.as_bool()) {
        prepared.spec.exclude_undetermined_isotype = value;
    }
    if let Some(policy) = options.get("ambiguousPolicy").and_then(|v| v.as_str()) {
        prepared.spec.ambiguous_policy = match policy {
            "exclude" => corrected::AmbiguousCallPolicy::Exclude,
            "split" => corrected::AmbiguousCallPolicy::Split,
            "first" => corrected::AmbiguousCallPolicy::First,
            "family" => corrected::AmbiguousCallPolicy::Family,
            _ => {
                return Err(js_error(format!(
                    "Invalid V/D/J ambiguity policy: {policy}"
                )));
            }
        };
    }
    let repertoire = if matches!(
        modality,
        cohort::CohortModality::Bcr
            | cohort::CohortModality::TcrAlpha
            | cohort::CohortModality::TcrBeta
    ) {
        Some(
            corrected::PortableRepertoireAccumulator::new(modality, prepared.spec.ambiguous_policy)
                .map_err(js_error)?,
        )
    } else {
        None
    };
    MULTIOMICS_SESSION.with(|cell| {
        let mut cell = cell.borrow_mut();
        if cell.is_some() {
            return Err(js_error("Another multi-omics session is already active"));
        }
        *cell = Some(MultiomicsSession {
            modality,
            prepared,
            repertoire,
            maps: Vec::new(),
            used_ids: HashSet::new(),
            skipped: 0,
        });
        Ok(String::from("{\"status\":\"ready\"}"))
    })
}

/// Check cohort membership before the browser reads large raw workbook bytes.
#[wasm_bindgen]
pub fn multiomics_sample_role_json(sample_id: &str) -> Result<String, JsValue> {
    MULTIOMICS_SESSION.with(|cell| {
        let session = cell.borrow();
        let session = session
            .as_ref()
            .ok_or_else(|| js_error("No active multi-omics session"))?;
        let comparison = &session.prepared.comparison;
        let role = if comparison.is_case_id(sample_id) {
            "case"
        } else if comparison.is_control_id(sample_id) {
            "control"
        } else {
            "outside"
        };
        serde_json::to_string(&serde_json::json!({"role": role})).map_err(js_error)
    })
}

/// Decode one local sample workbook. Never retain the original XLSX bytes.
#[wasm_bindgen]
pub fn multiomics_add_sample_json(
    file_name: &str,
    sample_id: &str,
    bytes: &[u8],
) -> Result<String, JsValue> {
    let mut session = MULTIOMICS_SESSION.with(|cell| {
        cell.borrow_mut()
            .take()
            .ok_or_else(|| js_error("No active multi-omics session"))
    })?;
    // Parsing can call into the JS host. Do not retain a RefCell borrow here.
    let outcome = (|| -> Result<String, JsValue> {
        let key = cohort::normalize_sample_id(sample_id);
        if key.is_empty() {
            return Err(js_error(format!(
                "Missing sample identifier for {file_name}"
            )));
        }
        let comparison = &session.prepared.comparison;
        let group = if comparison.is_case_id(&key) {
            comparison.case_label.clone()
        } else if comparison.is_control_id(&key) {
            comparison.control_label.clone()
        } else {
            session.skipped += 1;
            return Ok(String::from("{\"status\":\"skipped\"}"));
        };
        if session.used_ids.contains(&key) {
            return Err(js_error(format!(
                "Two files map to sample {sample_id}; resolve this ambiguity in Sample Metadata"
            )));
        }
        if let Some(repertoire) = session.repertoire.as_mut() {
            repertoire
                .add_bytes(file_name, bytes, sample_id, &group)
                .map_err(js_error)?;
        } else if session.modality == cohort::CohortModality::BcrIsotype {
            let map = corrected::parse_isotype_bytes(
                bytes,
                session.prepared.spec.exclude_undetermined_isotype,
            )
            .map_err(js_error)?;
            session.maps.push((sample_id.to_string(), group, map));
        } else {
            let map = corrected::parse_immune_bytes(bytes).map_err(js_error)?;
            session.maps.push((sample_id.to_string(), group, map));
        }
        session.used_ids.insert(key);
        serde_json::to_string(&serde_json::json!({
            "status": "added", "count": session.used_ids.len(),
        }))
        .map_err(js_error)
    })();
    // A recoverable parse error must not leave the session inaccessible.
    MULTIOMICS_SESSION.with(|cell| {
        let mut slot = cell.borrow_mut();
        debug_assert!(
            slot.is_none(),
            "multi-omics session re-entered during parsing"
        );
        *slot = Some(session);
    });
    outcome
}
#[wasm_bindgen]
pub fn multiomics_finish_json() -> Result<String, JsValue> {
    let session = MULTIOMICS_SESSION
        .with(|cell| cell.borrow_mut().take())
        .ok_or_else(|| js_error("No active multi-omics session"))?;
    let MultiomicsSession {
        modality,
        prepared,
        repertoire,
        maps,
        skipped,
        ..
    } = session;
    let analysis = match modality {
        cohort::CohortModality::Bcr
        | cohort::CohortModality::TcrAlpha
        | cohort::CohortModality::TcrBeta => {
            let repertoire = repertoire.ok_or_else(|| js_error("Repertoire session is missing"))?;
            serde_json::to_value(
                repertoire
                    .finish(
                        &prepared.metadata,
                        &prepared.comparison,
                        &prepared.spec,
                        |_, _, _| {},
                    )
                    .map_err(js_error)?,
            )
            .map_err(js_error)?
        }
        cohort::CohortModality::BcrIsotype => serde_json::to_value(
            corrected::run_corrected_isotype_from_maps(
                maps,
                &prepared.metadata,
                &prepared.comparison,
                &prepared.spec,
                Vec::new(),
            )
            .map_err(js_error)?,
        )
        .map_err(js_error)?,
        cohort::CohortModality::ImmuneInfiltrate => serde_json::to_value(
            corrected::run_corrected_immune_from_maps(
                maps,
                &prepared.metadata,
                &prepared.comparison,
                &prepared.spec,
                Vec::new(),
            )
            .map_err(js_error)?,
        )
        .map_err(js_error)?,
        _ => return Err(js_error("Unsupported modality")),
    };
    serde_json::to_string(&serde_json::json!({
        "analysis": analysis,
        "comparison": prepared.summary,
        "exclusions": prepared.exclusions,
        "skippedFilesOutsideGroups": skipped,
    }))
    .map_err(js_error)
}

#[wasm_bindgen]
pub fn multiomics_abort() {
    MULTIOMICS_SESSION.with(|cell| *cell.borrow_mut() = None);
}

#[wasm_bindgen]
pub fn engine_info_json() -> String {
    serde_json::to_string(&EngineInfo {
        product: "4C Insight",
        package: env!("CARGO_PKG_NAME"),
        version: env!("CARGO_PKG_VERSION"),
        execution: "local-browser-wasm",
        capabilities: [
            "filename-classification",
            "metadata",
            "expression-qc",
            "metadata-grouping",
            "corrected-expression-comparison",
            "longitudinal-expression-signature",
            "significant-hit-expression-structure",
            "corrected-bcr-repertoire",
            "corrected-tcr-alpha-repertoire",
            "corrected-tcr-beta-repertoire",
            "corrected-isotype-composition",
            "corrected-immune-infiltrate",
        ],
    })
    .expect("engine info serialization is infallible")
}

#[wasm_bindgen]
pub fn classify_filename_json(name: &str) -> String {
    let classification = match classify_input_filename(name) {
        Some(value) => FilenameClassification {
            recognized: true,
            kind: Some(kind_slug(value.kind)),
            label: Some(value.kind.display_name()),
            sample_id: value.sample_id,
        },
        None => FilenameClassification {
            recognized: false,
            kind: None,
            label: None,
            sample_id: None,
        },
    };

    serde_json::to_string(&classification)
        .expect("filename classification serialization is infallible")
}

#[wasm_bindgen]
pub fn inspect_metadata_json(
    file_name: &str,
    bytes: &[u8],
    id_column: &str,
) -> Result<String, JsValue> {
    let selected_id = (!id_column.trim().is_empty()).then_some(id_column);
    let inspection = inspect_metadata_bytes(file_name, bytes, selected_id).map_err(js_error)?;
    serde_json::to_string(&inspection).map_err(js_error)
}

/// Read all metadata rows for the browser editor. The same parser is used by
/// native and WASM science, including XLSX, TSV, and CSV.
#[wasm_bindgen]
pub fn read_metadata_table_json(file_name: &str, bytes: &[u8]) -> Result<String, JsValue> {
    let table = parse_metadata_bytes(file_name, bytes).map_err(js_error)?;
    serde_json::to_string(&table).map_err(js_error)
}

#[wasm_bindgen]
pub fn summarize_expression_json(
    file_name: &str,
    bytes: &[u8],
    requested_kind: &str,
    metric: &str,
    ignore_zeros: bool,
) -> Result<String, JsValue> {
    let requested_kind = PortableExpressionKind::parse(requested_kind).map_err(js_error)?;
    let metric = PortableExpressionMetric::parse(metric).map_err(js_error)?;
    let summary =
        summarize_expression_bytes(file_name, bytes, requested_kind, metric, ignore_zeros)
            .map_err(js_error)?;
    serde_json::to_string(&summary).map_err(js_error)
}

#[wasm_bindgen]
pub fn metadata_column_values_json(
    file_name: &str,
    bytes: &[u8],
    column: &str,
) -> Result<String, JsValue> {
    let values = metadata_column_values_bytes(file_name, bytes, column).map_err(js_error)?;
    serde_json::to_string(&values).map_err(js_error)
}

fn run_expression_comparison_json_core(
    metadata_file_name: &str,
    metadata_bytes: &[u8],
    merged_file_name: &str,
    merged_bytes: &[u8],
    config_json: &str,
) -> Result<String, String> {
    let config: PortableExpressionComparisonConfig =
        serde_json::from_str(config_json).map_err(|error| error.to_string())?;
    let result = run_portable_expression_comparison(
        metadata_file_name,
        metadata_bytes,
        merged_file_name,
        merged_bytes,
        &config,
        |_done, _total, _message| {},
    )
    .map_err(|error| error.to_string())?;
    serde_json::to_string(&result).map_err(|error| error.to_string())
}

#[wasm_bindgen]
pub fn run_expression_comparison_json(
    metadata_file_name: &str,
    metadata_bytes: &[u8],
    merged_file_name: &str,
    merged_bytes: &[u8],
    config_json: &str,
) -> Result<String, JsValue> {
    run_expression_comparison_json_core(
        metadata_file_name,
        metadata_bytes,
        merged_file_name,
        merged_bytes,
        config_json,
    )
    .map_err(js_error)
}

fn run_expression_longitudinal_json_core(
    metadata_file_name: &str,
    metadata_bytes: &[u8],
    merged_file_name: &str,
    merged_bytes: &[u8],
    config_json: &str,
) -> Result<String, String> {
    let config: PortableLongitudinalConfig =
        serde_json::from_str(config_json).map_err(|error| error.to_string())?;
    let result = run_portable_expression_longitudinal(
        metadata_file_name,
        metadata_bytes,
        merged_file_name,
        merged_bytes,
        &config,
        |_done, _total, _message| {},
    )
    .map_err(|error| error.to_string())?;
    serde_json::to_string(&result).map_err(|error| error.to_string())
}

#[wasm_bindgen]
pub fn run_expression_longitudinal_json(
    metadata_file_name: &str,
    metadata_bytes: &[u8],
    merged_file_name: &str,
    merged_bytes: &[u8],
    config_json: &str,
) -> Result<String, JsValue> {
    run_expression_longitudinal_json_core(
        metadata_file_name,
        metadata_bytes,
        merged_file_name,
        merged_bytes,
        config_json,
    )
    .map_err(js_error)
}

fn run_expression_hit_structure_json_core(
    metadata_file_name: &str,
    metadata_bytes: &[u8],
    merged_file_name: &str,
    merged_bytes: &[u8],
    comparison_config_json: &str,
    hit_config_json: &str,
) -> Result<String, String> {
    let comparison_config: PortableExpressionComparisonConfig =
        serde_json::from_str(comparison_config_json).map_err(|error| error.to_string())?;
    let hit_config: SignificantHitStructureConfig =
        serde_json::from_str(hit_config_json).map_err(|error| error.to_string())?;
    let result = run_portable_expression_hit_structure(
        metadata_file_name,
        metadata_bytes,
        merged_file_name,
        merged_bytes,
        &comparison_config,
        hit_config,
        |_done, _total, _message| {},
    )
    .map_err(|error| error.to_string())?;
    serde_json::to_string(&result).map_err(|error| error.to_string())
}

#[wasm_bindgen]
pub fn run_expression_hit_structure_json(
    metadata_file_name: &str,
    metadata_bytes: &[u8],
    merged_file_name: &str,
    merged_bytes: &[u8],
    comparison_config_json: &str,
    hit_config_json: &str,
) -> Result<String, JsValue> {
    run_expression_hit_structure_json_core(
        metadata_file_name,
        metadata_bytes,
        merged_file_name,
        merged_bytes,
        comparison_config_json,
        hit_config_json,
    )
    .map_err(js_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wasm_bridge_reuses_shared_filename_conventions() {
        let json = classify_filename_json("4C_465_tumor_rna_gene_expression_report.xlsx");
        assert!(json.contains("gene-expression"));
        assert!(json.contains("4C_465"));
    }

    #[test]
    fn unrelated_files_remain_unrecognized() {
        let json = classify_filename_json("notes.pdf");
        assert!(json.contains("\"recognized\":false"));
    }

    #[test]
    fn metadata_fixture_is_parsed_by_wasm_bridge() {
        let bytes = include_bytes!("../../tests/fixtures/w2_metadata.tsv");
        let json = inspect_metadata_json("w2_metadata.tsv", bytes, "sample_id").unwrap();
        assert!(json.contains("\"rows\":3"));
        assert!(json.contains("\"id_column\":\"sample_id\""));
    }

    #[test]
    fn full_metadata_table_preserves_all_rows() {
        let bytes = include_bytes!("../../tests/fixtures/w2_metadata.tsv");
        let json = read_metadata_table_json("w2_metadata.tsv", bytes).unwrap();
        let table: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(table["columns"][0], "sample_id");
        assert_eq!(table["rows"].as_array().unwrap().len(), 3);
        assert_eq!(table["id_column"], "sample_id");
    }

    #[test]
    fn full_metadata_table_reads_xlsx_fixture() {
        let bytes = include_bytes!("../../tests/fixtures/w2_metadata.xlsx");
        let json = read_metadata_table_json("w2_metadata.xlsx", bytes).unwrap();
        let table: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(table["rows"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn gene_expression_fixture_is_summarized_by_wasm_bridge() {
        let bytes = include_bytes!("../../tests/fixtures/S1_tumor_rna_gene_expression_report.xlsx");
        let json = summarize_expression_json(
            "S1_tumor_rna_gene_expression_report.xlsx",
            bytes,
            "gene-expression",
            "tpm",
            false,
        )
        .unwrap();
        assert!(json.contains("\"sample\":\"S1\""));
        assert!(json.contains("\"median\":3.0"));
    }

    #[test]
    fn w3_gene_comparison_runs_through_wasm_bridge() {
        let metadata = include_bytes!("../../tests/fixtures/w3_metadata.tsv");
        let merged = include_bytes!("../../tests/fixtures/w3_merged_gene_expression.tsv");
        let config = serde_json::json!({
            "kind": "gene-expression",
            "metadataIdColumn": "sample_id",
            "groupColumn": "analysis_group",
            "caseValues": ["Case"],
            "controlValues": ["Control"],
            "comparisonName": "Case vs Control",
            "caseLabel": "Case",
            "controlLabel": "Control",
            "subjectColumn": "patient_id",
            "covariates": ["batch"],
            "exclusions": [{
                "enabled": true,
                "label": "Reviewed QC",
                "column": "qc_review_status",
                "values": ["Failed"]
            }],
            "minCount": 10.0,
            "minFractionSamples": 0.20
        });
        let json = run_expression_comparison_json_core(
            "w3_metadata.tsv",
            metadata,
            "w3_merged_gene_expression.tsv",
            merged,
            &config.to_string(),
        )
        .unwrap();
        assert!(json.contains("\"excluded_rows\":1"));
        assert!(json.contains("\"n_model_samples\":7"));
        assert!(json.contains("\"feature_label\":\"Gene\""));
    }

    #[test]
    fn w4_longitudinal_gene_signature_runs_through_wasm_bridge() {
        let metadata = include_bytes!("../../tests/fixtures/w4_metadata.tsv");
        let merged = include_bytes!("../../tests/fixtures/w3_merged_gene_expression.tsv");
        let config = serde_json::json!({
            "kind": "gene-expression",
            "observationIdColumn": "sample_id",
            "patientIdColumn": "patient_id",
            "timeColumn": "visit",
            "exclusionEnabled": true,
            "exclusionColumn": "qc_review_status",
            "exclusionValues": ["Failed"],
            "minCount": 10.0,
            "minFractionSamples": 0.20
        });
        let json = run_expression_longitudinal_json_core(
            "w4_metadata.tsv",
            metadata,
            "w3_merged_gene_expression.tsv",
            merged,
            &config.to_string(),
        )
        .unwrap();
        assert!(json.contains("\"qc_excluded_rows\":1"));
        assert!(json.contains("\"longitudinal_patients\":3"));
        assert!(json.contains("\"samples\":7"));
    }

    #[test]
    fn w5_gene_significant_hit_structure_runs_through_wasm_bridge() {
        let metadata = include_bytes!("../../tests/fixtures/w3_metadata.tsv");
        let merged = include_bytes!("../../tests/fixtures/w3_merged_gene_expression.tsv");
        let comparison = serde_json::json!({
            "kind": "gene-expression",
            "metadataIdColumn": "sample_id",
            "groupColumn": "analysis_group",
            "caseValues": ["Case"],
            "controlValues": ["Control"],
            "comparisonName": "Case vs Control",
            "caseLabel": "Case",
            "controlLabel": "Control",
            "subjectColumn": "patient_id",
            "covariates": ["batch"],
            "exclusions": [{
                "enabled": true,
                "label": "Reviewed QC",
                "column": "qc_review_status",
                "values": ["Failed"]
            }],
            "minCount": 10.0,
            "minFractionSamples": 0.20,
            "permutations": 49
        });
        let hits = serde_json::json!({
            "qThreshold": 1.0,
            "minAbsEffect": 0.0,
            "maxAnalysisFeatures": 50,
            "maxHeatmapFeatures": 20
        });
        let json = run_expression_hit_structure_json_core(
            "w3_metadata.tsv",
            metadata,
            "w3_merged_gene_expression.tsv",
            merged,
            &comparison.to_string(),
            &hits.to_string(),
        )
        .unwrap();
        assert!(json.contains("\"significant_features_total\""));
        assert!(json.contains("\"heatmap_zscores\""));
        assert!(json.contains("\"silhouette\""));
        assert!(json.contains("\"nearest_neighbor_group_accuracy\""));
    }
}

// M1: Portable XLSX merger. One workbook is read per call. The JS worker owns
// browser-local scratch, so neither Rust nor WASM retains the cohort matrix.
use portable_merge::{PortableMergeKind, PortableMergeMetric, PortableMergeOrder, PortableMerger};
thread_local! {
    static MERGE_SESSION: RefCell<Option<PortableMerger>> = const { RefCell::new(None) };
    static LAST_MERGE_SAMPLE: RefCell<Option<String>> = const { RefCell::new(None) };
}

#[wasm_bindgen]
pub fn merge_begin_json(
    first_bytes: &[u8],
    kind_json: &str,
    metrics_json: &str,
    order_json: &str,
    names_json: &str,
) -> Result<String, JsValue> {
    let kind: PortableMergeKind = serde_json::from_str(kind_json).map_err(js_error)?;
    let metrics: Vec<PortableMergeMetric> = serde_json::from_str(metrics_json).map_err(js_error)?;
    let order: PortableMergeOrder = serde_json::from_str(order_json).map_err(js_error)?;
    let names: Vec<String> = serde_json::from_str(names_json).map_err(js_error)?;
    let session =
        PortableMerger::begin(first_bytes, kind, metrics, order, &names).map_err(js_error)?;
    let plan = serde_json::to_string(&session.plan()).map_err(js_error)?;
    MERGE_SESSION.with(|cell| *cell.borrow_mut() = Some(session));
    Ok(plan)
}

#[wasm_bindgen]
pub fn merge_sample_packed(bytes: &[u8]) -> Result<Vec<u8>, JsValue> {
    MERGE_SESSION.with(|cell| {
        let borrow = cell.borrow();
        let session = borrow
            .as_ref()
            .ok_or_else(|| js_error("merge session not started"))?;
        let (packed, report) = session.sample(bytes).map_err(js_error)?;
        let json = serde_json::to_string(&report).map_err(js_error)?;
        LAST_MERGE_SAMPLE.with(|last| *last.borrow_mut() = Some(json));
        Ok(packed)
    })
}

#[wasm_bindgen]
pub fn merge_last_sample_json() -> String {
    LAST_MERGE_SAMPLE.with(|cell| cell.borrow().clone().unwrap_or_else(|| "{}".to_string()))
}

#[wasm_bindgen]
pub fn merge_header_tsv() -> Result<String, JsValue> {
    MERGE_SESSION.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(PortableMerger::header)
            .ok_or_else(|| js_error("merge session not started"))
    })
}

#[wasm_bindgen]
pub fn merge_render_rows_tsv(start: usize, count: usize, packed: &[u8]) -> Result<String, JsValue> {
    MERGE_SESSION.with(|cell| {
        cell.borrow()
            .as_ref()
            .ok_or_else(|| js_error("merge session not started"))?
            .rows_tsv(start, count, packed)
            .map_err(js_error)
    })
}

#[wasm_bindgen]
pub fn merge_reset() {
    MERGE_SESSION.with(|cell| *cell.borrow_mut() = None);
    LAST_MERGE_SAMPLE.with(|cell| *cell.borrow_mut() = None);
}
