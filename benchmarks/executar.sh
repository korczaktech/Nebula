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
    printf '\n%s — laço\n' "$nivel"
    executavel="$temporario/nebula-laco-${nivel#-}"
    "$nebula" compilar benchmarks/loop.nbx -o "$executavel" "$nivel"
    for repeticao in 1 2 3 4 5; do
        /usr/bin/time -f "  execução $repeticao: %e s" "$executavel" >/dev/null
    done

    printf '\n%s — recursão\n' "$nivel"
    executavel="$temporario/nebula-recursao-${nivel#-}"
    "$nebula" compilar benchmarks/fibonacci.nbx -o "$executavel" "$nivel"
    for repeticao in 1 2 3 4 5; do
        /usr/bin/time -f "  execução $repeticao: %e s" "$executavel" >/dev/null
    done
done
