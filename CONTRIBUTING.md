# Contribuindo com a Nebula

## Princípios

- Leia primeiro `README.md` e os documentos em `especificacao/`.
- Não implemente uma sintaxe que contradiga a gramática aprovada.
- Mudanças na linguagem devem atualizar especificação, exemplos e fixtures.
- Não anuncie recursos como concluídos sem testes.
- Prefira dependências pequenas e justificadas.
- Preserve mensagens e documentação oficiais em português.
- Código inseguro deve ser evitado; qualquer exceção futura exige justificativa e revisão.

## Verificações locais

Com Rust 1.85.0 instalado (versão fixada em rust-toolchain.toml):

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace --all-targets
```

## Escopo atual

A Fase 1 estabelece especificação, workspace e estruturas-base. Lexer, Parser, AST completa, semântica e geração de código ainda não estão implementados. Contribuições não devem preencher essas lacunas com stubs que aparentem funcionalidade real.
