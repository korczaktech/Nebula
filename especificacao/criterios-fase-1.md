# Critérios de aceite — Fase 1

**Estado: concluída após revisão documental e validação automatizada.**

Este documento define somente os critérios da Fase 1 — Fundação e especificação. Não exige Lexer, Parser, AST completa, análise semântica ou backend, que pertencem às fases seguintes.

| Critério | Evidência | Estado |
|---|---|---|
| Identidade, objetivos e limites da linguagem documentados | linguagem.md e README.md | Concluído |
| Tokens e regras léxicas documentados | tokens.md | Concluído |
| Gramática inicial coerente e delimitada | gramatica.md | Concluído |
| Decisões adotadas separadas de metas e questões abertas | decisoes.md | Concluído |
| Tipos, memória, módulos, operadores, diagnósticos e alvos registrados | tipos.md, memoria.md, modulos.md, operadores.md, diagnosticos.md, alvos.md | Concluído |
| Workspace Rust e crates-base criados | Cargo.toml e compilador/ | Concluído |
| Tipos básicos de arquivo-fonte, intervalos e diagnósticos | crates nebula-fonte e nebula-diagnosticos | Concluído |
| CLI informativa sem simular compilação | crate nebula-cli | Concluído |
| Exemplos e fixtures identificados como não executados por Parser | exemplos/ e testes/conformidade/ | Concluído |
| Versão do toolchain e resolução de dependências fixadas | rust-toolchain.toml e Cargo.lock | Concluído |
| CI valida formatação, Clippy e testes com lockfile | .github/workflows/ci.yml | Concluído |
| Política de contribuição e changelog | CONTRIBUTING.md e CHANGELOG.md | Concluído |

## Limites explícitos

Os itens abaixo não são pendências da Fase 1: Lexer, Parser, AST, recuperação sintática, análise semântica, sistema de tipos executável, IR, otimizações, geração de código, backends, biblioteca padrão e benchmarks. Eles serão avaliados nos critérios das fases correspondentes.

A gramática desta fase é a definição normativa do subconjunto inicial que a Fase 2 deverá implementar. Recursos identificados como futuros ou decisões abertas não podem ser anunciados como disponíveis.

## Regra de encerramento

A Fase 1 só pode ser reaberta se uma inconsistência na fundação, na especificação ou na infraestrutura for descoberta. A aprovação da CI confirma o estado do código no commit indicado no histórico do GitHub; não prova funcionalidades que ainda não foram implementadas.
