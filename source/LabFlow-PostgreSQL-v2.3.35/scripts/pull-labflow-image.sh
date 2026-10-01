#!/usr/bin/env bash
set -euo pipefail
compose_file="${COMPOSE_FILE:-docker-compose-ghcr.yml}"
env_file="${ENV_FILE:-.env}"
[[ -f "$compose_file" ]] || { echo "Compose file not found: $compose_file" >&2; exit 1; }
if [[ -f "$env_file" ]]; then
  set -a
  # shellcheck disable=SC1090
  source "$env_file"
  set +a
fi
version="${LABFLOW_VERSION:-2.3.11}"
ghcr_image="${LABFLOW_GHCR_IMAGE:-ghcr.io/hotll233/labflow}"
gitee_image="${LABFLOW_GITEE_IMAGE:-}"
priority="${LABFLOW_IMAGE_PRIORITY:-gitee,ghcr}"
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
    docker compose --env-file "$env_file" -f "$compose_file" up -d
    echo "Using $image"
    exit 0
  fi
done
echo "No configured LabFlow image could be pulled. Set LABFLOW_GITEE_IMAGE and/or LABFLOW_GHCR_IMAGE." >&2
exit 1
