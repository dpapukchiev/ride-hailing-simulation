import json
import os
import sys
import unittest
from pathlib import Path

MODULE_DIR = Path(__file__).resolve().parents[1] / "lambda"
sys.path.insert(0, str(MODULE_DIR))

from serverless_sweep import (
    compute_shard_bounds,
    parent_handler,
    validate_sweep_request,
)


class FakeLambdaClient:
    def __init__(self):
        self.invocations = []

    def invoke(self, **kwargs):
        self.invocations.append(kwargs)
        return {"StatusCode": 202}


class SweepHandlerTests(unittest.TestCase):
    def setUp(self):
        os.environ["CHILD_LAMBDA_ARN"] = "arn:aws:lambda:region:acct:function:child"

    def test_partitioning_covers_space_exactly_once(self):
        req = validate_sweep_request(
            {
                "run_id": "unit-run",
                "dimensions": {
                    "commission_rate": [0.1, 0.2],
                    "num_drivers": [100, 200, 300],
                },
                "shard_count": 4,
            }
        )
        bounds = compute_shard_bounds(req)

        self.assertEqual(bounds[0].start_index, 0)
        self.assertEqual(bounds[-1].end_index, req["total_points"])

        seen = set()
        for shard in bounds:
            for idx in range(shard.start_index, shard.end_index):
                self.assertNotIn(idx, seen)
                seen.add(idx)
        self.assertEqual(len(seen), req["total_points"])

    def test_invalid_request_rejected_without_dispatch(self):
        client = FakeLambdaClient()
        event = {"body": json.dumps({"run_id": "missing-dimensions"})}
        response = parent_handler(event, None, lambda_client=client)

        self.assertEqual(response["statusCode"], 400)
        self.assertEqual(client.invocations, [])

    def test_parent_dispatches_async_invocations(self):
        client = FakeLambdaClient()
        event = {
            "body": json.dumps(
                {
                    "run_id": "dispatch-run",
                    "dimensions": {
                        "commission_rate": [0.1, 0.2],
                        "num_drivers": [100, 200],
                    },
                    "shard_count": 2,
                    "seed": 7,
                    "failure_injection_shards": [1],
                }
            )
        }
        response = parent_handler(event, None, lambda_client=client)

        self.assertEqual(response["statusCode"], 202)
        self.assertEqual(len(client.invocations), 2)
        self.assertTrue(
            all(
                invocation["InvocationType"] == "Event"
                for invocation in client.invocations
            )
        )


if __name__ == "__main__":
    unittest.main()
