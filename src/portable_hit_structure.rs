//! Browser-safe significant-hit structure orchestration for expression data.
//!
//! The browser supplies the same metadata/group-comparison configuration used
//! by W3 plus a post-selection feature threshold. The corrected expression
//! model is rerun from local bytes, then the shared significant-hit core builds
//! PCA, hierarchical clustering, heatmap z-scores, silhouette, nearest-neighbour
//! coherence, and subject-blocked PERMANOVA.

use crate::cohort::normalize_sample_id;
use crate::corrected::CorrectedExpressionResult;
use crate::hit_structure_core::{
    SignificantHitPanel, SignificantHitStructureConfig, build_panel, expression_rows_from_reader,
    selected_features,
};
use crate::portable_compare::{
    PortableComparisonSummary, PortableExclusionSummary, PortableExpressionComparisonConfig,
    PortableExpressionComparisonResult, run_portable_expression_comparison,
};
use crate::portable_io::PortableExpressionKind;
use anyhow::{Result, anyhow, bail};
use serde::Serialize;
use std::collections::HashMap;
use std::io::Cursor;

#[derive(Debug, Clone, Serialize)]
pub struct PortableSignificantHitResult {
    pub kind: PortableExpressionKind,
    pub source_metric: String,
    pub metadata_file: String,
    pub merged_expression_file: String,
    pub comparison: PortableComparisonSummary,
    pub exclusions: Vec<PortableExclusionSummary>,
    pub panel: Option<SignificantHitPanel>,
    pub warnings: Vec<String>,
}

fn annotations_from_expression_result(
    result: &CorrectedExpressionResult,
) -> Result<(Vec<bool>, Vec<String>, Vec<String>)> {
    let mut annotations = HashMap::<String, (bool, String, String)>::new();
    if let Some(pca) = result.pca.as_ref() {
        for point in &pca.points {
            let is_case = point.group.eq_ignore_ascii_case("Case");
            annotations.insert(
                normalize_sample_id(&point.sample_id),
                (is_case, point.group.clone(), point.subject.clone()),
            );
        }
    }
    if let Some(tree) = result.dendrogram.as_ref() {
        for leaf in &tree.leaves {
            let is_case = leaf.group.eq_ignore_ascii_case("Case");
            annotations
                .entry(normalize_sample_id(&leaf.sample_id))
                .or_insert_with(|| (is_case, leaf.group.clone(), leaf.subject.clone()));
        }
    }

    let mut cases = Vec::with_capacity(result.model_sample_ids.len());
    let mut groups = Vec::with_capacity(result.model_sample_ids.len());
    let mut subjects = Vec::with_capacity(result.model_sample_ids.len());
    for sample in &result.model_sample_ids {
        let key = normalize_sample_id(sample);
        let (is_case, group, subject) = annotations.get(&key).cloned().ok_or_else(|| {
            anyhow!("Missing group/subject annotation for modeled sample '{sample}'")
        })?;
        cases.push(is_case);
        groups.push(group);
        subjects.push(subject);
    }
    Ok((cases, groups, subjects))
}

pub fn run_portable_expression_hit_structure<F>(
    metadata_file_name: &str,
    metadata_bytes: &[u8],
    merged_file_name: &str,
    merged_bytes: &[u8],
    comparison_config: &PortableExpressionComparisonConfig,
    hit_config: SignificantHitStructureConfig,
    mut progress: F,
) -> Result<PortableSignificantHitResult>
where
    F: FnMut(usize, usize, &str),
{
    if !(0.0..=1.0).contains(&hit_config.q_threshold) {
        bail!("Significant-hit q threshold must be between 0 and 1");
    }
    if !hit_config.min_abs_effect.is_finite() || hit_config.min_abs_effect < 0.0 {
        bail!("Significant-hit minimum absolute effect must be finite and non-negative");
    }
    if hit_config.max_analysis_features < 2 {
        bail!("Significant-hit maxAnalysisFeatures must be at least 2");
    }
    if hit_config.max_heatmap_features == 0 {
        bail!("Significant-hit maxHeatmapFeatures must be at least 1");
    }

    progress(
        0,
        3,
        "Re-running the corrected comparison with the frozen W3 settings",
    );
    let PortableExpressionComparisonResult {
        kind,
        source_metric,
        metadata_file,
        merged_expression_file,
        comparison,
        exclusions,
        analysis,
    } = run_portable_expression_comparison(
        metadata_file_name,
        metadata_bytes,
        merged_file_name,
        merged_bytes,
        comparison_config,
        |done, total, message| {
            progress(
                0,
                3,
                &format!("Corrected comparison — {message} [{done}/{total}]"),
            );
        },
    )?;

    progress(1, 3, "Selecting FDR-supported features");
    let (hit_total, features) = selected_features(&analysis.tests, "", &hit_config);
    let mut warnings = vec![
        "Post-selection plots reuse the same Case/Control labels that selected the significant features. Separation is descriptive evidence of coherence, not independent validation or predictive performance."
            .to_string(),
    ];
    if hit_total == 0 {
        warnings.push(format!(
            "No features satisfied BH q <= {:.3} and |adjusted effect| >= {:.3}.",
            hit_config.q_threshold, hit_config.min_abs_effect
        ));
        progress(
            3,
            3,
            "No significant features passed the configured threshold",
        );
        return Ok(PortableSignificantHitResult {
            kind,
            source_metric,
            metadata_file,
            merged_expression_file,
            comparison,
            exclusions,
            panel: None,
            warnings,
        });
    }

    progress(2, 3, "Reconstructing the selected-feature matrix");
    let rows = expression_rows_from_reader(
        Cursor::new(merged_bytes),
        merged_bytes.len(),
        &analysis,
        &features,
        |done, total, message| {
            progress(
                2,
                3,
                &format!("Selected-hit matrix — {message} [{done}/{total}]"),
            );
        },
    )?;
    let (cases, groups, subjects) = annotations_from_expression_result(&analysis)?;
    let (modality, feature_set, distance_label) = match kind {
        PortableExpressionKind::Gene => (
            "Gene expression",
            "significant adjusted abundance hits",
            "Euclidean distance on significant normalized log2 counts",
        ),
        PortableExpressionKind::Transcript => (
            "Transcript / isoform expression",
            "significant adjusted abundance hits",
            "Euclidean distance on significant normalized log2 counts",
        ),
    };
    let panel = build_panel(
        modality,
        feature_set,
        analysis.model_sample_ids.clone(),
        cases,
        groups,
        subjects,
        hit_total,
        features,
        rows,
        distance_label,
        &hit_config,
        comparison_config.permutations,
        Vec::new(),
    )?;
    progress(3, 3, "Significant-hit structure complete");

    Ok(PortableSignificantHitResult {
        kind,
        source_metric,
        metadata_file,
        merged_expression_file,
        comparison,
        exclusions,
        panel: Some(panel),
        warnings,
    })
}
