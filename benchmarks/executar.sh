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

for carga in loop fibonacci; do
    printf '\n%s — compilação nativa sem passes de otimização\n' "$carga"
    executavel="$temporario/nebula-$carga"
    "$nebula" compilar "benchmarks/$carga.nbx" -o "$executavel"
    for repeticao in 1 2 3 4 5; do
        /usr/bin/time -f "  execução $repeticao: %e s" "$executavel" >/dev/null || {
            codigo=$?
            if [[ "$carga" == "fibonacci" && "$codigo" -eq 121 ]]; then
                continue
            fi
            echo "Programa terminou com código inesperado: $codigo" >&2
            exit 1
        }
    done
done
