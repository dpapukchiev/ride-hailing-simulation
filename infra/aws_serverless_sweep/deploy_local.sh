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

timestamp() {
    date +%s
}

duration_since() {
    local start="$1"
    echo "$(( $(timestamp) - start ))s"
}

require_command() {
    local cmd="$1"
    local install_hint="$2"
    if ! command -v "${cmd}" >/dev/null 2>&1; then
        fail "required command '${cmd}' was not found. ${install_hint}"
    fi
}

pull_image_by_policy() {
    local image="$1"
    local policy="$2"

    case "${policy}" in
        always)
            docker pull "${image}" >/dev/null
            ;;
        if-missing)
            if ! docker image inspect "${image}" >/dev/null 2>&1; then
                docker pull "${image}" >/dev/null
            fi
            ;;
        never)
            if ! docker image inspect "${image}" >/dev/null 2>&1; then
                fail "docker image '${image}' is missing and SWEEP_DOCKER_PULL_POLICY=never prevents pulling."
            fi
            ;;
        *)
            fail "invalid SWEEP_DOCKER_PULL_POLICY='${policy}'. expected one of: always, if-missing, never."
            ;;
    esac
}

compute_source_hash() {
    local file
    local digest_input=""
    local hash_files

    hash_files="$(
        git -C "${REPO_ROOT}" ls-files | rg '^(Cargo.lock|Cargo.toml|xtask/|crates/sim_serverless_sweep_core/|crates/sim_serverless_sweep_lambda/)'
    )"

    while IFS= read -r file; do
        [[ -z "${file}" ]] && continue
        digest_input+="${file}:$(sha256sum "${REPO_ROOT}/${file}" | awk '{print $1}')"$'\n'
    done <<< "${hash_files}"

    printf '%s' "${digest_input}" | sha256sum | awk '{print $1}'
}

write_build_metadata() {
    local metadata_path="$1"
    local source_hash="$2"

    cat > "${metadata_path}" <<EOF
target=${TARGET}
profile=${PROFILE}
docker_image=${DOCKER_IMAGE}
builder_image=${BUILDER_IMAGE}
source_hash=${source_hash}
EOF
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
DOCKER_PULL_POLICY="${SWEEP_DOCKER_PULL_POLICY:-if-missing}"
FORCE_REBUILD="${SWEEP_FORCE_REBUILD:-0}"

TERRAFORM_ARGS=()
for arg in "$@"; do
    case "${arg}" in
        --force-rebuild)
            FORCE_REBUILD=1
            ;;
        *)
            TERRAFORM_ARGS+=("${arg}")
            ;;
    esac
done

IMAGE_SLUG="$(printf '%s' "${DOCKER_IMAGE}" | tr '/:@' '---')"
BUILDER_IMAGE="${SWEEP_BUILDER_IMAGE:-ride-sweep-builder:${IMAGE_SLUG}}"

RUNTIME_ZIP="${DIST_DIR}/runtime.zip"
BUILD_METADATA="${DIST_DIR}/runtime-build.metadata"

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
require_command "git" "Install Git and ensure it is on PATH."
require_command "sha256sum" "Install coreutils (sha256sum) and ensure it is on PATH."
require_command "rg" "Install ripgrep and ensure it is on PATH."
preflight_aws_session

mkdir -p "${DIST_DIR}"

SOURCE_HASH="$(compute_source_hash)"

CACHE_HIT=0
CACHE_REASON=""
if [[ "${FORCE_REBUILD}" == "1" ]]; then
    CACHE_REASON="forced rebuild"
elif [[ ! -f "${RUNTIME_ZIP}" ]]; then
    CACHE_REASON="runtime artifact missing"
elif [[ ! -f "${BUILD_METADATA}" ]]; then
    CACHE_REASON="build metadata missing"
elif ! grep -Fxq "target=${TARGET}" "${BUILD_METADATA}"; then
    CACHE_REASON="target changed"
elif ! grep -Fxq "profile=${PROFILE}" "${BUILD_METADATA}"; then
    CACHE_REASON="profile changed"
elif ! grep -Fxq "docker_image=${DOCKER_IMAGE}" "${BUILD_METADATA}"; then
    CACHE_REASON="docker image changed"
elif ! grep -Fxq "builder_image=${BUILDER_IMAGE}" "${BUILD_METADATA}"; then
    CACHE_REASON="builder image changed"
elif ! grep -Fxq "source_hash=${SOURCE_HASH}" "${BUILD_METADATA}"; then
    CACHE_REASON="source hash changed"
else
    CACHE_HIT=1
fi

step "Prepare Docker builder image"
PREPARE_START="$(timestamp)"
pull_image_by_policy "${DOCKER_IMAGE}" "${DOCKER_PULL_POLICY}"

if ! docker image inspect "${BUILDER_IMAGE}" >/dev/null 2>&1; then
    docker build \
        --build-arg "BASE_IMAGE=${DOCKER_IMAGE}" \
        -t "${BUILDER_IMAGE}" \
        - <<'EOF'
ARG BASE_IMAGE
FROM ${BASE_IMAGE}
RUN apt-get update \
    && apt-get install -y --no-install-recommends clang cmake make perl pkg-config \
    && rm -rf /var/lib/apt/lists/*
EOF
fi

if ! MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL="*" docker run --rm \
    "${BUILDER_IMAGE}" \
    sh -ec "export PATH=/usr/local/cargo/bin:\$PATH && cargo --version >/dev/null 2>&1 && rustup --version >/dev/null 2>&1"; then
    fail "docker builder image '${BUILDER_IMAGE}' does not provide cargo + rustup. Set SWEEP_DOCKER_IMAGE/SWEEP_BUILDER_IMAGE to valid Rust toolchain images."
fi
echo "Builder image ready in $(duration_since "${PREPARE_START}") (pull-policy=${DOCKER_PULL_POLICY})."

if [[ "${CACHE_HIT}" == "1" ]]; then
    step "Build and package Rust Lambda artifacts (Docker)"
    echo "cache-hit: reusing ${RUNTIME_ZIP}."
else
    step "Build and package Rust Lambda artifacts (Docker)"
    BUILD_START="$(timestamp)"
    echo "cache-miss: ${CACHE_REASON}. rebuilding runtime artifacts..."
    MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL="*" docker run --rm \
        -v "${DOCKER_MOUNT_ROOT}:/workspace" \
        -w /workspace \
        "${BUILDER_IMAGE}" \
        sh -ec "export PATH=/usr/local/cargo/bin:\$PATH && rustup target add ${TARGET} && CC=clang CXX=clang++ cargo run -p xtask -- serverless-package --target ${TARGET} --profile ${PROFILE}"
    write_build_metadata "${BUILD_METADATA}" "${SOURCE_HASH}"
    echo "Build completed in $(duration_since "${BUILD_START}")."
fi

if [[ ! -f "${RUNTIME_ZIP}" ]]; then
    echo "Expected packaged artifact at:"
    echo "- ${RUNTIME_ZIP}"
    exit 1
fi

step "Terraform init"
terraform -chdir="${TERRAFORM_DIR}" init

step "Terraform apply"
terraform -chdir="${TERRAFORM_DIR}" apply \
    -var "runtime_lambda_zip=${RUNTIME_ZIP}" \
    "${TERRAFORM_ARGS[@]}"

echo ""
echo "Deployment flow complete."
