"""Local end-to-end verification for mixed shard outcomes.

This script exercises parent fan-out + child execution with one injected failure,
then verifies full shard coverage in the captured outcome records.
"""

import json
import os
import sys
from pathlib import Path

MODULE_DIR = Path(__file__).resolve().parents[1] / "lambda"
sys.path.insert(0, str(MODULE_DIR))

from serverless_sweep import child_handler, parent_handler


class InMemoryLambdaClient:
    def __init__(self):
        self.events = []

    def invoke(self, **kwargs):
        payload = json.loads(kwargs["Payload"].decode("utf-8"))
        self.events.append(payload)
        return {"StatusCode": 202}


def main() -> int:
    os.environ["CHILD_LAMBDA_ARN"] = "arn:aws:lambda:region:acct:function:child"
    os.environ["SWEEP_RESULTS_BUCKET"] = "local-bucket"
    os.environ["SWEEP_RESULTS_PREFIX"] = "serverless-sweeps/outcomes"

    client = InMemoryLambdaClient()
    writes = []

    def outcome_writer(bucket: str, key: str, body: str):
        writes.append({"bucket": bucket, "key": key, "body": json.loads(body)})

    request = {
        "run_id": "e2e-mixed-001",
        "dimensions": {
            "commission_rate": [0.1, 0.2],
            "num_drivers": [100, 200],
            "num_riders": [500, 700],
        },
        "shard_count": 4,
        "seed": 42,
        "failure_injection_shards": [2],
    }

    response = parent_handler({"body": json.dumps(request)}, None, lambda_client=client)
    if response["statusCode"] != 202:
        raise RuntimeError(f"Parent should accept request, got {response}")

    for event in client.events:
        try:
            child_handler(event, None, outcome_writer=outcome_writer)
        except Exception:
            pass

    observed = {record["body"]["shard_id"] for record in writes}
    expected = set(range(len(client.events)))
    statuses = [record["body"]["status"] for record in writes]

    print(f"Dispatched shards: {len(client.events)}")
    print(f"Outcome records: {len(writes)}")
    print(f"Observed shard IDs: {sorted(observed)}")
    print(f"Statuses: {statuses}")

    if observed != expected:
        raise RuntimeError(
            f"Incomplete shard coverage. expected={sorted(expected)} observed={sorted(observed)}"
        )
    if "failure" not in statuses or "success" not in statuses:
        raise RuntimeError("Expected mixed success/failure outcomes")

    print("Coverage verification passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
