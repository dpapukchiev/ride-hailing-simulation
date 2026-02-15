#!/usr/bin/env python3
"""Apply Athena SQL files in an ordered local workflow.

This script executes SQL files listed in a manifest (plan) using AWS CLI Athena APIs,
waiting for each statement to finish before moving to the next one.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from pathlib import Path
from urllib.parse import urlparse


def parse_args() -> argparse.Namespace:
    script_dir = Path(__file__).resolve().parent
    parser = argparse.ArgumentParser(
        description="Apply ordered Athena SQL files via AWS CLI",
    )
    parser.add_argument(
        "--plan-file",
        default=str(script_dir / "athena_bootstrap.plan"),
        help="Path to ordered SQL plan file (default: athena_bootstrap.plan)",
    )
    parser.add_argument(
        "--query-results-s3",
        required=True,
        help="Athena output S3 location, e.g. s3://my-bucket/athena-query-results/",
    )
    parser.add_argument(
        "--results-bucket",
        required=True,
        help="S3 bucket (or s3://bucket[/prefix]) containing serverless sweep outcome datasets",
    )
    parser.add_argument(
        "--results-prefix",
        default="serverless-sweeps/outcomes",
        help="S3 prefix containing outcome datasets (default: serverless-sweeps/outcomes)",
    )
    parser.add_argument(
        "--database",
        default="ride_sim_analytics",
        help="Athena database name to substitute into SQL templates",
    )
    parser.add_argument(
        "--workgroup",
        default="primary",
        help="Athena workgroup to execute queries in",
    )
    parser.add_argument(
        "--region",
        default=None,
        help="AWS region override (optional if configured via AWS CLI)",
    )
    parser.add_argument(
        "--poll-interval-seconds",
        type=float,
        default=2.0,
        help="Polling interval while waiting for query completion",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print resolved SQL execution order without calling AWS",
    )
    return parser.parse_args()


def normalize_s3_inputs(results_bucket: str, results_prefix: str) -> tuple[str, str]:
    bucket_input = results_bucket.strip()
    if not bucket_input:
        raise ValueError("--results-bucket cannot be empty")

    normalized_prefix = results_prefix.strip("/")
    if bucket_input.startswith("s3://"):
        parsed = urlparse(bucket_input)
        if parsed.scheme != "s3" or not parsed.netloc:
            raise ValueError(
                "--results-bucket must be a bucket name or valid s3://bucket[/prefix] URI"
            )

        bucket_name = parsed.netloc.strip()
        uri_prefix = parsed.path.strip("/")
        if uri_prefix:
            normalized_prefix = "/".join(
                part for part in (uri_prefix, normalized_prefix) if part
            )
        return bucket_name, normalized_prefix

    if "/" in bucket_input:
        raise ValueError(
            "--results-bucket should be a bucket name (no slashes) unless using s3://bucket[/prefix]"
        )

    return bucket_input, normalized_prefix


def read_plan(plan_file: Path) -> list[Path]:
    if not plan_file.exists():
        raise FileNotFoundError(f"Plan file not found: {plan_file}")

    sql_paths: list[Path] = []
    for line in plan_file.read_text(encoding="utf-8").splitlines():
        entry = line.strip()
        if not entry or entry.startswith("#"):
            continue

        candidate = (
            (plan_file.parent / entry).resolve()
            if not Path(entry).is_absolute()
            else Path(entry)
        )
        if candidate.suffix.lower() != ".sql":
            raise ValueError(f"Plan entries must point to .sql files, got: {entry}")
        if not candidate.exists():
            raise FileNotFoundError(f"SQL file in plan does not exist: {candidate}")
        sql_paths.append(candidate)

    if not sql_paths:
        raise ValueError("Plan file contains no SQL entries")

    return sql_paths


def apply_substitutions(
    sql_text: str, *, results_bucket: str, results_prefix: str, database: str
) -> str:
    substitutions = {
        "<results-bucket>": results_bucket,
        "ride_sim_analytics": database,
        "serverless-sweeps/outcomes": results_prefix,
    }

    rendered = sql_text
    for needle, replacement in substitutions.items():
        rendered = rendered.replace(needle, replacement)

    return rendered


def run_aws_json(command: list[str]) -> dict:
    completed = subprocess.run(command, capture_output=True, text=True, check=False)
    if completed.returncode != 0:
        stderr = completed.stderr.strip()
        stdout = completed.stdout.strip()
        detail = stderr or stdout or "unknown AWS CLI error"
        raise RuntimeError(f"AWS CLI command failed: {' '.join(command)}\n{detail}")
    return json.loads(completed.stdout)


def start_query(
    sql: str, *, workgroup: str, query_results_s3: str, region: str | None
) -> str:
    cmd = [
        "aws",
        "athena",
        "start-query-execution",
        "--work-group",
        workgroup,
        "--result-configuration",
        f"OutputLocation={query_results_s3}",
        "--query-string",
        sql,
        "--output",
        "json",
    ]
    if region:
        cmd.extend(["--region", region])

    payload = run_aws_json(cmd)
    query_execution_id = payload.get("QueryExecutionId")
    if not query_execution_id:
        raise RuntimeError(f"Missing QueryExecutionId in Athena response: {payload}")
    return query_execution_id


def wait_for_query(
    query_execution_id: str, *, poll_interval: float, region: str | None
) -> tuple[str, str]:
    while True:
        cmd = [
            "aws",
            "athena",
            "get-query-execution",
            "--query-execution-id",
            query_execution_id,
            "--output",
            "json",
        ]
        if region:
            cmd.extend(["--region", region])

        payload = run_aws_json(cmd)
        status = payload["QueryExecution"]["Status"].get("State", "UNKNOWN")
        reason = payload["QueryExecution"]["Status"].get("StateChangeReason", "")

        if status in {"SUCCEEDED", "FAILED", "CANCELLED"}:
            return status, reason

        time.sleep(poll_interval)


def main() -> int:
    args = parse_args()
    plan_file = Path(args.plan_file).resolve()

    try:
        resolved_results_bucket, resolved_results_prefix = normalize_s3_inputs(
            args.results_bucket,
            args.results_prefix,
        )
    except ValueError as error:
        print(f"Argument error: {error}", file=sys.stderr)
        return 2

    try:
        sql_paths = read_plan(plan_file)
    except (FileNotFoundError, ValueError) as error:
        print(f"Plan error: {error}", file=sys.stderr)
        return 2

    print(f"Using plan: {plan_file}")
    print("Execution order:")
    for idx, sql_path in enumerate(sql_paths, start=1):
        print(f"  {idx:02d}. {sql_path}")

    if args.dry_run:
        print("Dry run only; no Athena queries executed.")
        return 0

    for idx, sql_path in enumerate(sql_paths, start=1):
        sql_template = sql_path.read_text(encoding="utf-8")
        sql = apply_substitutions(
            sql_template,
            results_bucket=resolved_results_bucket,
            results_prefix=resolved_results_prefix,
            database=args.database,
        )

        print(f"[{idx:02d}/{len(sql_paths):02d}] Executing {sql_path.name}...")
        try:
            query_execution_id = start_query(
                sql,
                workgroup=args.workgroup,
                query_results_s3=args.query_results_s3,
                region=args.region,
            )
            print(f"    QueryExecutionId={query_execution_id}")
            state, reason = wait_for_query(
                query_execution_id,
                poll_interval=args.poll_interval_seconds,
                region=args.region,
            )
        except RuntimeError as error:
            print(
                f"Athena invocation failed for {sql_path.name}: {error}",
                file=sys.stderr,
            )
            return 1

        if state != "SUCCEEDED":
            print(
                f"Athena query failed for {sql_path.name}: state={state}, "
                f"query_execution_id={query_execution_id}, reason={reason}",
                file=sys.stderr,
            )
            return 1

        print(f"    {sql_path.name} -> SUCCEEDED")

    print("Athena bootstrap completed successfully.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
