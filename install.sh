#!/bin/sh
set -eu

REPO="redac1ed/sonion"
VERSION="${SONION_VERSION:-latest}"

case "$(uname -s)" in
    Linux) os=unknown-linux-gnu ;;
    Darwin) os=apple-darwin ;;
    *) echo "unsupported os: $(uname -s), grab a build from https://github.com/$REPO/releases" >&2; exit 1 ;;
esac

case "$(uname -m)" in 
    x86_64 | amd64) arch=x86_64 ;;
    aarch64 | arm64) arch=aarch64 ;;
    *) echo "unsupported cpu: $(uname -m)" >&2; exit 1 ;;
esac

file="sonion-$arch-$os.tar.gz"
if [ "$VERSION" = latest ]; then
    url="https://github.com/$REPO/releases/latest/download/$file"
else
    url="https://github.com/$REPO/releases/download/$VERSION/$file"
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

echo "downloading $url"
curl -fsSL "$url" -o "$tmp/$file"
curl -fsSL "$url.sha256" -o "$tmp/$file.sha256"

cd "$tmp"
if command -v sha256sum >/dev/null 2>&1; then
    sha256sum -c "$file.sha256" >/dev/null
elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 -c "$file.sha256" >/dev/null
else
    echo "warning: no sha256 tool found, skipping checksum" >&2
fi
tar -xzf "$file"

dir="${SONION_INSTALL_DIR:-/usr/local/bin}"
if [ -w "$dir" ]; then
    install -m 755 sonion "$dir/sonion"
elif command -v sudo >/dev/null 2>&1; then
    sudo install -m 755 sonion "$dir/sonion"
else
    dir="$HOME/.local/bin"
    mkdir -p "$dir"
    install -m 755 sonion "$dir/sonion"
fi

echo "installed sonion to $dir/sonion"
case ":$PATH:" in
    *":$dir:"*) ;;
    *) echo "add $dir to your PATH" ;;
esac