#!/usr/bin/env bash
# 检查版本号是否一致

VERSION_FILE=$(cat VERSION 2>/dev/null || echo "未找到VERSION文件")
CARGO=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
PACKAGE=$(grep '"version":' frontend/package.json | head -1 | sed 's/.*"version": "\(.*\)".*/\1/')
SERVER_ISS=$(grep '#define MyAppVersion' build_server_installer.iss | sed 's/#define MyAppVersion "\(.*\)"/\1/')
HOTFIX_ISS=$(grep '#define MyAppVersion' build_hot_update_installer.iss | sed 's/#define MyAppVersion "\(.*\)"/\1/')

echo "版本号检查:"
echo "  VERSION 文件:              $VERSION_FILE"
echo "  Cargo.toml:                $CARGO"
echo "  frontend/package.json:     $PACKAGE"
echo "  build_server_installer:    $SERVER_ISS"
echo "  build_hot_update_installer: $HOTFIX_ISS"

if [ "$VERSION_FILE" = "$CARGO" ] && [ "$CARGO" = "$PACKAGE" ] && [ "$PACKAGE" = "$SERVER_ISS" ] && [ "$SERVER_ISS" = "$HOTFIX_ISS" ]; then
    echo ""
    echo "✓ 版本号一致: $VERSION_FILE"
    exit 0
else
    echo ""
    echo "✗ 版本号不一致，请运行 ./scripts/set-version.sh 更新"
    exit 1
fi
