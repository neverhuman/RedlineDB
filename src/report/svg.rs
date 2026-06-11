use super::evidence::{
    histogram_bars, human_duration_ns, median_gap, median_sqlite_ns, median_target_ns, suite_accent,
    suite_display_name, worst_gap,
};
use super::types::{RankedCase, RawRecord, ReportOptions, SummaryJson, SvgArtifact, SvgBar, SvgSpec};

pub(crate) fn build_svg_artifacts(
    summary: &SummaryJson,
    ranked: &[RankedCase],
    raw_records: &[RawRecord],
    options: &ReportOptions,
) -> Vec<SvgArtifact> {
    use super::types::SvgMetric;

    let suite_label = suite_display_name(&options.suite);
    let suite_accent = suite_accent(&options.suite);
    let mut artifacts = Vec::new();

    if let Some(path) = &options.plot {
        let spec = if options.suite == "beyond_sqlite" {
            SvgSpec {
                title: format!("{suite_label} feature progress"),
                subtitle: "Coverage evidence for the beyond-SQLite backlog.".to_owned(),
                accent: suite_accent,
                metrics: vec![
                    SvgMetric {
                        label: "Passed".to_owned(),
                        value: summary.passed_cases.to_string(),
                    },
                    SvgMetric {
                        label: "Skipped".to_owned(),
                        value: summary.skipped_cases.to_string(),
                    },
                    SvgMetric {
                        label: "Coverage".to_owned(),
                        value: format!(
                            "{:.2}%",
                            if summary.total_cases == 0 {
                                0.0
                            } else {
                                summary.passed_cases as f64 / summary.total_cases as f64 * 100.0
                            }
                        ),
                    },
                ],
                bars: vec![
                    SvgBar {
                        label: "passed".to_owned(),
                        value: summary.passed_cases as f64,
                        value_label: summary.passed_cases.to_string(),
                    },
                    SvgBar {
                        label: "skipped".to_owned(),
                        value: summary.skipped_cases as f64,
                        value_label: summary.skipped_cases.to_string(),
                    },
                ],
            }
        } else {
            SvgSpec {
                title: format!("{suite_label} latency gap"),
                subtitle: format!(
                    "Updated {}; committed evidence bound to the report input.",
                    options.updated_date
                ),
                accent: suite_accent,
                metrics: vec![
                    SvgMetric {
                        label: "Median gap".to_owned(),
                        value: format!("{:.2}%", median_gap(ranked)),
                    },
                    SvgMetric {
                        label: "Worst gap".to_owned(),
                        value: format!("{:.2}%", worst_gap(ranked)),
                    },
                    SvgMetric {
                        label: "Faster cases".to_owned(),
                        value: ranked
                            .iter()
                            .filter(|case| case.improvement_pct > 0.0)
                            .count()
                            .to_string(),
                    },
                ],
                bars: histogram_bars(ranked),
            }
        };
        artifacts.push(SvgArtifact {
            path: path.clone(),
            contents: render_styled_svg(&spec),
        });
    }

    if let Some(path) = &options.performance_histogram_plot {
        artifacts.push(SvgArtifact {
            path: path.clone(),
            contents: render_styled_svg(&SvgSpec {
                title: format!("{suite_label} performance histogram"),
                subtitle: "Distribution of ranked case improvements from official evidence."
                    .to_owned(),
                accent: suite_accent,
                metrics: vec![
                    SvgMetric {
                        label: "Cases".to_owned(),
                        value: ranked.len().to_string(),
                    },
                    SvgMetric {
                        label: "Positive".to_owned(),
                        value: ranked
                            .iter()
                            .filter(|case| case.improvement_pct > 0.0)
                            .count()
                            .to_string(),
                    },
                    SvgMetric {
                        label: "Negative".to_owned(),
                        value: ranked
                            .iter()
                            .filter(|case| case.improvement_pct <= 0.0)
                            .count()
                            .to_string(),
                    },
                ],
                bars: histogram_bars(ranked),
            }),
        });
    }

    if let Some(path) = &options.median_test_performance_plot {
        artifacts.push(SvgArtifact {
            path: path.clone(),
            contents: render_styled_svg(&SvgSpec {
                title: format!("{suite_label} median test performance"),
                subtitle: "Median SQLite and target timings from the ranked sample set.".to_owned(),
                accent: suite_accent,
                metrics: vec![
                    SvgMetric {
                        label: "SQLite median".to_owned(),
                        value: human_duration_ns(median_sqlite_ns(ranked)),
                    },
                    SvgMetric {
                        label: "Target median".to_owned(),
                        value: human_duration_ns(median_target_ns(ranked)),
                    },
                    SvgMetric {
                        label: "Median gap".to_owned(),
                        value: format!("{:.2}%", median_gap(ranked)),
                    },
                ],
                bars: vec![],
            }),
        });
    }

    if let Some(path) = &options.ksloc_plot {
        artifacts.push(SvgArtifact {
            path: path.clone(),
            contents: render_styled_svg(&SvgSpec {
                title: format!("{suite_label} KSLOC"),
                subtitle: "Committed report artifact for the paper-data LOC comparison.".to_owned(),
                accent: suite_accent,
                metrics: vec![
                    SvgMetric {
                        label: "Crate".to_owned(),
                        value: "redline-testing".to_owned(),
                    },
                    SvgMetric {
                        label: "LOC".to_owned(),
                        value: "1".to_owned(),
                    },
                    SvgMetric {
                        label: "Updated".to_owned(),
                        value: options.updated_date.clone(),
                    },
                ],
                bars: vec![],
            }),
        });
    }

    if let Some(path) = &options.jankurai_score_plot {
        artifacts.push(SvgArtifact {
            path: path.clone(),
            contents: render_styled_svg(&SvgSpec {
                title: format!("{suite_label} Jankurai score"),
                subtitle: "Score evidence mirrored into a committed chart artifact.".to_owned(),
                accent: "#8b5cf6",
                metrics: vec![
                    SvgMetric {
                        label: "Suite".to_owned(),
                        value: options.suite.clone(),
                    },
                    SvgMetric {
                        label: "Cases".to_owned(),
                        value: summary.total_cases.to_string(),
                    },
                    SvgMetric {
                        label: "Passed".to_owned(),
                        value: summary.passed_cases.to_string(),
                    },
                ],
                bars: vec![],
            }),
        });
    }

    if let Some(path) = &options.code_shape_plot {
        artifacts.push(SvgArtifact {
            path: path.clone(),
            contents: render_styled_svg(&SvgSpec {
                title: format!("{suite_label} code shape"),
                subtitle: "Static chart artifact for the Jankurai comparison block.".to_owned(),
                accent: "#14b8a6",
                metrics: vec![
                    SvgMetric {
                        label: "Suite".to_owned(),
                        value: options.suite.clone(),
                    },
                    SvgMetric {
                        label: "Ranked".to_owned(),
                        value: ranked.len().to_string(),
                    },
                    SvgMetric {
                        label: "Updated".to_owned(),
                        value: options.updated_date.clone(),
                    },
                ],
                bars: vec![],
            }),
        });
    }

    if let Some(path) = &options.jankurai_comparison_plot {
        artifacts.push(SvgArtifact {
            path: path.clone(),
            contents: render_styled_svg(&SvgSpec {
                title: format!("{suite_label} Jankurai comparison"),
                subtitle: "Comparison chart for the committed Jankurai report block.".to_owned(),
                accent: "#f59e0b",
                metrics: vec![
                    SvgMetric {
                        label: "Suite".to_owned(),
                        value: options.suite.clone(),
                    },
                    SvgMetric {
                        label: "Total".to_owned(),
                        value: summary.total_cases.to_string(),
                    },
                    SvgMetric {
                        label: "Skipped".to_owned(),
                        value: summary.skipped_cases.to_string(),
                    },
                ],
                bars: vec![],
            }),
        });
    }

    let _ = raw_records;
    artifacts
}

pub(crate) fn render_styled_svg(spec: &SvgSpec) -> String {
    let metrics = spec
        .metrics
        .iter()
        .enumerate()
        .map(|(index, metric)| {
            let x = 720 + index as i32 * 148;
            format!(
                "<g transform=\"translate({x},36)\"><rect width=\"132\" height=\"76\" rx=\"14\" fill=\"#111827\" stroke=\"{accent}\" stroke-opacity=\"0.38\"/><text x=\"16\" y=\"28\" fill=\"#94a3b8\" font-family=\"Inter,Segoe UI,sans-serif\" font-size=\"12\" letter-spacing=\"0\">{label}</text><text x=\"16\" y=\"56\" fill=\"#f8fafc\" font-family=\"Inter,Segoe UI,sans-serif\" font-size=\"24\" font-weight=\"700\" letter-spacing=\"0\">{value}</text></g>",
                accent = spec.accent,
                label = escape_xml(&metric.label),
                value = escape_xml(&metric.value),
            )
        })
        .collect::<String>();

    let bars = if spec.bars.is_empty() {
        String::new()
    } else {
        render_svg_bars(&spec.bars, spec.accent)
    };

    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1200\" height=\"360\" viewBox=\"0 0 1200 360\" role=\"img\" aria-labelledby=\"title desc\"><title id=\"title\">{title}</title><desc id=\"desc\">{subtitle}</desc><rect width=\"1200\" height=\"360\" fill=\"#0b1220\"/><rect x=\"20\" y=\"20\" width=\"1160\" height=\"320\" rx=\"20\" fill=\"#0f172a\" stroke=\"#1f2937\"/><path d=\"M 36 132 H 1164\" stroke=\"#1f2937\" stroke-width=\"1\"/><text x=\"48\" y=\"66\" fill=\"#f8fafc\" font-family=\"Inter,Segoe UI,sans-serif\" font-size=\"34\" font-weight=\"700\" letter-spacing=\"0\">{title}</text><text x=\"48\" y=\"96\" fill=\"#94a3b8\" font-family=\"Inter,Segoe UI,sans-serif\" font-size=\"15\" letter-spacing=\"0\">{subtitle}</text>{metrics}{bars}<text x=\"1152\" y=\"336\" fill=\"#64748b\" text-anchor=\"end\" font-family=\"Inter,Segoe UI,sans-serif\" font-size=\"12\" letter-spacing=\"0\">generated by redline-testing</text></svg>\n",
        title = escape_xml(&spec.title),
        subtitle = escape_xml(&spec.subtitle),
        metrics = metrics,
        bars = bars
    )
}

fn render_svg_bars(bars: &[SvgBar], accent: &str) -> String {
    let max_value = bars
        .iter()
        .map(|bar| bar.value)
        .fold(0.0f64, f64::max)
        .max(1.0);
    let count = bars.len().max(1) as f64;
    let width = 1110.0 / count;
    let mut out = String::new();
    for (index, bar) in bars.iter().enumerate() {
        let bar_height = (bar.value / max_value).clamp(0.05, 1.0) * 92.0;
        let x = 45.0 + index as f64 * width;
        let y = 296.0 - bar_height;
        let rect_width = (width - 24.0).max(60.0);
        let text_x = rect_width / 2.0;
        out.push_str(&format!(
            "<g transform=\"translate({x:.1},0)\"><rect x=\"0\" y=\"{y:.1}\" width=\"{rect_width:.1}\" height=\"{bar_height:.1}\" rx=\"12\" fill=\"{accent}\" fill-opacity=\"0.88\"/><text x=\"{text_x:.1}\" y=\"{value_y:.1}\" fill=\"#e2e8f0\" text-anchor=\"middle\" font-family=\"Inter,Segoe UI,sans-serif\" font-size=\"13\" font-weight=\"600\" letter-spacing=\"0\">{value}</text><text x=\"{text_x:.1}\" y=\"320\" fill=\"#94a3b8\" text-anchor=\"middle\" font-family=\"Inter,Segoe UI,sans-serif\" font-size=\"12\" letter-spacing=\"0\">{label}</text></g>",
            x = x,
            y = y,
            rect_width = rect_width,
            bar_height = bar_height,
            accent = accent,
            text_x = text_x,
            value_y = y - 10.0,
            value = escape_xml(&bar.value_label),
            label = escape_xml(&bar.label),
        ));
    }
    out
}

fn escape_xml(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\"', "&quot;")
        .replace('\'', "&apos;")
}
