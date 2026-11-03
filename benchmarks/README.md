# Benchmarks iniciais da Nebula

Este conjunto mede o tempo de execução de programas compilados pelo backend nativo ELF x86-64, implementado em Rust. O backend executa a IR otimizada pelo passe O1 padrão da CLI; os resultados são uma linha de base de execução, não uma comparação entre níveis.

## Requisitos

- Linux x86-64.
- Rust 1.85.0 e Cargo.
- GNU `time` em `/usr/bin/time`.

## Cargas

- `loop.nbx`: laço de 50 milhões de iterações.
- `fibonacci.nbx`: Fibonacci(37), exercitando chamadas e recursão.

## Execução

Na raiz do repositório:

```bash
bash benchmarks/executar.sh
```

O script compila a CLI em modo release, gera um executável para cada carga e executa cada programa cinco vezes. A saída é descartada durante a cronometragem.

## Interpretação

- Compare somente resultados obtidos na mesma máquina e sob condições equivalentes.
- Registre CPU, sistema operacional, versão da Nebula e tempos brutos.
- Repita medições e use mediana; não compare uma única execução.
- O tempo medido inclui somente a execução do programa gerado.
- O compilador possui níveis `-O0`, `-O1`, `-O2`, `-O3` e `-Os`; este script usa o nível padrão `-O1`. Compare níveis diferentes somente com o mesmo ambiente.
- Este benchmark é uma linha de base inicial, não uma comparação científica nem uma prova de superioridade sobre C/C++.

## Baseline versionada

Primeira medição de referência registrada pela CI em `ubuntu-latest`, Rust 1.85.0, build release, nível padrão O1, cinco execuções por carga. Commit medido: `a6786adda1311e0bc56884534e3713c595031a6b`.

| Carga | Tempos brutos (s) | Mediana |
|---|---|---:|
| loop | 0.11, 0.11, 0.11, 0.11, 0.11 | 0.11 s |
| fibonacci | 0.23, 0.23, 0.22, 0.25, 0.25 | 0.23 s |

O runner hospedado é variável e a CPU exata não foi registrada nessa execução; portanto, estes valores são apenas referência histórica. Uma comparação de desempenho válida exige repetir a coleta na mesma máquina, registrar CPU/OS/toolchain e comparar medianas.
### Medição posterior

Coleta na CI em `ubuntu-latest`, Rust 1.85.0, build release, O1, cinco execuções por carga. Commit medido: `2548183c6729e25e4a193d566b47d5a71830d98a`.

| Carga | Tempos brutos (s) | Mediana |
|---|---|---:|
| loop | 0.12, 0.12, 0.12, 0.12, 0.13 | 0.12 s |
| fibonacci | 0.26, 0.25, 0.25, 0.25, 0.25 | 0.25 s |

A diferença entre medições não deve ser interpretada como regressão sem controle do hardware do runner; o runner hospedado não garante a mesma CPU em execuções distintas.
