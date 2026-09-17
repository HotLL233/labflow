#!/usr/bin/env bash
set -euo pipefail

compose_file="${COMPOSE_FILE:-docker-compose.deploy.yml}"
env_file="${ENV_FILE:-.env}"
version="${LABFLOW_VERSION:-latest}"
ghcr_image="${LABFLOW_GHCR_IMAGE:-ghcr.io/hotll233/labflow}"
gitee_image="${LABFLOW_GITEE_IMAGE:-}"
priority="${LABFLOW_IMAGE_PRIORITY:-gitee,ghcr}"

[[ -f "$compose_file" ]] || { echo "Compose file not found: $compose_file" >&2; exit 1; }

if [[ -f "$env_file" ]]; then
  set -a
  # The deployment .env file is operator-controlled and contains simple KEY=VALUE entries.
  # shellcheck disable=SC1090
  source "$env_file"
  set +a
  version="${LABFLOW_VERSION:-latest}"
  ghcr_image="${LABFLOW_GHCR_IMAGE:-ghcr.io/hotll233/labflow}"
  gitee_image="${LABFLOW_GITEE_IMAGE:-}"
  priority="${LABFLOW_IMAGE_PRIORITY:-gitee,ghcr}"
fi

IFS=',' read -ra names <<< "$priority"
for raw_name in "${names[@]}"; do
  name="${raw_name//[[:space:]]/}"
  case "$name" in
    gitee) base_image="$gitee_image" ;;
    ghcr) base_image="$ghcr_image" ;;
    *) continue ;;
  esac
  [[ -n "$base_image" ]] || continue
  image="${base_image}:${version}"
  echo "Trying $name image: $image"
  if docker pull "$image"; then
    export LABFLOW_IMAGE="$base_image"
    export LABFLOW_VERSION="$version"
    compose_args=(compose)
    [[ -f "$env_file" ]] && compose_args+=(--env-file "$env_file")
    compose_args+=(-f "$compose_file" up -d)
    docker "${compose_args[@]}"
    echo "Using $image"
    exit 0
  fi
done

echo "No configured LabFlow image could be pulled. Set LABFLOW_GITEE_IMAGE and/or LABFLOW_GHCR_IMAGE." >&2
exit 1
