# Benchmarks iniciais da Nebula

Este conjunto mede o tempo de execução de programas compilados pelo backend nativo ELF x86-64, implementado em Rust. O backend atual não possui passes de otimização; os resultados são uma linha de base, não uma comparação de níveis de otimização.

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
- O backend ainda não aplica otimizações sobre a IR. Os níveis `-O0`, `-O1`, `-O2`, `-O3` e `-Os` serão introduzidos somente após a implementação dos recursos-base e do otimizador próprio.
- Este benchmark é uma linha de base inicial, não uma comparação científica nem uma prova de superioridade sobre C/C++.
