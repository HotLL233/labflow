#!/usr/bin/env bash
# 版本号统一更新脚本
# 用法: ./scripts/set-version.sh <version>
# 示例: ./scripts/set-version.sh 2.2.7

set -e

if [ $# -ne 1 ]; then
    echo "用法: $0 <version>"
    echo "示例: $0 2.2.7"
    exit 1
fi

VERSION=$1
VERSION_NUM="${VERSION}.0"

echo "更新版本号到: $VERSION"

# 1. 更新 VERSION 文件
echo "$VERSION" > VERSION

# 2. 更新 Cargo.toml
if [ -f "Cargo.toml" ]; then
    sed -i.bak "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml
    rm -f Cargo.toml.bak
    echo "✓ Cargo.toml"
fi

# 3. 更新 frontend/package.json
if [ -f "frontend/package.json" ]; then
    sed -i.bak "s/\"version\": \".*\"/\"version\": \"$VERSION\"/" frontend/package.json
    rm -f frontend/package.json.bak
    echo "✓ frontend/package.json"
fi

# 4. 更新服务器安装包脚本
if [ -f "build_server_installer.iss" ]; then
    sed -i.bak "s/#define MyAppVersion \".*\"/#define MyAppVersion \"$VERSION\"/" build_server_installer.iss
    sed -i.bak "s/#define MyAppNumericVersion \".*\"/#define MyAppNumericVersion \"$VERSION_NUM\"/" build_server_installer.iss
    rm -f build_server_installer.iss.bak
    echo "✓ build_server_installer.iss"
fi

# 5. 更新热更新安装包脚本
if [ -f "build_hot_update_installer.iss" ]; then
    sed -i.bak "s/#define MyAppVersion \".*\"/#define MyAppVersion \"$VERSION\"/" build_hot_update_installer.iss
    sed -i.bak "s/#define MyAppNumericVersion \".*\"/#define MyAppNumericVersion \"$VERSION_NUM\"/" build_hot_update_installer.iss
    rm -f build_hot_update_installer.iss.bak
    echo "✓ build_hot_update_installer.iss"
fi

echo ""
echo "版本号已更新为: $VERSION"
echo "请检查 git diff 确认修改正确，然后提交。"
