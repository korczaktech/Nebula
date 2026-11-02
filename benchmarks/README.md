# Benchmarks iniciais da Nebula

Este conjunto mede, de forma reproduzível, o tempo de execução de um mesmo programa Nebula compilado com cada nível aceito pelo backend C++17: `-O0`, `-O1`, `-O2`, `-O3` e `-Os`.

## Requisitos

- Linux x86-64.
- Rust 1.85.0 e Cargo.
- Um compilador C11 acessível como `cc`.
- GNU `time` em `/usr/bin/time`.

## Execução

Na raiz do repositório:

```bash
bash benchmarks/executar.sh
```

O script compila a CLI em modo release, gera cinco binários com os níveis de otimização e executa cada binário cinco vezes. O programa imprime o mesmo resultado em todas as execuções; a saída é descartada durante a cronometragem.

## Interpretação

- Compare somente resultados obtidos na mesma máquina, com o mesmo sistema, compilador C e condições de carga.
- Registre CPU, sistema operacional, versão de `c++`, versão da Nebula e os tempos brutos.
- Repita medições e use mediana; não compare uma única execução.
- O tempo inclui somente a execução do programa gerado, não a compilação C.
- Os níveis são encaminhados ao compilador C instalado. Resultados variam entre G++, Clang++ e suas versões.
- Este benchmark não demonstra superioridade sobre C/C++ e não valida a meta experimental de desempenho de até 3× C++.
