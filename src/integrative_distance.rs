//! I2A: Unsupervised multi-view distance fusion from full source distances.
//! No case/control labels, adjusted hits, imputation, or pairwise identity guessing.
//! Per-view scale is the median strictly positive off-diagonal distance.
use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Deserialize)]
struct DistanceInput {
    observations: Vec<String>,
    views: Vec<DistanceView>,
    #[serde(default = "default_min_views")]
    min_views: usize,
}
fn default_min_views() -> usize {
    2
}

#[derive(Debug, Deserialize)]
struct DistanceView {
    modality: String,
    distance_label: String,
    pairs: Vec<DistancePair>,
}
#[derive(Debug, Deserialize)]
struct DistancePair {
    a: String,
    b: String,
    distance: f64,
}

#[derive(Debug, Serialize)]
struct ViewSummary {
    modality: String,
    distance_label: String,
    source_pairs: usize,
    positive_median: f64,
    normalization: &'static str,
}
#[derive(Debug, Serialize)]
struct FusedPair {
    a: String,
    b: String,
    distance: f64,
    contributing_views: usize,
    modalities: Vec<String>,
}
#[derive(Debug, Serialize)]
struct Neighbor {
    observation: String,
    neighbor: Option<String>,
    distance: Option<f64>,
    contributing_views: usize,
}
#[derive(Debug, Serialize)]
struct Ablation {
    omitted: String,
    overlapping_pairs: usize,
    pearson_distance_r: Option<f64>,
}
#[derive(Debug, Serialize)]
struct DistanceOutput {
    schema: &'static str,
    model: &'static str,
    normalization: &'static str,
    observations: Vec<String>,
    views: Vec<ViewSummary>,
    min_views: usize,
    possible_pairs: usize,
    fused_pairs: usize,
    unavailable_pairs: usize,
    pairs: Vec<FusedPair>,
    nearest_neighbors: Vec<Neighbor>,
    ablation: Vec<Ablation>,
    warnings: Vec<String>,
}

fn key(a: &str, b: &str) -> (String, String) {
    if a < b {
        (a.to_owned(), b.to_owned())
    } else {
        (b.to_owned(), a.to_owned())
    }
}
fn median_positive(input: impl Iterator<Item = f64>) -> Result<f64> {
    let mut numbers: Vec<f64> = input.filter(|x| *x > 0.0 && x.is_finite()).collect();
    if numbers.is_empty() {
        bail!("A view has no positive distances; a scale cannot be estimated")
    }
    numbers.sort_by(f64::total_cmp);
    let n = numbers.len();
    let middle = n / 2;
    let m = if n % 2 == 1 {
        numbers[middle]
    } else {
        numbers[middle - 1] / 2.0 + numbers[middle] / 2.0
    };
    if !m.is_finite() || m <= 0.0 {
        bail!("Invalid view distance scale")
    }
    Ok(m)
}
fn pearson(points: &[(f64, f64)]) -> Option<f64> {
    if points.len() < 3 {
        return None;
    }
    let n = points.len() as f64;
    let (mut sx, mut sy) = (0.0, 0.0);
    for &(x, y) in points {
        sx += x;
        sy += y;
    }
    let (mx, my) = (sx / n, sy / n);
    let (mut cov, mut vx, mut vy) = (0.0, 0.0, 0.0);
    for &(x, y) in points {
        let dx = x - mx;
        let dy = y - my;
        cov += dx * dy;
        vx += dx * dx;
        vy += dy * dy;
    }
    let den = (vx * vy).sqrt();
    if den == 0.0 || !den.is_finite() {
        None
    } else {
        Some((cov / den).clamp(-1.0, 1.0))
    }
}

/// A numeric input is canonicalized from the explicit I1 alignment in the browser.
/// Rust verifies identity, source scope, distance validity, and duplicate pairs again.
pub fn fuse_distances_json(input_json: &str) -> Result<String> {
    let request: DistanceInput = serde_json::from_str(input_json)?;
    if request.observations.len() < 3 {
        bail!("At least three aligned observations are required")
    }
    if request.views.len() < 2 || request.views.len() > 7 {
        bail!("Select between two and seven distance views")
    }
    if request.min_views < 2 || request.min_views > request.views.len() {
        bail!("Minimum contributing views must be at least two and no more than selected views")
    }
    let mut seen = HashSet::new();
    for id in &request.observations {
        if id.is_empty() || id.trim() != id || !seen.insert(id.clone()) {
            bail!("Observation IDs must be nonempty, exact and unique")
        }
    }
    let expected = [
        "gene",
        "transcript",
        "bcr",
        "tcrAlpha",
        "tcrBeta",
        "isotype",
        "immune",
    ];
    let mut used = HashSet::new();
    let mut views = Vec::new();
    let mut indexed = Vec::new();
    for view in &request.views {
        if !expected.contains(&view.modality.as_str()) || !used.insert(view.modality.as_str()) {
            bail!("Unknown or duplicate modality: {}", view.modality)
        }
        if view.distance_label.trim().is_empty() {
            bail!("Distance method/units must be specified for each view")
        }
        let mut pairs = BTreeMap::new();
        for p in &view.pairs {
            if p.a == p.b {
                bail!("A distance view contains a self-pair")
            }
            if !seen.contains(&p.a) || !seen.contains(&p.b) {
                bail!(
                    "A distance view contains an ID outside the aligned observations: {} / {}",
                    p.a,
                    p.b
                )
            }
            if !p.distance.is_finite() || p.distance < 0.0 {
                bail!("Distances must be finite and nonnegative")
            }
            let pair = key(&p.a, &p.b);
            if pairs.insert(pair, p.distance).is_some() {
                bail!("Duplicate unordered sample pair in {}", view.modality)
            }
        }
        if pairs.len() < 2 {
            bail!("{} needs at least two distinct source pairs", view.modality)
        }
        let scale = median_positive(pairs.values().copied())
            .map_err(|e| anyhow!("{}: {}", view.modality, e))?;
        views.push(ViewSummary {
            modality: view.modality.clone(),
            distance_label: view.distance_label.clone(),
            source_pairs: pairs.len(),
            positive_median: scale,
            normalization: "distance / within-view median positive pair distance",
        });
        indexed.push(pairs);
    }
    let mut union: BTreeMap<(String, String), Vec<(usize, f64)>> = BTreeMap::new();
    for (view_index, distances) in indexed.iter().enumerate() {
        for (pair, value) in distances {
            union
                .entry(pair.clone())
                .or_default()
                .push((view_index, value / views[view_index].positive_median));
        }
    }
    let n = request.observations.len();
    let possible_pairs = n
        .checked_mul(n - 1)
        .and_then(|x| x.checked_div(2))
        .ok_or_else(|| anyhow!("Too many observations to count sample pairs"))?;
    let mut pairs = Vec::new();
    let mut ablation_data: Vec<Vec<(f64, f64)>> = vec![Vec::new(); views.len()];
    let mut neighbor_index: HashMap<&str, (&str, f64, usize)> = HashMap::new();
    for ((a, b), contributions) in &union {
        if contributions.len() < request.min_views {
            continue;
        }
        let sum: f64 = contributions.iter().map(|(_, x)| x).sum();
        let fused = sum / contributions.len() as f64;
        if !fused.is_finite() {
            bail!("Normalized fused distance overflowed")
        }
        for removed in 0..views.len() {
            let retained: Vec<f64> = contributions
                .iter()
                .filter(|(idx, _)| *idx != removed)
                .map(|(_, v)| *v)
                .collect();
            if retained.len() >= request.min_views {
                ablation_data[removed]
                    .push((fused, retained.iter().sum::<f64>() / retained.len() as f64));
            }
        }
        for (source, target) in [(a.as_str(), b.as_str()), (b.as_str(), a.as_str())] {
            let prior = neighbor_index.get(source);
            if prior.is_none_or(|(id, d, _)| fused < *d || (fused == *d && target < *id)) {
                neighbor_index.insert(source, (target, fused, contributions.len()));
            }
        }
        pairs.push(FusedPair {
            a: a.clone(),
            b: b.clone(),
            distance: fused,
            contributing_views: contributions.len(),
            modalities: contributions
                .iter()
                .map(|(idx, _)| views[*idx].modality.clone())
                .collect(),
        });
    }
    let nearest_neighbors = request
        .observations
        .iter()
        .map(|id| {
            if let Some(&(other, d, count)) = neighbor_index.get(id.as_str()) {
                Neighbor {
                    observation: id.clone(),
                    neighbor: Some(other.to_string()),
                    distance: Some(d),
                    contributing_views: count,
                }
            } else {
                Neighbor {
                    observation: id.clone(),
                    neighbor: None,
                    distance: None,
                    contributing_views: 0,
                }
            }
        })
        .collect();
    let ablation = views
        .iter()
        .enumerate()
        .map(|(i, view)| Ablation {
            omitted: view.modality.clone(),
            overlapping_pairs: ablation_data[i].len(),
            pearson_distance_r: pearson(&ablation_data[i]),
        })
        .collect();
    let unavailable = possible_pairs.saturating_sub(pairs.len());
    let mut warnings = Vec::new();
    if unavailable > 0 {
        warnings.push(format!(
            "{} sample pairs lack the required view overlap; no values were imputed",
            unavailable
        ))
    }
    if request.views.len() == 2 {
        warnings.push(
            "Leave-one-view-out stability cannot use two views at a minimum overlap of two"
                .to_string(),
        )
    }
    if pairs.is_empty() {
        warnings.push("No observation pair has sufficient independent view coverage".to_string())
    }
    let output = DistanceOutput {
        schema: "4c-insight-distance-fusion-v1",
        model: "unsupervised-median-scaled-mean-distance",
        normalization: "Each original distance is divided by its view's median positive distance. Only observed pairs with sufficient view coverage are averaged. No matrix completion, p-values or supervised fitting.",
        observations: request.observations,
        views,
        min_views: request.min_views,
        possible_pairs,
        fused_pairs: pairs.len(),
        unavailable_pairs: unavailable,
        pairs,
        nearest_neighbors,
        ablation,
        warnings,
    };
    Ok(serde_json::to_string(&output)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    fn input() -> Value {
        json!({"observations":["A","B","C","D"],"min_views":2,"views":[
          {"modality":"gene","distance_label":"raw expression log2 distances","pairs":[
            {"a":"A","b":"B","distance":2.0},{"a":"A","b":"C","distance":4.0},{"a":"A","b":"D","distance":6.0},
            {"a":"B","b":"C","distance":2.0},{"a":"B","b":"D","distance":4.0},{"a":"C","b":"D","distance":2.0}]},
          {"modality":"bcr","distance_label":"Morisita-Horn","pairs":[
            {"a":"A","b":"B","distance":0.1},{"a":"A","b":"C","distance":0.2},{"a":"A","b":"D","distance":0.3},
            {"a":"B","b":"C","distance":0.1},{"a":"B","b":"D","distance":0.2},{"a":"C","b":"D","distance":0.1}]},
          {"modality":"immune","distance_label":"standardized ssGSEA","pairs":[
            {"a":"A","b":"B","distance":1.0},{"a":"A","b":"C","distance":2.0},{"a":"A","b":"D","distance":3.0},
            {"a":"B","b":"C","distance":1.0},{"a":"B","b":"D","distance":2.0},{"a":"C","b":"D","distance":1.0}]}
        ]})
    }
    fn run(v: &Value) -> Value {
        serde_json::from_str(&fuse_distances_json(&v.to_string()).unwrap()).unwrap()
    }
    #[test]
    fn scale_invariance_and_analytic_pairs() {
        let r = run(&input());
        assert_eq!(r["fused_pairs"], 6);
        // Medians 3.0, 0.15, 1.5 (even sorted positive medians).
        assert!((r["pairs"][0]["distance"].as_f64().unwrap() - 2.0 / 3.0).abs() < 1e-12);
        assert_eq!(r["pairs"][0]["contributing_views"], 3);
        assert_eq!(r["nearest_neighbors"][0]["neighbor"], "B");
        for a in r["ablation"].as_array().unwrap() {
            assert!((a["pearson_distance_r"].as_f64().unwrap() - 1.0).abs() < 1e-12);
        }
    }
    #[test]
    fn missing_pairs_not_imputed_and_overlap_counted() {
        let mut v = input();
        v["views"][1]["pairs"]
            .as_array_mut()
            .unwrap()
            .retain(|p| p["b"] != "D");
        v["views"][2]["pairs"]
            .as_array_mut()
            .unwrap()
            .retain(|p| p["b"] != "D");
        let r = run(&v);
        assert_eq!(r["fused_pairs"], 3);
        assert_eq!(r["unavailable_pairs"], 3);
        assert!(r["nearest_neighbors"][3]["neighbor"].is_null());
    }
    #[test]
    fn no_labels_in_schema_and_reject_invalid_inputs() {
        let r = run(&input());
        assert!(!r.to_string().contains("group_case"));
        let mut v = input();
        v["views"][0]["pairs"][0]["distance"] = json!(-1.0);
        assert!(fuse_distances_json(&v.to_string()).is_err());
        let mut v = input();
        v["views"][0]["pairs"][1]["a"] = json!("B");
        v["views"][0]["pairs"][1]["b"] = json!("A");
        assert!(fuse_distances_json(&v.to_string()).is_err());
        let mut v = input();
        v["observations"][1] = json!("A");
        assert!(fuse_distances_json(&v.to_string()).is_err());
        let mut v = input();
        v["views"][1]["modality"] = json!("gene");
        assert!(fuse_distances_json(&v.to_string()).is_err());
        let mut v = input();
        v["min_views"] = json!(4);
        assert!(fuse_distances_json(&v.to_string()).is_err());
    }
}
