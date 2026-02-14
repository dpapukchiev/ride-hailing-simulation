"""Parent/child Lambda handlers for serverless parameter sweeps.

This module keeps business logic in plain Python so it can be unit-tested
outside AWS Lambda and reused by local verification scripts.
"""

from __future__ import annotations

import hashlib
import itertools
import json
import math
import os
from dataclasses import dataclass
from datetime import UTC, datetime
from typing import Any, Callable

try:
    import boto3
except Exception:  # pragma: no cover - boto3 is available in Lambda runtime
    boto3 = None


MAX_DIMENSION_VALUES = 10_000
MAX_TOTAL_PARAMETER_POINTS = 200_000


class ValidationError(ValueError):
    """Raised when a sweep payload is malformed."""


@dataclass(frozen=True)
class ShardBounds:
    shard_id: int
    start_index: int
    end_index: int


def _stable_json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"))


def _response(status_code: int, payload: dict[str, Any]) -> dict[str, Any]:
    return {
        "statusCode": status_code,
        "headers": {"Content-Type": "application/json"},
        "body": json.dumps(payload),
    }


def _sha256_seed(*parts: str) -> int:
    digest = hashlib.sha256("|".join(parts).encode("utf-8")).digest()
    return int.from_bytes(digest[:8], byteorder="big", signed=False)


def _normalize_apigw_event(event: dict[str, Any]) -> dict[str, Any]:
    if "body" not in event:
        return event

    body = event.get("body")
    if body is None:
        return {}
    if isinstance(body, dict):
        return body
    if not isinstance(body, str):
        raise ValidationError("Request body must be a JSON object")
    try:
        return json.loads(body)
    except json.JSONDecodeError as exc:
        raise ValidationError(f"Malformed JSON body: {exc.msg}") from exc


def _required(payload: dict[str, Any], key: str, expected_type: type) -> Any:
    if key not in payload:
        raise ValidationError(f"Missing required field '{key}'")
    value = payload[key]
    if not isinstance(value, expected_type):
        raise ValidationError(
            f"Field '{key}' must be {expected_type.__name__}, got {type(value).__name__}"
        )
    return value


def validate_sweep_request(payload: dict[str, Any]) -> dict[str, Any]:
    run_id = _required(payload, "run_id", str).strip()
    if not run_id:
        raise ValidationError("run_id cannot be empty")

    dimensions = _required(payload, "dimensions", dict)
    if not dimensions:
        raise ValidationError("dimensions cannot be empty")

    for name, values in dimensions.items():
        if not isinstance(name, str) or not name.strip():
            raise ValidationError("dimension names must be non-empty strings")
        if not isinstance(values, list) or not values:
            raise ValidationError(f"Dimension '{name}' must be a non-empty list")
        if len(values) > MAX_DIMENSION_VALUES:
            raise ValidationError(
                f"Dimension '{name}' exceeds MAX_DIMENSION_VALUES={MAX_DIMENSION_VALUES}"
            )

    shard_count = payload.get("shard_count")
    shard_size = payload.get("shard_size")
    if shard_count is None and shard_size is None:
        raise ValidationError("Either shard_count or shard_size is required")
    if shard_count is not None and (
        not isinstance(shard_count, int) or shard_count <= 0
    ):
        raise ValidationError("shard_count must be a positive integer")
    if shard_size is not None and (not isinstance(shard_size, int) or shard_size <= 0):
        raise ValidationError("shard_size must be a positive integer")

    max_shards = payload.get("max_shards", 1_000)
    if not isinstance(max_shards, int) or max_shards <= 0:
        raise ValidationError("max_shards must be a positive integer")

    seed = payload.get("seed", 0)
    if not isinstance(seed, int):
        raise ValidationError("seed must be an integer")

    failure_injection_shards = payload.get("failure_injection_shards", [])
    if not isinstance(failure_injection_shards, list) or not all(
        isinstance(v, int) and v >= 0 for v in failure_injection_shards
    ):
        raise ValidationError(
            "failure_injection_shards must be a list of non-negative integers"
        )

    total_points = 1
    for values in dimensions.values():
        total_points *= len(values)
        if total_points > MAX_TOTAL_PARAMETER_POINTS:
            raise ValidationError(
                "Parameter space is too large for this demo deployment "
                f"(>{MAX_TOTAL_PARAMETER_POINTS} points)"
            )

    normalized = {
        "run_id": run_id,
        "dimensions": {k: dimensions[k] for k in sorted(dimensions.keys())},
        "total_points": total_points,
        "max_shards": max_shards,
        "seed": seed,
        "failure_injection_shards": sorted(set(failure_injection_shards)),
    }
    if shard_count is not None:
        normalized["shard_count"] = shard_count
    if shard_size is not None:
        normalized["shard_size"] = shard_size
    return normalized


def compute_shard_bounds(request: dict[str, Any]) -> list[ShardBounds]:
    total_points = request["total_points"]
    shard_count = request.get("shard_count")
    shard_size = request.get("shard_size")

    if shard_count is None:
        shard_count = math.ceil(total_points / shard_size)
    else:
        shard_count = min(shard_count, total_points)

    if shard_count <= 0:
        raise ValidationError("No shards to process")
    if shard_count > request["max_shards"]:
        raise ValidationError(
            f"Computed shard count {shard_count} exceeds max_shards={request['max_shards']}"
        )

    base_size = total_points // shard_count
    remainder = total_points % shard_count
    bounds: list[ShardBounds] = []
    cursor = 0
    for shard_id in range(shard_count):
        current_size = base_size + (1 if shard_id < remainder else 0)
        start = cursor
        end = cursor + current_size
        bounds.append(ShardBounds(shard_id=shard_id, start_index=start, end_index=end))
        cursor = end

    if bounds and (bounds[0].start_index != 0 or bounds[-1].end_index != total_points):
        raise RuntimeError("Shard boundaries do not cover full parameter space")
    for idx in range(1, len(bounds)):
        prev = bounds[idx - 1]
        current = bounds[idx]
        if prev.end_index != current.start_index:
            raise RuntimeError("Shard boundaries overlap or leave gaps")
    return bounds


def _dimension_product(dimensions: dict[str, list[Any]]) -> list[dict[str, Any]]:
    names = list(dimensions.keys())
    values = [dimensions[name] for name in names]
    return [dict(zip(names, combo)) for combo in itertools.product(*values)]


def _default_lambda_client():
    if boto3 is None:
        raise RuntimeError("boto3 is required for live Lambda dispatch")
    return boto3.client("lambda")


def parent_handler(
    event: dict[str, Any],
    _context: Any,
    lambda_client: Any = None,
) -> dict[str, Any]:
    """Validates a sweep request, shards it, and asynchronously dispatches children."""
    try:
        payload = _normalize_apigw_event(event)
        request = validate_sweep_request(payload)
    except ValidationError as exc:
        return _response(400, {"error": "validation_error", "message": str(exc)})

    child_arn = os.environ.get("CHILD_LAMBDA_ARN")
    if not child_arn:
        return _response(
            500,
            {
                "error": "misconfiguration",
                "message": "CHILD_LAMBDA_ARN must be configured",
            },
        )

    shard_bounds = compute_shard_bounds(request)
    lambda_client = lambda_client or _default_lambda_client()
    dispatches = []

    for shard in shard_bounds:
        child_payload = {
            "run_id": request["run_id"],
            "dimensions": request["dimensions"],
            "total_points": request["total_points"],
            "shard_id": shard.shard_id,
            "start_index": shard.start_index,
            "end_index": shard.end_index,
            "seed": request["seed"],
            "failure_injection_shards": request["failure_injection_shards"],
        }
        invocation = lambda_client.invoke(
            FunctionName=child_arn,
            InvocationType="Event",
            Payload=_stable_json(child_payload).encode("utf-8"),
        )
        dispatches.append(
            {
                "shard_id": shard.shard_id,
                "status_code": invocation.get("StatusCode", 0),
            }
        )

    return _response(
        202,
        {
            "run_id": request["run_id"],
            "total_points": request["total_points"],
            "shards_dispatched": len(shard_bounds),
            "dispatches": dispatches,
            "status": "dispatch_submitted",
        },
    )


def _default_s3_writer(bucket: str, key: str, body: str) -> None:
    if boto3 is None:
        raise RuntimeError("boto3 is required for S3 writes")
    boto3.client("s3").put_object(
        Bucket=bucket,
        Key=key,
        Body=body.encode("utf-8"),
        ContentType="application/json",
    )


def _partitioned_outcome_key(
    run_id: str, status: str, shard_id: int, prefix: str
) -> str:
    date = datetime.now(UTC).strftime("%Y-%m-%d")
    base_prefix = prefix.strip("/")
    return (
        f"{base_prefix}/run_id={run_id}/status={status}/date={date}/"
        f"shard_id={shard_id}.json"
    )


def _deterministic_result(
    run_id: str, shard_id: int, point: dict[str, Any], seed: int
) -> dict[str, Any]:
    point_json = _stable_json(point)
    state = _sha256_seed(run_id, str(shard_id), point_json, str(seed))
    completed_rides = (state % 200) + 10
    abandoned_rides = state // 10 % 40
    revenue_cents = completed_rides * (500 + (state % 1200))
    return {
        "completed_rides": completed_rides,
        "abandoned_rides": abandoned_rides,
        "platform_revenue_cents": revenue_cents,
    }


def _run_shard(payload: dict[str, Any]) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    parameter_points = _dimension_product(payload["dimensions"])
    start_index = payload["start_index"]
    end_index = payload["end_index"]
    if start_index < 0 or end_index > len(parameter_points) or start_index >= end_index:
        raise ValidationError("Invalid shard bounds")

    shard_points = parameter_points[start_index:end_index]
    aggregate = {
        "points_processed": len(shard_points),
        "completed_rides": 0,
        "abandoned_rides": 0,
        "platform_revenue_cents": 0,
    }
    for point in shard_points:
        metrics = _deterministic_result(
            payload["run_id"],
            payload["shard_id"],
            point,
            payload["seed"],
        )
        aggregate["completed_rides"] += metrics["completed_rides"]
        aggregate["abandoned_rides"] += metrics["abandoned_rides"]
        aggregate["platform_revenue_cents"] += metrics["platform_revenue_cents"]

    summary = {
        "run_id": payload["run_id"],
        "shard_id": payload["shard_id"],
        "start_index": start_index,
        "end_index": end_index,
        "metrics": aggregate,
    }
    return shard_points, summary


def _outcome_record(
    payload: dict[str, Any],
    status: str,
    output_metadata: dict[str, Any] | None = None,
    error: Exception | None = None,
) -> dict[str, Any]:
    record = {
        "run_id": payload["run_id"],
        "shard_id": payload["shard_id"],
        "status": status,
        "start_index": payload["start_index"],
        "end_index": payload["end_index"],
        "event_time": datetime.now(UTC).isoformat(),
        "record_schema": "v1",
    }
    if output_metadata is not None:
        record["output_metadata"] = output_metadata
    if error is not None:
        record["error_code"] = type(error).__name__
        record["error_message"] = str(error)
    return record


def child_handler(
    event: dict[str, Any],
    _context: Any,
    outcome_writer: Callable[[str, str, str], None] | None = None,
) -> dict[str, Any]:
    """Executes the assigned shard and writes success/failure outcomes to S3."""
    payload = event
    required_keys = {
        "run_id": str,
        "dimensions": dict,
        "total_points": int,
        "shard_id": int,
        "start_index": int,
        "end_index": int,
        "seed": int,
        "failure_injection_shards": list,
    }
    for key, expected_type in required_keys.items():
        _required(payload, key, expected_type)

    bucket = os.environ.get("SWEEP_RESULTS_BUCKET")
    prefix = os.environ.get("SWEEP_RESULTS_PREFIX", "serverless-sweeps/outcomes")
    if not bucket:
        raise RuntimeError("SWEEP_RESULTS_BUCKET must be configured")

    write_outcome = outcome_writer or _default_s3_writer

    try:
        if payload["shard_id"] in payload["failure_injection_shards"]:
            raise RuntimeError("Injected shard failure for verification")

        shard_points, shard_summary = _run_shard(payload)
        result_key = _partitioned_outcome_key(
            payload["run_id"], "success", payload["shard_id"], prefix
        )
        success_record = _outcome_record(
            payload,
            status="success",
            output_metadata={
                "result_key": result_key,
                "points_processed": len(shard_points),
                "summary": shard_summary,
                "format": "parquet-compatible-json",
            },
        )
        write_outcome(bucket, result_key, _stable_json(success_record))
        return {
            "status": "ok",
            "shard_id": payload["shard_id"],
            "outcome_key": result_key,
        }
    except Exception as exc:
        failure_key = _partitioned_outcome_key(
            payload["run_id"], "failure", payload["shard_id"], prefix
        )
        failure_record = _outcome_record(payload, status="failure", error=exc)
        write_outcome(bucket, failure_key, _stable_json(failure_record))
        raise
