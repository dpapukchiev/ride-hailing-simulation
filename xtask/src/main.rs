use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{exit, Command, ExitStatus};

use clap::{Parser, Subcommand, ValueEnum};
use zip::write::FileOptions;
use zip::{CompressionMethod, ZipWriter};

// ── CLI definition ─────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "xtask",
    about = "Task runner for the ride-hailing simulation workspace",
    long_about = "A unified CLI for running simulations, experiments, benchmarks,\n\
                  and CI checks in the ride-hailing simulation workspace."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Launch the simulation GUI
    Ui,
    /// Run the standard scenario (500 riders, 100 drivers)
    Run,
    /// Run the large scenario (10 000 riders, 7 000 drivers)
    RunLarge,
    /// Run a parameter sweep experiment
    Sweep,
    /// Export a precomputed route table to JSON
    RouteExport {
        /// Number of origin-destination samples
        #[arg(long, default_value_t = 100)]
        sample_count: usize,
        /// Output file path
        #[arg(long, default_value = "route_table.json")]
        output: String,
    },
    /// Run Criterion benchmarks
    Bench,
    /// Compare benchmarks: stash changes, create baseline, restore, compare
    BenchCompare,
    /// Run CI checks (fmt, clippy, tests, examples, benchmarks)
    Ci {
        /// Job to run
        #[arg(value_enum, default_value_t = CiJob::Check)]
        job: CiJob,
    },
    /// Run load tests (ignored tests in sim_core)
    LoadTest,
    /// Build and package Rust Lambda artifacts for Terraform inputs
    ServerlessPackage {
        /// Compilation target triple for Lambda binaries
        #[arg(long, default_value = "x86_64-unknown-linux-gnu")]
        target: String,
        /// Build profile used for binaries
        #[arg(value_enum, long, default_value_t = BuildProfile::Release)]
        profile: BuildProfile,
    },
    /// Ensure Athena metadata/partitions are ready and validate run results
    PostRunIngest {
        /// Target run identifier to validate
        #[arg(long)]
        run_id: String,
        /// Athena database containing the sweep tables
        #[arg(long, default_value = "ride_sim_analytics")]
        athena_db: String,
        /// Athena workgroup for query execution
        #[arg(long, default_value = "primary")]
        athena_workgroup: String,
        /// S3 URI where Athena writes query outputs (s3://bucket/prefix)
        #[arg(long, env = "ATHENA_QUERY_OUTPUT")]
        athena_query_output: String,
        /// Results bucket used by external table LOCATION clauses
        #[arg(long, env = "SWEEP_RESULTS_BUCKET")]
        results_bucket: String,
        /// Results prefix used by external table LOCATION clauses
        #[arg(long, default_value = "serverless-sweeps/outcomes")]
        results_prefix: String,
        /// Optional expected number of shards for full-coverage validation
        #[arg(long)]
        expected_shards: Option<u64>,
    },
}

#[derive(Clone, ValueEnum)]
enum CiJob {
    /// Formatting, clippy, and tests
    Check,
    /// Build and run example scenarios
    Examples,
    /// Run benchmarks
    Bench,
    /// Run check + examples + bench
    All,
}

#[derive(Clone, Copy, ValueEnum)]
enum BuildProfile {
    Debug,
    Release,
}

impl BuildProfile {
    fn dir_name(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Release => "release",
        }
    }

    fn as_cargo_flag(self) -> Option<&'static str> {
        match self {
            Self::Debug => None,
            Self::Release => Some("--release"),
        }
    }
}

// ── helpers ────────────────────────────────────────────────────────

fn step(label: &str) {
    eprintln!("\n=== {label} ===");
}

fn cargo(args: &[&str]) -> ExitStatus {
    eprintln!("+ cargo {}", args.join(" "));
    Command::new("cargo")
        .args(args)
        .status()
        .expect("failed to execute cargo")
}

fn git(args: &[&str]) -> ExitStatus {
    eprintln!("+ git {}", args.join(" "));
    Command::new("git")
        .args(args)
        .status()
        .expect("failed to execute git")
}

fn run_cargo(args: &[&str]) {
    let status = cargo(args);
    if !status.success() {
        exit(status.code().unwrap_or(1));
    }
}

fn run_git(args: &[&str]) {
    let status = git(args);
    if !status.success() {
        exit(status.code().unwrap_or(1));
    }
}

fn run_command(program: &str, args: &[&str]) -> String {
    eprintln!("+ {program} {}", args.join(" "));
    let output = Command::new(program)
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("failed to execute {program}: {error}"));

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!(
            "command `{program} {}` failed: {}",
            args.join(" "),
            stderr.trim()
        );
    }

    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn ensure_tool_available(tool: &str) {
    let status = Command::new(tool).arg("--version").status();
    match status {
        Ok(ok) if ok.success() => {}
        Ok(_) | Err(_) => panic!("required tool `{tool}` is not available on PATH"),
    }
}

fn sanitize_sql_literal(value: &str) -> String {
    value.replace("'", "''")
}

fn run_athena_query(query: &str, database: &str, workgroup: &str, output: &str) -> String {
    let query_id = run_command(
        "aws",
        &[
            "athena",
            "start-query-execution",
            "--query-string",
            query,
            "--query-execution-context",
            &format!("Database={database}"),
            "--work-group",
            workgroup,
            "--result-configuration",
            &format!("OutputLocation={output}"),
            "--query",
            "QueryExecutionId",
            "--output",
            "text",
        ],
    );

    loop {
        let state = run_command(
            "aws",
            &[
                "athena",
                "get-query-execution",
                "--query-execution-id",
                &query_id,
                "--query",
                "QueryExecution.Status.State",
                "--output",
                "text",
            ],
        );

        match state.as_str() {
            "SUCCEEDED" => return query_id,
            "FAILED" | "CANCELLED" => {
                let reason = run_command(
                    "aws",
                    &[
                        "athena",
                        "get-query-execution",
                        "--query-execution-id",
                        &query_id,
                        "--query",
                        "QueryExecution.Status.StateChangeReason",
                        "--output",
                        "text",
                    ],
                );
                panic!("athena query failed ({state}): {reason}");
            }
            _ => std::thread::sleep(std::time::Duration::from_millis(1_000)),
        }
    }
}

fn query_scalar(query: &str, database: &str, workgroup: &str, output: &str) -> String {
    let query_id = run_athena_query(query, database, workgroup, output);
    run_command(
        "aws",
        &[
            "athena",
            "get-query-results",
            "--query-execution-id",
            &query_id,
            "--query",
            "ResultSet.Rows[1].Data[0].VarCharValue",
            "--output",
            "text",
        ],
    )
}

fn apply_athena_template(sql: &str, db: &str, bucket: &str, prefix: &str) -> String {
    sql.replace("ride_sim_analytics", db)
        .replace("<results-bucket>", bucket)
        .replace("serverless-sweeps/outcomes", prefix.trim_matches('/'))
}

fn run_sql_file(path: &str, db: &str, workgroup: &str, output: &str, bucket: &str, prefix: &str) {
    let template =
        fs::read_to_string(path).unwrap_or_else(|error| panic!("failed to read {path}: {error}"));
    let sql = apply_athena_template(&template, db, bucket, prefix);
    step(&format!("Athena SQL: {path}"));
    run_athena_query(&sql, db, workgroup, output);
}

fn run_post_ingest(
    run_id: &str,
    db: &str,
    workgroup: &str,
    output: &str,
    bucket: &str,
    prefix: &str,
    expected_shards: Option<u64>,
) {
    ensure_tool_available("aws");

    step("Bootstrap Athena database/tables");
    let create_files = [
        "infra/aws_serverless_sweep/athena/create_database.sql",
        "infra/aws_serverless_sweep/athena/create_table.sql",
        "infra/aws_serverless_sweep/athena/create_table_shard_metrics.sql",
        "infra/aws_serverless_sweep/athena/create_table_trip_data.sql",
        "infra/aws_serverless_sweep/athena/create_table_snapshot_counts.sql",
    ];
    for path in create_files {
        run_sql_file(path, db, workgroup, output, bucket, prefix);
    }

    step("Load partitions");
    let repair_files = [
        "infra/aws_serverless_sweep/athena/repair_table.sql",
        "infra/aws_serverless_sweep/athena/repair_table_shard_metrics.sql",
        "infra/aws_serverless_sweep/athena/repair_table_trip_data.sql",
        "infra/aws_serverless_sweep/athena/repair_table_snapshot_counts.sql",
    ];
    for path in repair_files {
        run_sql_file(path, db, workgroup, output, bucket, prefix);
    }

    step("Validate run readiness");
    let run_id_sql = sanitize_sql_literal(run_id);
    let outcomes_count = query_scalar(
        &format!(
            "SELECT CAST(COUNT(*) AS VARCHAR) FROM {db}.sweep_shard_outcomes WHERE run_id_partition = '{run_id_sql}'"
        ),
        db,
        workgroup,
        output,
    )
    .parse::<u64>()
    .unwrap_or(0);
    let distinct_shards = query_scalar(
        &format!(
            "SELECT CAST(COUNT(DISTINCT shard_id_partition) AS VARCHAR) FROM {db}.sweep_shard_outcomes WHERE run_id_partition = '{run_id_sql}'"
        ),
        db,
        workgroup,
        output,
    )
    .parse::<u64>()
    .unwrap_or(0);
    let successful_shards = query_scalar(
        &format!(
            "SELECT CAST(COUNT(*) AS VARCHAR) FROM {db}.sweep_shard_outcomes WHERE run_id_partition = '{run_id_sql}' AND status_partition = 'success'"
        ),
        db,
        workgroup,
        output,
    )
    .parse::<u64>()
    .unwrap_or(0);
    let failed_shards = query_scalar(
        &format!(
            "SELECT CAST(COUNT(*) AS VARCHAR) FROM {db}.sweep_shard_outcomes WHERE run_id_partition = '{run_id_sql}' AND status_partition = 'failure'"
        ),
        db,
        workgroup,
        output,
    )
    .parse::<u64>()
    .unwrap_or(0);
    let metrics_rows = query_scalar(
        &format!(
            "SELECT CAST(COUNT(*) AS VARCHAR) FROM {db}.sweep_shard_metrics WHERE run_id = '{run_id_sql}'"
        ),
        db,
        workgroup,
        output,
    )
    .parse::<u64>()
    .unwrap_or(0);
    let trip_rows = query_scalar(
        &format!(
            "SELECT CAST(COUNT(*) AS VARCHAR) FROM {db}.sweep_trip_data WHERE run_id = '{run_id_sql}'"
        ),
        db,
        workgroup,
        output,
    )
    .parse::<u64>()
    .unwrap_or(0);
    let snapshot_rows = query_scalar(
        &format!(
            "SELECT CAST(COUNT(*) AS VARCHAR) FROM {db}.sweep_snapshot_counts WHERE run_id = '{run_id_sql}'"
        ),
        db,
        workgroup,
        output,
    )
    .parse::<u64>()
    .unwrap_or(0);

    let missing_shards = if let Some(expected) = expected_shards {
        query_scalar(
            &format!(
                "WITH expected AS (SELECT sequence(0, {expected} - 1) AS ids), observed AS (
                    SELECT CAST(shard_id_partition AS BIGINT) AS shard_id
                    FROM {db}.sweep_shard_outcomes
                    WHERE run_id_partition = '{run_id_sql}'
                )
                SELECT array_join(transform(filter((SELECT ids FROM expected), x -> x NOT IN (SELECT shard_id FROM observed)), x -> CAST(x AS VARCHAR)), ',')"
            ),
            db,
            workgroup,
            output,
        )
    } else {
        String::new()
    };

    let mut errors = Vec::new();
    if outcomes_count == 0 {
        errors.push("no shard_outcomes rows for run_id".to_string());
    }
    if metrics_rows == 0 {
        errors.push("no shard_metrics rows for run_id".to_string());
    }
    if trip_rows == 0 {
        errors.push("no trip_data rows for run_id".to_string());
    }
    if snapshot_rows == 0 {
        errors.push("no snapshot_counts rows for run_id".to_string());
    }
    if let Some(expected) = expected_shards {
        if distinct_shards != expected {
            errors.push(format!(
                "shard coverage mismatch: observed {distinct_shards}, expected {expected}"
            ));
        }
    }

    eprintln!("\nPost-run ingestion summary:");
    eprintln!("- run_id: {run_id}");
    eprintln!("- shard_outcomes rows: {outcomes_count}");
    eprintln!("- successful shard outcomes: {successful_shards}");
    eprintln!("- failed shard outcomes: {failed_shards}");
    eprintln!("- distinct shards observed: {distinct_shards}");
    eprintln!("- shard_metrics rows: {metrics_rows}");
    eprintln!("- trip_data rows: {trip_rows}");
    eprintln!("- snapshot_counts rows: {snapshot_rows}");
    if !missing_shards.is_empty() && missing_shards != "None" {
        eprintln!("- missing shard ids: {missing_shards}");
    }

    if !errors.is_empty() {
        eprintln!("\nReadiness check failed:");
        for error in errors {
            eprintln!("  - {error}");
        }
        eprintln!(
            "\nNext steps: rerun partition repair and inspect infra/aws_serverless_sweep/athena/query_failure_diagnostics.sql"
        );
        exit(1);
    }

    eprintln!("\nReadiness check passed.");
}

fn package_serverless_lambdas(target: &str, profile: BuildProfile) {
    ensure_rust_target_installed(target);
    ensure_c_linker_available(target);

    step("Build serverless lambda binaries");

    let mut cargo_args = vec![
        "build",
        "-p",
        "sim_serverless_sweep_lambda",
        "--target",
        target,
        "--bin",
        "parent_lambda",
        "--bin",
        "child_lambda",
    ];
    if let Some(flag) = profile.as_cargo_flag() {
        cargo_args.push(flag);
    }
    run_cargo(&cargo_args);

    step("Package Terraform lambda zip artifacts");
    let profile_dir = profile.dir_name();
    let target_dir = Path::new("target").join(target).join(profile_dir);
    let dist_dir = Path::new("infra/aws_serverless_sweep/dist");
    fs::create_dir_all(dist_dir).expect("failed to create lambda dist directory");

    package_lambda_zip(
        &target_dir.join(binary_name("parent_lambda", target)),
        &dist_dir.join("parent.zip"),
    );
    package_lambda_zip(
        &target_dir.join(binary_name("child_lambda", target)),
        &dist_dir.join("child.zip"),
    );

    eprintln!(
        "\nPackaged artifacts:\n- {}\n- {}",
        dist_dir.join("parent.zip").display(),
        dist_dir.join("child.zip").display()
    );
}

fn ensure_rust_target_installed(target: &str) {
    let output = Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output();

    let output = match output {
        Ok(value) => value,
        Err(error) => {
            eprintln!(
                "warning: failed to run `rustup target list --installed` ({error}); continuing without target preflight"
            );
            return;
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!(
            "failed to list installed rust targets; run `rustup target list --installed` manually. details: {}",
            stderr.trim()
        );
    }

    let installed = String::from_utf8_lossy(&output.stdout);
    if !installed.lines().any(|line| line.trim() == target) {
        panic!(
            "required rust target `{target}` is not installed. install it with `rustup target add {target}` and re-run `cargo run -p xtask -- serverless-package`"
        );
    }
}

fn ensure_c_linker_available(target: &str) {
    if !cfg!(windows) || !target.ends_with("unknown-linux-gnu") {
        return;
    }

    let env_override_keys = [
        format!("CC_{}", target.replace('-', "_")),
        format!("CC_{target}"),
        "TARGET_CC".to_string(),
        "CC".to_string(),
    ];

    for key in env_override_keys {
        if let Ok(value) = std::env::var(&key) {
            let candidate = value.trim();
            if candidate.is_empty() {
                continue;
            }
            if tool_works(candidate) {
                return;
            }
        }
    }

    let canonical = "x86_64-linux-gnu-gcc";
    if tool_works(canonical) {
        return;
    }

    panic!(
        "missing C cross-linker for target `{target}`. install `{canonical}` (or set CC_x86_64_unknown_linux_gnu) before running `cargo run -p xtask -- serverless-package`.\n\
         Tip: crates in this workspace (for example zstd-sys via parquet) require a Linux C toolchain when cross-compiling from Windows."
    );
}

fn tool_works(program: &str) -> bool {
    let mut parts = program.split_whitespace();
    let Some(bin) = parts.next() else {
        return false;
    };
    let args: Vec<&str> = parts.collect();

    Command::new(bin)
        .args(&args)
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn binary_name(bin_name: &str, target: &str) -> String {
    if target.contains("windows") {
        format!("{bin_name}.exe")
    } else {
        bin_name.to_string()
    }
}

fn package_lambda_zip(binary_path: &Path, zip_path: &Path) {
    if !binary_path.exists() {
        panic!("expected lambda binary at '{}'", binary_path.display());
    }

    let binary = fs::read(binary_path).expect("failed to read lambda binary");
    let file = fs::File::create(zip_path).expect("failed to create lambda zip");
    let mut zip = ZipWriter::new(file);
    let options = FileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o755);
    zip.start_file("bootstrap", options)
        .expect("failed to start bootstrap entry in lambda zip");
    zip.write_all(&binary)
        .expect("failed to write bootstrap entry");
    zip.finish().expect("failed to finish lambda zip");
}

// ── CI jobs ────────────────────────────────────────────────────────

fn ci_check() {
    step("Check formatting");
    run_cargo(&["fmt", "--all", "--", "--check"]);

    step("Clippy");
    run_cargo(&[
        "clippy",
        "--all-targets",
        "--all-features",
        "--",
        "-D",
        "warnings",
    ]);

    step("Test sim_core");
    run_cargo(&["test", "-p", "sim_core"]);

    step("Test sim_experiments");
    run_cargo(&["test", "-p", "sim_experiments"]);
}

fn ci_examples() {
    step("Run scenario_run (500 riders, 100 drivers)");
    run_cargo(&[
        "run",
        "-p",
        "sim_core",
        "--example",
        "scenario_run",
        "--release",
    ]);

    step("Run scenario_run_large (10K riders, 7K drivers)");
    run_cargo(&[
        "run",
        "-p",
        "sim_core",
        "--example",
        "scenario_run_large",
        "--release",
    ]);
}

fn ci_bench() {
    step("Run benchmarks");
    run_cargo(&["bench", "--package", "sim_core", "--bench", "performance"]);
}

// ── main ───────────────────────────────────────────────────────────

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Ui => {
            run_cargo(&["run", "-p", "sim_ui"]);
        }
        Commands::Run => {
            run_cargo(&[
                "run",
                "-p",
                "sim_core",
                "--example",
                "scenario_run",
                "--release",
            ]);
        }
        Commands::RunLarge => {
            run_cargo(&[
                "run",
                "-p",
                "sim_core",
                "--example",
                "scenario_run_large",
                "--release",
            ]);
        }
        Commands::Sweep => {
            run_cargo(&[
                "run",
                "-p",
                "sim_experiments",
                "--example",
                "parameter_sweep",
            ]);
        }
        Commands::RouteExport {
            sample_count,
            output,
        } => {
            let sc = sample_count.to_string();
            run_cargo(&[
                "run",
                "-p",
                "sim_experiments",
                "--example",
                "route_export",
                "--",
                "--sample-count",
                &sc,
                "--output",
                &output,
            ]);
        }
        Commands::Bench => {
            run_cargo(&["bench", "--package", "sim_core", "--bench", "performance"]);
        }
        Commands::BenchCompare => {
            let baseline_dir = Path::new("target/criterion");
            if baseline_dir.exists() {
                step("Removing existing benchmark data");
                std::fs::remove_dir_all(baseline_dir).expect("failed to remove target/criterion");
            }

            step("Stashing current changes");
            run_git(&[
                "stash",
                "push",
                "-m",
                "Temporary stash for benchmark comparison",
            ]);

            step("Running benchmark to create baseline");
            run_cargo(&[
                "bench",
                "--package",
                "sim_core",
                "--bench",
                "performance",
                "--",
                "--save-baseline",
                "main",
            ]);

            step("Reapplying changes");
            run_git(&["stash", "pop"]);

            step("Running benchmark comparing against baseline");
            run_cargo(&[
                "bench",
                "--package",
                "sim_core",
                "--bench",
                "performance",
                "--",
                "--baseline",
                "main",
            ]);

            eprintln!("\nDone! Check the output above to see performance comparison.");
        }
        Commands::Ci { job } => {
            match job {
                CiJob::Check => ci_check(),
                CiJob::Examples => ci_examples(),
                CiJob::Bench => ci_bench(),
                CiJob::All => {
                    ci_check();
                    ci_examples();
                    ci_bench();
                }
            }
            eprintln!("\nCI job passed.");
        }
        Commands::LoadTest => {
            run_cargo(&[
                "test",
                "-p",
                "sim_core",
                "--test",
                "load_tests",
                "--",
                "--ignored",
            ]);
        }
        Commands::ServerlessPackage { target, profile } => {
            package_serverless_lambdas(&target, profile);
        }
        Commands::PostRunIngest {
            run_id,
            athena_db,
            athena_workgroup,
            athena_query_output,
            results_bucket,
            results_prefix,
            expected_shards,
        } => {
            run_post_ingest(
                &run_id,
                &athena_db,
                &athena_workgroup,
                &athena_query_output,
                &results_bucket,
                &results_prefix,
                expected_shards,
            );
        }
    }
}
