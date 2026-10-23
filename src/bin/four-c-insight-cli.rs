use anyhow::{Result, bail};
use clap::{Parser, ValueEnum};
use four_c_insight::{
    ColumnOrder, DataKind, MergeConfig, Metric, all_metrics, collaborator_default_metrics,
    discover_xlsx_files, inspect_kind, merge_wide,
};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    version,
    about = "4C Insight CLI: merge gene/transcript expression Excel reports with bounded memory use"
)]
struct Args {
    /// Folder containing .xlsx reports.
    #[arg(short, long)]
    input: PathBuf,

    /// Output TSV path.
    #[arg(short, long)]
    output: PathBuf,

    /// Report type. Auto is normally sufficient.
    #[arg(long, value_enum, default_value_t = KindArg::Auto)]
    kind: KindArg,

    /// Measurement preset.
    #[arg(long, value_enum, default_value_t = PresetArg::Collaborator)]
    preset: PresetArg,

    /// Explicit comma-separated measurements for --preset custom.
    /// Gene: raw-count,fpkm,cpm,tpm,percentile-rank,is-expressed
    /// Transcript: transcript-length,effective-transcript-length,total-reads,tpm,percentile-rank,is-expressed
    #[arg(long, value_delimiter = ',')]
    metrics: Vec<MetricArg>,

    /// Arrange measurements adjacent within each sample, or group all samples by measurement.
    #[arg(long, value_enum, default_value_t = OrderArg::Adjacent)]
    order: OrderArg,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
enum KindArg {
    Auto,
    Gene,
    Transcript,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
enum PresetArg {
    Collaborator,
    All,
    Custom,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
enum OrderArg {
    Adjacent,
    Grouped,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
enum MetricArg {
    RawCount,
    Fpkm,
    Cpm,
    Tpm,
    PercentileRank,
    IsExpressed,
    TranscriptLength,
    EffectiveTranscriptLength,
    TotalReads,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let files = discover_xlsx_files(&args.input)?;
    if files.is_empty() {
        bail!("no .xlsx files found in {}", args.input.display());
    }

    let requested_kind = match args.kind {
        KindArg::Auto => DataKind::Auto,
        KindArg::Gene => DataKind::Gene,
        KindArg::Transcript => DataKind::Transcript,
    };

    let effective_kind = match requested_kind {
        DataKind::Auto => inspect_kind(&files[0])?,
        other => other,
    };

    let metrics = match args.preset {
        PresetArg::Collaborator => collaborator_default_metrics(effective_kind),
        PresetArg::All => all_metrics(effective_kind),
        PresetArg::Custom => {
            if args.metrics.is_empty() {
                bail!("--preset custom requires at least one --metrics value");
            }
            args.metrics.into_iter().map(map_metric).collect()
        }
    };

    let config = MergeConfig {
        files,
        output: args.output,
        kind: requested_kind,
        metrics,
        column_order: match args.order {
            OrderArg::Adjacent => ColumnOrder::AdjacentBySample,
            OrderArg::Grouped => ColumnOrder::GroupedByMeasurement,
        },
    };

    let summary = merge_wide(config, |done, total, msg| {
        eprintln!("[{done}/{total}] {msg}");
    })?;

    eprintln!(
        "Done: {:?}, {} rows, {} samples -> {}",
        summary.kind,
        summary.rows,
        summary.samples,
        summary.output.display()
    );
    if summary.missing_rows > 0 || summary.unknown_rows > 0 {
        eprintln!(
            "Warnings: {} missing canonical rows; {} rows not present in the first-file index",
            summary.missing_rows, summary.unknown_rows
        );
    }
    Ok(())
}

fn map_metric(m: MetricArg) -> Metric {
    match m {
        MetricArg::RawCount => Metric::RawCount,
        MetricArg::Fpkm => Metric::Fpkm,
        MetricArg::Cpm => Metric::Cpm,
        MetricArg::Tpm => Metric::Tpm,
        MetricArg::PercentileRank => Metric::PercentileRank,
        MetricArg::IsExpressed => Metric::IsExpressed,
        MetricArg::TranscriptLength => Metric::TranscriptLength,
        MetricArg::EffectiveTranscriptLength => Metric::EffectiveTranscriptLength,
        MetricArg::TotalReads => Metric::TotalReads,
    }
}
