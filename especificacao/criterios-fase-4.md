# Critérios de aceite — Fase 4

**Estado: CONCLUÍDA E VALIDADA.** O backend gera ELF64 diretamente em Rust. Todos os critérios de aceite desta implementação inicial foram verificados pela CI no commit `e3a2ad338f96b832bd652876e220114f8bd53858` (Linux x86-64, Rust 1.85.0).

## Objetivo

Entregar um compilador que transforme a IR em executáveis nativos sem gerar C, C++, assembly textual ou outra linguagem intermediária e sem invocar compiladores, assemblers ou linkers externos.

## Decisões obrigatórias

- Implementação do compilador e do backend em Rust.
- Geração direta de instruções de máquina e do formato executável pelo próprio backend.
- Nenhuma dependência obrigatória de C, C++, LLVM, Cranelift, assembler ou linker externo.
- Primeiro alvo implementado e validado na CI: Linux x86-64, ELF64.
- O runtime Nebula e a biblioteca padrão não serão implementados antecipadamente. Só entram na etapa prevista no plano.
- O otimizador próprio só será desenvolvido depois que os recursos-base estiverem implementados e validados.
- Não oferecer flags de otimização que apenas encaminhem opções a ferramentas externas.

## Estado da implementação

| Critério | Estado atual |
|---|---|
| Pipeline Lexer/Parser → semântica → IR → backend | Implementado e validado pela CI |
| Escrita de cabeçalho ELF64 em Rust | Implementada para ELF64 estático Linux x86-64 |
| Emissão de instruções x86-64 em Rust | Subconjunto implementado e validado por testes unitários de codificação e integração nativa |
| Entrada ELF e encerramento do processo | Implementados para o alvo Linux x86-64, com syscall `exit` |
| Variáveis locais, globais, sombreamento e parâmetros (até seis registradores do ABI) | Implementados com segmento ELF RW separado; testes de integração adicionados |
| Expressões inteiras e lógicas, incluindo potência inteira por quadratura | Implementadas para o subconjunto de 64 bits; overflow checked e expoentes no intervalo u32 |
| Funções, chamadas aninhadas, seis parâmetros e recursão | Implementados em Rust; testes de integração de ABI adicionados |
| Condicionais e laços | Implementados; testes de integração para `quando`, `enquanto`, `para`, `parar` e `seguir` |
| CLI sem invocar compilador C/C++ | Implementada; bloqueia sobrescrita do fonte e publica a saída atomicamente |
| Testes Rust do workspace | 169 testes aprovados na CI: 20 backend, 5 CLI unitários, 13 CLI integração, 37 nativos, 1 diagnósticos, 4 fonte, 14 IR, 30 semântica e 45 sintaxe |
| Runtime e biblioteca padrão | Adiados; não implementados |
| Otimizador e níveis de otimização | Adiados; não implementados |
| Benchmark sem otimizações | Script validado com `bash -n` e as duas cargas executadas com sucesso na CI |

## Critérios de conclusão

- O backend gera executáveis nativos ELF64 diretamente em Rust.
- A CLI não invoca compilador, assembler ou linker externo.
- O executável gerado roda no alvo Linux x86-64 declarado.
- As instruções implementadas têm testes unitários de codificação de bytes x86-64 e testes de integração que executam os binários, incluindo limites signed, saltos, relocação, chamadas e controle de fluxo.
- Chamadas, parâmetros (até seis), chamadas aninhadas, recursão, escopos, retorno, saltos e alinhamento de pilha seguem o ABI x86-64 documentado.
- A aritmética inteira signed de 64 bits tem semântica checked: overflow em soma, subtração, multiplicação, negação, divisão, resto e potência termina com código de saída 2; divisão por zero e expoente negativo também terminam com código 2. Potência usa exponenciação por quadratura e aceita expoentes entre 0 e `u32::MAX`.
- O literal mínimo `-9223372036854775808` é aceito sem conversão intermediária inválida.
- Variáveis globais usam segmento ELF gravável separado; o segmento de código permanece somente leitura e execução, e a pilha é marcada como não executável (W^X/NX).
- A gravação do executável é atômica: falhas de compilação não publicam arquivos parciais nem destroem uma saída anterior.
- Entradas inválidas ou recursos ainda não implementados geram diagnósticos claros e não deixam executáveis inválidos.
- A CI aprovou formatação, Clippy com `-D warnings`, todos os 169 testes e o benchmark inicial.
- A documentação não anuncia como prontos recursos que dependem do runtime ou do otimizador.

## Recursos explicitamente fora desta implementação inicial

- Tipos inteiros de 8, 16, 32 e 128 bits, byte e caractere como variável (o alvo inicial só aceita inteiro de 64 bits e lógico).
- Strings e concatenação de texto.
- Entrada e saída padrão da linguagem (`exibir`, `ler_texto`).
- Listas, indexação e alocação dinâmica.
- Tipos decimais e operações de ponto flutuante.
- Caracteres Unicode.
- Runtime Nebula e biblioteca padrão.
- Otimizador, passes de IR e níveis `-O0`, `-O1`, `-O2`, `-O3` e `-Os`.
- Outros formatos executáveis, arquiteturas e sistemas operacionais.
- WebAssembly, carregamento de módulos do disco e resolvedor de pacotes.

Esses itens não podem ser simulados nem implementados parcialmente como se estivessem prontos. Sua implementação depende da etapa apropriada do plano e de critérios próprios.

## Comandos atuais

```text
nebula verificar programa.nbx
nebula analisar programa.nbx
nebula ir programa.nbx
nebula compilar programa.nbx [-o executavel]
nebula executar programa.nbx
nebula testar programa.nbx
```

O comando `compilar` produz executável ELF Linux x86-64. `executar` e `testar` compilam e iniciam o executável gerado. O código de saída do programa é preservado.

A Fase 4 está concluída dentro do escopo inicial documentado. A validação foi executada no GitHub Actions em Linux x86-64 com Rust 1.85.0: `cargo fmt --all -- --check`, `cargo clippy --locked --workspace --all-targets -- -D warnings`, `cargo test --locked --workspace --all-targets`, validação sintática do script e benchmark inicial passaram. Evidência: https://github.com/korczaktech/Nebula/actions/runs/41392238233 (commit `e3a2ad338f96b832bd652876e220114f8bd53858`).
