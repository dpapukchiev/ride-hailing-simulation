terraform {
  required_version = ">= 1.5.0"
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = ">= 5.0"
    }
  }
}

provider "aws" {
  region = var.aws_region
}

locals {
  parent_lambda_name = "${var.project_name}-parent"
  child_lambda_name  = "${var.project_name}-child"
}

resource "aws_s3_bucket" "results" {
  bucket = var.results_bucket_name
}

resource "aws_s3_bucket_versioning" "results" {
  bucket = aws_s3_bucket.results.id
  versioning_configuration {
    status = "Enabled"
  }
}

resource "aws_s3_bucket_server_side_encryption_configuration" "results" {
  bucket = aws_s3_bucket.results.id
  rule {
    apply_server_side_encryption_by_default {
      sse_algorithm = "AES256"
    }
  }
}

resource "aws_iam_role" "parent_lambda_role" {
  name = "${local.parent_lambda_name}-role"
  assume_role_policy = jsonencode({
    Version = "2012-10-17",
    Statement = [{
      Effect = "Allow",
      Principal = {
        Service = "lambda.amazonaws.com"
      },
      Action = "sts:AssumeRole"
    }]
  })
}

resource "aws_iam_role" "child_lambda_role" {
  name = "${local.child_lambda_name}-role"
  assume_role_policy = jsonencode({
    Version = "2012-10-17",
    Statement = [{
      Effect = "Allow",
      Principal = {
        Service = "lambda.amazonaws.com"
      },
      Action = "sts:AssumeRole"
    }]
  })
}

resource "aws_iam_role_policy_attachment" "parent_basic_logs" {
  role       = aws_iam_role.parent_lambda_role.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole"
}

resource "aws_iam_role_policy_attachment" "child_basic_logs" {
  role       = aws_iam_role.child_lambda_role.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole"
}

resource "aws_iam_role_policy" "parent_dispatch_policy" {
  name = "${local.parent_lambda_name}-dispatch"
  role = aws_iam_role.parent_lambda_role.id
  policy = jsonencode({
    Version = "2012-10-17",
    Statement = [
      {
        Effect   = "Allow",
        Action   = ["lambda:InvokeFunction"],
        Resource = aws_lambda_function.child_lambda.arn
      }
    ]
  })
}

resource "aws_iam_role_policy" "child_s3_policy" {
  name = "${local.child_lambda_name}-s3"
  role = aws_iam_role.child_lambda_role.id
  policy = jsonencode({
    Version = "2012-10-17",
    Statement = [
      {
        Effect = "Allow",
        Action = [
          "s3:PutObject"
        ],
        Resource = "${aws_s3_bucket.results.arn}/${trim(var.results_prefix, "/")}/*"
      },
      {
        Effect   = "Allow",
        Action   = ["s3:ListBucket"],
        Resource = aws_s3_bucket.results.arn,
        Condition = {
          StringLike = {
            "s3:prefix" = ["${trim(var.results_prefix, "/")}/*"]
          }
        }
      }
    ]
  })
}

resource "aws_lambda_function" "child_lambda" {
  function_name    = local.child_lambda_name
  role             = aws_iam_role.child_lambda_role.arn
  runtime          = "provided.al2023"
  handler          = "bootstrap"
  filename         = var.child_lambda_zip
  source_code_hash = filebase64sha256(var.child_lambda_zip)
  timeout          = 900
  memory_size      = 1024

  environment {
    variables = {
      SWEEP_RESULTS_BUCKET = aws_s3_bucket.results.bucket
      SWEEP_RESULTS_PREFIX = var.results_prefix
      MAX_SHARDS           = tostring(var.max_shards)
    }
  }
}

resource "aws_lambda_function" "parent_lambda" {
  function_name    = local.parent_lambda_name
  role             = aws_iam_role.parent_lambda_role.arn
  runtime          = "provided.al2023"
  handler          = "bootstrap"
  filename         = var.parent_lambda_zip
  source_code_hash = filebase64sha256(var.parent_lambda_zip)
  timeout          = 60
  memory_size      = 512

  environment {
    variables = {
      CHILD_LAMBDA_ARN = aws_lambda_function.child_lambda.arn
      MAX_SHARDS       = tostring(var.max_shards)
    }
  }
}

resource "aws_api_gateway_rest_api" "sweep_api" {
  name = "${var.project_name}-api"
}

resource "aws_api_gateway_resource" "sweep_run" {
  rest_api_id = aws_api_gateway_rest_api.sweep_api.id
  parent_id   = aws_api_gateway_rest_api.sweep_api.root_resource_id
  path_part   = "sweep-run"
}

resource "aws_api_gateway_model" "sweep_request" {
  rest_api_id  = aws_api_gateway_rest_api.sweep_api.id
  name         = "SweepRunRequest"
  content_type = "application/json"
  schema = jsonencode({
    type     = "object",
    required = ["run_id", "dimensions"],
    properties = {
      run_id = { type = "string", minLength = 1 },
      dimensions = {
        type                 = "object",
        minProperties        = 1,
        additionalProperties = { type = "array", minItems = 1 }
      },
      shard_count = { type = "integer", minimum = 1 },
      shard_size  = { type = "integer", minimum = 1 },
      seed        = { type = "integer" }
    },
    anyOf = [
      { required = ["shard_count"] },
      { required = ["shard_size"] }
    ]
  })
}

resource "aws_api_gateway_request_validator" "body_validator" {
  rest_api_id                 = aws_api_gateway_rest_api.sweep_api.id
  name                        = "validate-body"
  validate_request_body       = true
  validate_request_parameters = false
}

resource "aws_api_gateway_method" "sweep_post" {
  rest_api_id          = aws_api_gateway_rest_api.sweep_api.id
  resource_id          = aws_api_gateway_resource.sweep_run.id
  http_method          = "POST"
  authorization        = "NONE"
  request_validator_id = aws_api_gateway_request_validator.body_validator.id
  request_models = {
    "application/json" = aws_api_gateway_model.sweep_request.name
  }
}

resource "aws_api_gateway_integration" "parent_proxy" {
  rest_api_id             = aws_api_gateway_rest_api.sweep_api.id
  resource_id             = aws_api_gateway_resource.sweep_run.id
  http_method             = aws_api_gateway_method.sweep_post.http_method
  integration_http_method = "POST"
  type                    = "AWS_PROXY"
  uri                     = aws_lambda_function.parent_lambda.invoke_arn
}

resource "aws_lambda_permission" "allow_apigw" {
  statement_id  = "AllowExecutionFromApiGateway"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.parent_lambda.function_name
  principal     = "apigateway.amazonaws.com"
  source_arn    = "${aws_api_gateway_rest_api.sweep_api.execution_arn}/*/*"
}

resource "aws_api_gateway_deployment" "deployment" {
  rest_api_id = aws_api_gateway_rest_api.sweep_api.id

  triggers = {
    redeploy = sha1(jsonencode([
      aws_api_gateway_method.sweep_post.id,
      aws_api_gateway_integration.parent_proxy.id,
      aws_api_gateway_model.sweep_request.id,
    ]))
  }

  lifecycle {
    create_before_destroy = true
  }
}

resource "aws_api_gateway_stage" "sandbox" {
  deployment_id = aws_api_gateway_deployment.deployment.id
  rest_api_id   = aws_api_gateway_rest_api.sweep_api.id
  stage_name    = "sandbox"
}

resource "aws_iam_policy" "athena_query_access" {
  name = "${var.project_name}-athena-query-access"
  policy = jsonencode({
    Version = "2012-10-17",
    Statement = [
      {
        Effect = "Allow",
        Action = [
          "athena:StartQueryExecution",
          "athena:GetQueryExecution",
          "athena:GetQueryResults"
        ],
        Resource = "*"
      },
      {
        Effect = "Allow",
        Action = [
          "glue:GetDatabase",
          "glue:GetDatabases",
          "glue:GetTable",
          "glue:GetTables",
          "glue:GetPartitions"
        ],
        Resource = "*"
      },
      {
        Effect = "Allow",
        Action = ["s3:GetObject", "s3:ListBucket"],
        Resource = [
          aws_s3_bucket.results.arn,
          "${aws_s3_bucket.results.arn}/${trim(var.results_prefix, "/")}/*"
        ]
      }
    ]
  })
}

output "api_url" {
  value = "https://${aws_api_gateway_rest_api.sweep_api.id}.execute-api.${var.aws_region}.amazonaws.com/${aws_api_gateway_stage.sandbox.stage_name}/sweep-run"
}

output "results_bucket" {
  value = aws_s3_bucket.results.bucket
}

output "athena_query_policy_arn" {
  value = aws_iam_policy.athena_query_access.arn
}
