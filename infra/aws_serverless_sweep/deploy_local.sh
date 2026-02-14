#!/usr/bin/env bash
set -euo pipefail

step() {
    echo ""
    echo "=== $1 ==="
}

fail() {
    echo "Error: $1" >&2
    exit 1
}

require_command() {
    local cmd="$1"
    local install_hint="$2"
    if ! command -v "${cmd}" >/dev/null 2>&1; then
        fail "required command '${cmd}' was not found. ${install_hint}"
    fi
}

preflight_aws_session() {
    if aws sts get-caller-identity >/dev/null 2>&1; then
        return
    fi

    local profile_hint=""
    if [[ -n "${AWS_PROFILE:-}" ]]; then
        profile_hint=" --profile ${AWS_PROFILE}"
    fi

    fail "no valid temporary AWS session detected. Run 'aws sso login${profile_hint}' and retry."
}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
TERRAFORM_DIR="${SCRIPT_DIR}/terraform"
DIST_DIR="${SCRIPT_DIR}/dist"

TARGET="${SWEEP_LAMBDA_TARGET:-x86_64-unknown-linux-gnu}"
PROFILE="${SWEEP_LAMBDA_PROFILE:-release}"
DOCKER_IMAGE="${SWEEP_DOCKER_IMAGE:-docker.io/library/rust:1-bullseye}"

DOCKER_MOUNT_ROOT="${REPO_ROOT}"
if [[ "${OSTYPE:-}" == msys* || "${OSTYPE:-}" == cygwin* ]]; then
    if command -v cygpath >/dev/null 2>&1; then
        DOCKER_MOUNT_ROOT="$(cygpath -m "${REPO_ROOT}")"
    fi
fi

step "Preflight checks"
require_command "docker" "Install Docker Desktop/Engine and ensure docker is on PATH."
require_command "aws" "Install AWS CLI v2 and ensure it is on PATH."
require_command "terraform" "Install Terraform CLI and ensure it is on PATH."
preflight_aws_session

step "Validate Docker Rust toolchain"
if ! MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL="*" docker run --rm --pull=always \
    "${DOCKER_IMAGE}" \
    sh -ec "export PATH=/usr/local/cargo/bin:\$PATH && cargo --version >/dev/null 2>&1 && rustup --version >/dev/null 2>&1"; then
    fail "docker image '${DOCKER_IMAGE}' does not provide cargo + rustup. Set SWEEP_DOCKER_IMAGE to an official Rust toolchain image (for example docker.io/library/rust:1-bullseye)."
fi

step "Build and package Rust Lambda artifacts (Docker)"
MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL="*" docker run --rm --pull=always \
    -v "${DOCKER_MOUNT_ROOT}:/workspace" \
    -w /workspace \
    "${DOCKER_IMAGE}" \
    sh -ec "export PATH=/usr/local/cargo/bin:\$PATH && rm -rf target/debug target/release target/${TARGET}/debug/build target/${TARGET}/release/build && apt-get update && apt-get install -y clang cmake make perl pkg-config && rustup target add ${TARGET} && CC=clang CXX=clang++ cargo run -p xtask -- serverless-package --target ${TARGET} --profile ${PROFILE}"

PARENT_ZIP="${DIST_DIR}/parent.zip"
CHILD_ZIP="${DIST_DIR}/child.zip"

if [[ ! -f "${PARENT_ZIP}" || ! -f "${CHILD_ZIP}" ]]; then
    echo "Expected packaged artifacts at:"
    echo "- ${PARENT_ZIP}"
    echo "- ${CHILD_ZIP}"
    exit 1
fi

step "Terraform init"
terraform -chdir="${TERRAFORM_DIR}" init

step "Terraform apply"
terraform -chdir="${TERRAFORM_DIR}" apply \
    -var "parent_lambda_zip=${PARENT_ZIP}" \
    -var "child_lambda_zip=${CHILD_ZIP}" \
    "$@"

echo ""
echo "Deployment flow complete."
