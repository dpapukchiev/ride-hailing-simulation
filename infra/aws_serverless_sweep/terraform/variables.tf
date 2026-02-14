variable "aws_region" {
  description = "AWS region for serverless sweep deployment"
  type        = string
  default     = "eu-central-1"
}

variable "project_name" {
  description = "Resource naming prefix"
  type        = string
  default     = "ride-sim-sweep"
}

variable "results_bucket_name" {
  description = "S3 bucket for partitioned sweep outcomes"
  type        = string
  default     = "ride-hailing-simulation-dpapukchiev"
}

variable "results_prefix" {
  description = "S3 prefix where worker outcomes are stored"
  type        = string
  default     = "serverless-sweeps/outcomes"
}

variable "parent_lambda_zip" {
  description = "Path to parent Lambda zip artifact"
  type        = string
  default     = "../dist/parent.zip"
}

variable "child_lambda_zip" {
  description = "Path to child Lambda zip artifact"
  type        = string
  default     = "../dist/child.zip"
}

variable "max_shards" {
  description = "Upper bound on shards per run"
  type        = number
  default     = 1000
}

variable "athena_database" {
  description = "Athena database for sweep analytics"
  type        = string
  default     = "ride_sim_analytics"
}

variable "athena_table" {
  description = "Athena table name for shard outcomes"
  type        = string
  default     = "sweep_shard_outcomes"
}
