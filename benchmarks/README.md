# Benchmarks iniciais da Nebula

Este conjunto mede, de forma reproduzível, o tempo de execução de dois programas Nebula compilados com cada nível aceito pelo backend nativo C++17: `-O0`, `-O1`, `-O2`, `-O3` e `-Os`.

## Requisitos

- Linux x86-64.
- Rust 1.85.0 e Cargo.
- Um compilador C++17 acessível como `c++` (G++ ou Clang++).
- GNU `time` em `/usr/bin/time`.

## Cargas

- `loop.nbx`: soma de 50 milhões de inteiros em um laço.
- `fibonacci.nbx`: cálculo recursivo de Fibonacci(37), exercitando chamadas e retornos.

## Execução

Na raiz do repositório:

```bash
bash benchmarks/executar.sh
```

O script compila a CLI em modo release, gera um binário por carga e nível de otimização e executa cada combinação cinco vezes. A saída dos programas é descartada durante a cronometragem.

## Interpretação

- Compare somente resultados obtidos na mesma máquina, com o mesmo sistema, compilador C++ e condições de carga.
- Registre CPU, sistema operacional, versão de `c++`, versão da Nebula e os tempos brutos.
- Repita medições e use mediana; não compare uma única execução.
- O tempo inclui somente a execução do programa gerado, não a compilação da Nebula ou do C++.
- Os níveis são encaminhados ao compilador C++ instalado. Resultados variam entre G++, Clang++ e suas versões.
- Medições abaixo da resolução de `/usr/bin/time` aparecem como `0.00 s`; aumente a carga ou use um medidor de maior resolução antes de tirar conclusões.
- Este benchmark é uma linha de base inicial, não uma comparação científica nem uma prova de superioridade sobre C/C++. Ele não valida a meta experimental de desempenho de até 3× C++.
