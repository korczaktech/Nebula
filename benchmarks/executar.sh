#!/usr/bin/env bash
set -euo pipefail

raiz="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$raiz"

command -v /usr/bin/time >/dev/null 2>&1 || {
    echo "Erro: este benchmark requer GNU time em /usr/bin/time." >&2
    exit 1
}

cargo build --release --locked -p nebula-cli
nebula="$raiz/target/release/nebula"
temporario="$(mktemp -d)"
trap 'rm -rf "$temporario"' EXIT

for nivel in -O0 -O1 -O2 -O3 -Os; do
    executavel="$temporario/nebula-${nivel#-}"
    "$nebula" compilar benchmarks/loop.nbx -o "$executavel" "$nivel"
    printf '\n%s\n' "$nivel"
    for repeticao in 1 2 3 4 5; do
        printf '  execução %s: ' "$repeticao"
        /usr/bin/time -f '%e s' "$executavel" >/dev/null
    done
done
