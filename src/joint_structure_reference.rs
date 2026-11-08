//! Independent Rust reference calculation for 4C Insight joint ordination.
//! Validates the browser's PCoA and average-linkage clustering on EXACTLY the
//! same complete fixed-view distance matrix. No group labels enter this fit.
//! O(n^3) time / O(n^2) space; do not treat this as a fast large-cohort engine.
use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Deserialize)]
struct ReferenceInput {
    observations: Vec<String>,
    distances: Vec<f64>,
}
#[derive(Serialize)]
struct ReferencePoint {
    observation: String,
    x: f64,
    y: f64,
}
#[derive(Serialize)]
struct ReferenceMerge {
    id: usize,
    left: usize,
    right: usize,
    height: f64,
    size: usize,
}
#[derive(Serialize)]
struct ReferenceOutput {
    schema: &'static str,
    observations: Vec<String>,
    points: Vec<ReferencePoint>,
    eigenvalues: Vec<f64>,
    axis_variance_explained: Vec<f64>,
    negative_inertia_ratio: f64,
    leaf_order: Vec<usize>,
    merges: Vec<ReferenceMerge>,
}

fn checked_matrix(data: &[f64], n: usize) -> Result<()> {
    if n < 3
        || data.len()
            != n.checked_mul(n)
                .ok_or_else(|| anyhow!("Dimension overflow"))?
    {
        bail!("PCoA requires a complete matrix with three or more observations")
    }
    for i in 0..n {
        for j in 0..n {
            let d = data[i * n + j];
            if !d.is_finite() || d < 0.0 || (d - data[j * n + i]).abs() > 1e-9 {
                bail!("Distance matrix is not finite, nonnegative and symmetric")
            }
            if i == j && d.abs() > 1e-10 {
                bail!("Nonzero distance diagonal")
            }
        }
    }
    Ok(())
}
fn jacobi_eigen(matrix: &[f64], n: usize) -> Result<(Vec<f64>, Vec<f64>)> {
    let mut a = matrix.to_vec();
    let mut v = vec![0.0; n * n];
    for i in 0..n {
        v[i * n + i] = 1.0
    }
    let maxdiag = (0..n).map(|i| a[i * n + i].abs()).fold(1.0_f64, f64::max);
    let tolerance = 1e-12 * maxdiag;
    let mut converged = false;
    for _ in 0..60 {
        let mut max_off = 0.0_f64;
        let mut changed = false;
        for p in 0..n - 1 {
            for q in p + 1..n {
                let pq = p * n + q;
                let apq = a[pq];
                let abs = apq.abs();
                max_off = max_off.max(abs);
                if abs <= tolerance {
                    continue;
                }
                changed = true;
                let tau = (a[q * n + q] - a[p * n + p]) / (2.0 * apq);
                let sign = if tau >= 0.0 { 1.0 } else { -1.0 };
                let t = sign / (tau.abs() + (1.0 + tau * tau).sqrt());
                let c = 1.0 / (1.0 + t * t).sqrt();
                let s = t * c;
                let app = a[p * n + p];
                let aqq = a[q * n + q];
                a[p * n + p] = app - t * apq;
                a[q * n + q] = aqq + t * apq;
                a[pq] = 0.0;
                a[q * n + p] = 0.0;
                for k in 0..n {
                    if k != p && k != q {
                        let akp = a[k * n + p];
                        let akq = a[k * n + q];
                        let x = c * akp - s * akq;
                        let y = s * akp + c * akq;
                        a[k * n + p] = x;
                        a[p * n + k] = x;
                        a[k * n + q] = y;
                        a[q * n + k] = y;
                    }
                    let vkp = v[k * n + p];
                    let vkq = v[k * n + q];
                    v[k * n + p] = c * vkp - s * vkq;
                    v[k * n + q] = s * vkp + c * vkq;
                }
            }
        }
        if !changed || max_off <= tolerance {
            converged = true;
            break;
        }
    }
    if !converged {
        bail!("Reference Jacobi eigensolver did not converge")
    }
    Ok(((0..n).map(|i| a[i * n + i]).collect(), v))
}
fn ordination(
    data: &[f64],
    n: usize,
    names: &[String],
) -> Result<(Vec<ReferencePoint>, Vec<f64>, Vec<f64>, f64)> {
    let mut row_means = vec![0.0; n];
    let mut grand = 0.0;
    for i in 0..n {
        for j in 0..n {
            row_means[i] += data[i * n + j] * data[i * n + j]
        }
        row_means[i] /= n as f64;
        grand += row_means[i]
    }
    grand /= n as f64;
    let mut b = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            b[i * n + j] =
                -0.5 * (data[i * n + j] * data[i * n + j] - row_means[i] - row_means[j] + grand);
        }
    }
    let (values, vectors) = jacobi_eigen(&b, n)?;
    let mut ranked: Vec<(f64, usize)> = values
        .into_iter()
        .enumerate()
        .map(|(i, v)| (v, i))
        .collect();
    ranked.sort_by(|a, b| b.0.total_cmp(&a.0));
    let threshold = ranked[0].0.abs().max(1.0) * 1e-9;
    let positive: Vec<_> = ranked
        .iter()
        .filter(|(v, _)| *v > threshold)
        .copied()
        .collect();
    if positive.len() < 2 {
        bail!("Joint matrix has fewer than two positive PCoA axes")
    }
    let pos_sum: f64 = positive.iter().map(|(v, _)| v).sum();
    let neg_sum: f64 = ranked
        .iter()
        .filter(|(v, _)| *v < -threshold)
        .map(|(v, _)| -*v)
        .sum();
    let mut coords = vec![[0.0; 2]; n];
    for axis in 0..2 {
        let (value, column) = positive[axis];
        for row in 0..n {
            coords[row][axis] = vectors[row * n + column] * value.sqrt()
        }
        let mut largest = 0.0;
        let mut sign = 1.0;
        for coord in &coords {
            let x = coord[axis];
            if x.abs() > largest {
                largest = x.abs();
                sign = if x < 0.0 { -1.0 } else { 1.0 }
            }
        }
        if sign < 0.0 {
            for coord in &mut coords {
                coord[axis] = -coord[axis]
            }
        }
    }
    let points = names
        .iter()
        .enumerate()
        .map(|(i, id)| ReferencePoint {
            observation: id.clone(),
            x: coords[i][0],
            y: coords[i][1],
        })
        .collect();
    let explained = positive.iter().take(2).map(|(v, _)| v / pos_sum).collect();
    let ratio = if pos_sum + neg_sum > 0.0 {
        neg_sum / (pos_sum + neg_sum)
    } else {
        0.0
    };
    Ok((
        points,
        ranked.iter().map(|(v, _)| *v).collect(),
        explained,
        ratio,
    ))
}
fn visit(
    id: usize,
    n: usize,
    children: &[Option<(usize, usize)>],
    order: &mut Vec<usize>,
) -> Result<()> {
    if id < n {
        order.push(id);
        return Ok(());
    }
    let (a, b) = children
        .get(id)
        .and_then(|x| *x)
        .ok_or_else(|| anyhow!("Invalid cluster tree"))?;
    visit(a, n, children, order)?;
    visit(b, n, children, order)?;
    Ok(())
}
fn average_linkage(data: &[f64], n: usize) -> Result<(Vec<ReferenceMerge>, Vec<usize>)> {
    let capacity = 2 * n - 1;
    let mut distances = vec![0.0; capacity * capacity];
    for i in 0..n {
        for j in 0..n {
            distances[i * capacity + j] = data[i * n + j]
        }
    }
    let mut active: BTreeSet<usize> = (0..n).collect();
    let mut sizes = vec![1_usize; capacity];
    let mut children = vec![None; capacity];
    let mut merges = Vec::with_capacity(n - 1);
    for step in 0..n - 1 {
        let ordered: Vec<_> = active.iter().copied().collect();
        let mut best = f64::INFINITY;
        let mut chosen = None;
        for i in 0..ordered.len() {
            for j in i + 1..ordered.len() {
                let a = ordered[i];
                let b = ordered[j];
                let distance = distances[a * capacity + b];
                if distance < best {
                    best = distance;
                    chosen = Some((a, b))
                }
            }
        }
        let (a, b) = chosen.ok_or_else(|| anyhow!("Missing distance merge"))?;
        if !best.is_finite() {
            bail!("Nonfinite linkage merge")
        }
        let id = n + step;
        let size = sizes[a] + sizes[b];
        sizes[id] = size;
        children[id] = Some((a, b));
        merges.push(ReferenceMerge {
            id,
            left: a,
            right: b,
            height: best,
            size,
        });
        for other in &active {
            if *other == a || *other == b {
                continue;
            }
            let d = (sizes[a] as f64 * distances[a * capacity + other]
                + sizes[b] as f64 * distances[b * capacity + other])
                / size as f64;
            distances[id * capacity + other] = d;
            distances[other * capacity + id] = d;
        }
        active.remove(&a);
        active.remove(&b);
        active.insert(id);
    }
    let mut order = Vec::with_capacity(n);
    visit(2 * n - 2, n, &children, &mut order)?;
    Ok((merges, order))
}

/// Validation/export function. Never reads group or outcome labels from JSON.
pub fn reference_joint_structure_json(input_json: &str) -> Result<String> {
    let input: ReferenceInput = serde_json::from_str(input_json)?;
    let n = input.observations.len();
    checked_matrix(&input.distances, n)?;
    let mut names = std::collections::HashSet::new();
    for name in &input.observations {
        if name.is_empty() || name.trim() != name || !names.insert(name) {
            bail!("Observation IDs must be exact and unique")
        }
    }
    let (points, eigenvalues, axis_variance_explained, negative_inertia_ratio) =
        ordination(&input.distances, n, &input.observations)?;
    let (merges, leaf_order) = average_linkage(&input.distances, n)?;
    Ok(serde_json::to_string(&ReferenceOutput {
        schema: "4c-insight-native-joint-reference-v1",
        observations: input.observations,
        points,
        eigenvalues,
        axis_variance_explained,
        negative_inertia_ratio,
        leaf_order,
        merges,
    })?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn euclidean_square_parity() {
        let input = serde_json::json!({
            "observations":["A","B","C","D"],
            "distances":[0.,1.,1.,2.0_f64.sqrt(),1.,0.,2.0_f64.sqrt(),1.,1.,2.0_f64.sqrt(),0.,1.,2.0_f64.sqrt(),1.,1.,0.]
        });
        let text = reference_joint_structure_json(&input.to_string()).unwrap();
        let result: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(result["leaf_order"].as_array().unwrap().len(), 4);
        let explained = result["axis_variance_explained"].as_array().unwrap();
        assert!((explained[0].as_f64().unwrap() - 0.5).abs() < 1e-8);
        assert!((explained[1].as_f64().unwrap() - 0.5).abs() < 1e-8);
        assert!(result["negative_inertia_ratio"].as_f64().unwrap() < 1e-8);
        let coords = result["points"].as_array().unwrap();
        for i in 0..4 {
            for j in i + 1..4 {
                let dx = coords[i]["x"].as_f64().unwrap() - coords[j]["x"].as_f64().unwrap();
                let dy = coords[i]["y"].as_f64().unwrap() - coords[j]["y"].as_f64().unwrap();
                let reconstructed = (dx * dx + dy * dy).sqrt();
                let expected = input["distances"][i * 4 + j].as_f64().unwrap();
                assert!((reconstructed - expected).abs() < 1e-6);
            }
        }
    }
    #[test]
    fn reject_invalid_matrix() {
        let bad = serde_json::json!({"observations":["A","B","C"],"distances":[0.,1.,2.,1.,0.,-1.,2.,-1.,0.]});
        assert!(reference_joint_structure_json(&bad.to_string()).is_err());
    }
}
