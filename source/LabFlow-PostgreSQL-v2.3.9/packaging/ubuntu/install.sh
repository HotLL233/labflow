#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
INSTALL_DIR="${LABFLOW_INSTALL_DIR:-/opt/labflow-v2.3.9}"
COMPOSE_PROJECT_NAME="${COMPOSE_PROJECT_NAME:-labflow}"
COMPOSE_FILE="${LABFLOW_COMPOSE_FILE:-docker-compose-ghcr.yml}"

if [[ "$(id -u)" -ne 0 ]]; then
  echo "Run as root: sudo $0" >&2
  exit 1
fi
if ! command -v docker >/dev/null 2>&1; then
  echo "Docker is required. Install Docker Engine and the Compose plugin first." >&2
  exit 1
fi
if ! docker compose version >/dev/null 2>&1; then
  echo "Docker Compose v2 is required." >&2
  exit 1
fi

install -d -m 0755 "$INSTALL_DIR"
cp "$ROOT_DIR/Dockerfile" "$ROOT_DIR/$COMPOSE_FILE" "$ROOT_DIR/.env.example" "$ROOT_DIR/.env.ghcr.example" "$INSTALL_DIR/"
if [[ ! -f "$INSTALL_DIR/.env" ]]; then
  cp "$ROOT_DIR/.env.example" "$INSTALL_DIR/.env"
  chmod 0600 "$INSTALL_DIR/.env"
  echo "Created $INSTALL_DIR/.env; edit PostgreSQL and admin passwords before production use."
fi

cat >/etc/systemd/system/labflow-docker.service <<EOF
[Unit]
Description=LabFlow v2.3.9 Docker deployment
Requires=docker.service
After=docker.service network-online.target

[Service]
Type=oneshot
RemainAfterExit=yes
WorkingDirectory=$INSTALL_DIR
Environment=COMPOSE_PROJECT_NAME=$COMPOSE_PROJECT_NAME
ExecStart=/usr/bin/docker compose -f "$INSTALL_DIR/$COMPOSE_FILE" --env-file "$INSTALL_DIR/.env" up -d
ExecStop=/usr/bin/docker compose -f "$INSTALL_DIR/$COMPOSE_FILE" --env-file "$INSTALL_DIR/.env" down

[Install]
WantedBy=multi-user.target
EOF

docker compose -f "$INSTALL_DIR/$COMPOSE_FILE" --env-file "$INSTALL_DIR/.env" -p "$COMPOSE_PROJECT_NAME" pull
docker compose -f "$INSTALL_DIR/$COMPOSE_FILE" --env-file "$INSTALL_DIR/.env" -p "$COMPOSE_PROJECT_NAME" up -d

if command -v systemctl >/dev/null 2>&1; then
  systemctl daemon-reload
  systemctl enable --now labflow-docker.service
fi

echo "LabFlow Docker deployment installed at $INSTALL_DIR"
echo "If image is private, run: docker login ghcr.io"
echo "Then restart service: systemctl restart labflow-docker.service"
