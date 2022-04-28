#!/usr/bin/sh

version="$1"
if [ -z "$version" ]; then
    echo "Usage: $0 <new version>"
    exit 1
fi

sed -i "/^version = .*/c\version = \"$version\"" Cargo.toml
sed -i "/^Version: .*/c\Version: $version" contrib/rust-cipo.spec
sed -i "/\.version.*/c\.version\(\"$version\"\)" src/args.rs
rustfmt src/args.rs
cargo generate-lockfile
