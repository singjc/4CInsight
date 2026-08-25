use anyhow::{Context, Result, anyhow, bail};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const MSIGDB_VERSION: &str = "2026.1.Hs";
pub const MSIGDB_RELEASE_URL: &str =
    "https://data.broadinstitute.org/gsea-msigdb/msigdb/release/2026.1.Hs/";
pub const MSIGDB_LICENSE_URL: &str = "https://www.gsea-msigdb.org/gsea/msigdb_license_terms.jsp";

#[derive(Debug, Clone, Copy)]
pub struct GmtCatalogEntry {
    pub label: &'static str,
    pub short_label: &'static str,
    pub filename: &'static str,
    pub description: &'static str,
    pub relevance: &'static str,
}

impl GmtCatalogEntry {
    pub fn url(self) -> String {
        format!("{MSIGDB_RELEASE_URL}{}", self.filename)
    }
}

pub const GMT_CATALOG: &[GmtCatalogEntry] = &[
    GmtCatalogEntry {
        label: "Reactome — curated mechanistic pathways (recommended)",
        short_label: "Reactome",
        filename: "c2.cp.reactome.v2026.1.Hs.symbols.gmt",
        description: "Curated Reactome pathways using human gene symbols.",
        relevance: "Recommended mechanistic collection for extracellular matrix, collagen, integrin, cytokine, signaling, metabolism, and related pathways.",
    },
    GmtCatalogEntry {
        label: "Hallmark — compact biological programs",
        short_label: "Hallmark",
        filename: "h.all.v2026.1.Hs.symbols.gmt",
        description: "The compact 50-set Hallmark collection using human gene symbols.",
        relevance: "Best for a concise overview and presentation-friendly biological themes.",
    },
    GmtCatalogEntry {
        label: "GO Biological Process — detailed processes",
        short_label: "GO Biological Process",
        filename: "c5.go.bp.v2026.1.Hs.symbols.gmt",
        description: "Gene Ontology Biological Process gene sets using human gene symbols.",
        relevance: "Detailed coverage of extracellular-matrix organization, collagen biology, adhesion, migration, inflammation, chemotaxis, development, and many other processes.",
    },
    GmtCatalogEntry {
        label: "ImmuneSigDB — immune signatures",
        short_label: "ImmuneSigDB",
        filename: "c7.immunesigdb.v2026.1.Hs.symbols.gmt",
        description: "MSigDB immunologic signatures using human gene symbols.",
        relevance: "Useful for the ME/CFS and immune-dysregulation component of the cohort; results can be redundant because the collection is large.",
    },
    GmtCatalogEntry {
        label: "HPO — phenotype-associated gene sets",
        short_label: "HPO",
        filename: "c5.hpo.v2026.1.Hs.symbols.gmt",
        description: "Human Phenotype Ontology-associated gene sets using human gene symbols.",
        relevance: "Use as phenotype relevance/enrichment rather than a literal pathway database; useful for connective-tissue, skin, joint, and systemic phenotype context.",
    },
    GmtCatalogEntry {
        label: "Cell Type Signatures (C8)",
        short_label: "Cell Type Signatures",
        filename: "c8.all.v2026.1.Hs.symbols.gmt",
        description: "MSigDB cell-type signature gene sets using human gene symbols.",
        relevance: "Useful when expression differences may reflect immune or tissue-cell composition shifts.",
    },
    GmtCatalogEntry {
        label: "WikiPathways — curated community pathways",
        short_label: "WikiPathways",
        filename: "c2.cp.wikipathways.v2026.1.Hs.symbols.gmt",
        description: "WikiPathways canonical-pathway collection distributed through MSigDB.",
        relevance: "A useful independent curated pathway view complementary to Reactome.",
    },
];

pub fn cache_path(project_root: &Path, entry: GmtCatalogEntry) -> PathBuf {
    project_root
        .join("gene_sets")
        .join(format!("msigdb_v{MSIGDB_VERSION}"))
        .join(entry.filename)
}

pub fn validate_gmt(path: &Path) -> Result<()> {
    let meta = fs::metadata(path)
        .with_context(|| format!("Cannot inspect downloaded GMT: {}", path.display()))?;
    if meta.len() < 32 {
        bail!(
            "Downloaded file is unexpectedly small ({} bytes).",
            meta.len()
        );
    }
    let text = fs::read_to_string(path)
        .with_context(|| format!("Cannot read downloaded GMT: {}", path.display()))?;
    let mut valid_lines = 0usize;
    for line in text.lines().take(20) {
        if line.trim().is_empty() {
            continue;
        }
        if line.split('\t').count() >= 3 {
            valid_lines += 1;
        }
    }
    if valid_lines == 0 {
        bail!(
            "Downloaded file does not look like a GMT gene-set file. The server may have returned an HTML/error page instead."
        );
    }
    Ok(())
}

fn try_curl(url: &str, output: &Path) -> Result<()> {
    let status = Command::new("curl")
        .arg("-L")
        .arg("--fail")
        .arg("--silent")
        .arg("--show-error")
        .arg("--connect-timeout")
        .arg("20")
        .arg("--max-time")
        .arg("300")
        .arg("-o")
        .arg(output)
        .arg(url)
        .status()
        .context("Could not start curl")?;
    if !status.success() {
        bail!("curl exited with status {status}");
    }
    Ok(())
}

fn try_wget(url: &str, output: &Path) -> Result<()> {
    let status = Command::new("wget")
        .arg("-q")
        .arg("-O")
        .arg(output)
        .arg(url)
        .status()
        .context("Could not start wget")?;
    if !status.success() {
        bail!("wget exited with status {status}");
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn try_powershell(url: &str, output: &Path) -> Result<()> {
    let script = "$ProgressPreference='SilentlyContinue'; Invoke-WebRequest -UseBasicParsing -Uri $args[0] -OutFile $args[1]";
    let status = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ])
        .arg(url)
        .arg(output)
        .status()
        .context("Could not start PowerShell")?;
    if !status.success() {
        bail!("PowerShell download exited with status {status}");
    }
    Ok(())
}

pub fn download_public_gmt(project_root: &Path, entry: GmtCatalogEntry) -> Result<PathBuf> {
    let destination = cache_path(project_root, entry);
    if destination.exists() {
        validate_gmt(&destination)?;
        return Ok(destination);
    }
    let parent = destination
        .parent()
        .ok_or_else(|| anyhow!("Invalid GMT cache path"))?;
    fs::create_dir_all(parent)
        .with_context(|| format!("Cannot create GMT cache directory: {}", parent.display()))?;
    let partial = destination.with_extension("gmt.part");
    let _ = fs::remove_file(&partial);
    let url = entry.url();

    let mut errors = Vec::new();
    let downloaded = match try_curl(&url, &partial) {
        Ok(()) => true,
        Err(e) => {
            errors.push(format!("curl: {e}"));
            false
        }
    } || match try_wget(&url, &partial) {
        Ok(()) => true,
        Err(e) => {
            errors.push(format!("wget: {e}"));
            false
        }
    };

    #[cfg(target_os = "windows")]
    let downloaded = downloaded
        || match try_powershell(&url, &partial) {
            Ok(()) => true,
            Err(e) => {
                errors.push(format!("PowerShell: {e}"));
                false
            }
        };

    if !downloaded {
        let _ = fs::remove_file(&partial);
        bail!(
            "Automatic GMT download failed. {}. You can still use 'Choose custom GMT…'. Source URL: {url}",
            errors.join(" | ")
        );
    }

    if let Err(e) = validate_gmt(&partial) {
        let _ = fs::remove_file(&partial);
        return Err(e).context("Downloaded GMT failed validation");
    }
    fs::rename(&partial, &destination).with_context(|| {
        format!(
            "Cannot move downloaded GMT into cache: {}",
            destination.display()
        )
    })?;
    Ok(destination)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_uses_symbol_gmts_and_unique_files() {
        let mut seen = std::collections::HashSet::new();
        assert!(GMT_CATALOG.len() >= 5);
        for entry in GMT_CATALOG {
            assert!(entry.filename.ends_with(".symbols.gmt"));
            assert!(seen.insert(entry.filename));
            assert!(entry.url().starts_with("https://"));
        }
    }
}
