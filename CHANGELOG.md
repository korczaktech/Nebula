# Histórico de alterações

## 0.1.2 — Auditoria final da Fase 1

- Corrige a árvore de arquivos documentada e registra critérios formais de aceite da Fase 1.
- Remove produções de tipos sem tokens correspondentes e elimina ambiguidades na notação da gramática.

## 0.1.1 — Consolidação da fundação

- Sincroniza a gramática, as palavras reservadas e as regras léxicas do subconjunto inicial.
- Define explicitamente recursos reservados para versões futuras e evita apresentá-los como sintaxe disponível.
- Esclarece separação de linhas, precedência, comparação, blocos e limites de literais.
- Fixa a versão mínima/reprodutível do toolchain Rust e adiciona Cargo.lock.
- Fortalece a CI para validar o workspace usando o lockfile.

## 0.1.0 — Fundação inicial

- Registro da identidade e dos princípios da linguagem.
- Especificação inicial, gramática de trabalho e decisões arquiteturais.
- Workspace Rust e crates-base de fonte, diagnósticos e CLI informativa.
- Integração contínua para formatação, análise estática e testes.
- Nenhum Lexer, Parser, análise semântica ou backend implementado nesta versão.
