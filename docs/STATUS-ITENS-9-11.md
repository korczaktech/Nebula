# Estado dos itens 9 a 11 da Fase 5

Este registro descreve somente as entregas desta rodada. Não altera os critérios integrais de aceite da Fase 5.

## Item 9 — Ferramentas

**Estado: parcial.**

Implementado nesta rodada:
- Binário independente `nebula-formatar`, registrado no pacote CLI.
- Validação sintática antes de qualquer alteração.
- Normalização conservadora de espaços finais, remoção de linhas vazias finais e quebra de linha final.
- Modo `--check`, que não grava alterações.
- Gravação temporária seguida de substituição, rejeitando links simbólicos.
- Testes unitários para idempotência, preservação de conteúdo, modo de verificação e rejeição de sintaxe inválida.

Pendente:
- Integrar `formatar` como subcomando do executável principal `nebula`.
- Formatador estilístico completo com indentação e regras canônicas.
- Cache/compilação incremental com invalidação por dependências.
- Completar e validar todos os comandos oficiais `criar`, `limpar` e os demais fluxos da CLI.

## Item 10 — Qualidade e segurança

**Estado: parcial.**

Implementado nesta rodada:
- Workflow periódico e por push/PR para `cargo audit --locked`.
- Dependabot semanal para dependências Cargo e GitHub Actions.
- Workflow periódico de fuzz smoke determinístico para entradas malformadas do Lexer/Parser.
- Política de relato responsável em `.github/SECURITY.md`.
- Documentação dos limites das auditorias e dos testes de fuzz smoke.

Pendente:
- Campanhas contínuas de fuzzing com corpus e cobertura.
- Fuzzing de pacotes, serialização e demais interfaces críticas.
- Matriz de execução real para todos os alvos que vierem a ser suportados.
- Auditoria independente de código, supply chain e segurança.
- Relatórios de stress e baseline versionada de benchmarks representativos.

## Item 11 — Documentação e lançamento

**Estado: parcial.**

Implementado nesta rodada:
- Documentos de estabilidade e compatibilidade, processo de lançamento, ferramentas e segurança.
- Índice de documentação e política de relato de vulnerabilidades.
- Workflow de release restrito ao Linux x86-64, com validação e checksums.
- Gate que impede distribuição enquanto não existir um arquivo `LICENSE` aprovado.
- Registro explícito de que `UNLICENSED` no Cargo não concede direitos de redistribuição.

Pendente:
- Decisão e revisão jurídica da licença e criação do arquivo `LICENSE`.
- Assinatura criptográfica dos artefatos e proteção/gestão das chaves.
- Instaladores e pacotes para plataformas oficialmente suportadas.
- Guia completo de migração e referências finais da linguagem, runtime, biblioteca, pacotes e backends.
- Congelamento e auditoria dos contratos estáveis da versão 1.0.

## Validação

Os novos workflows foram adicionados ao repositório, mas sua aprovação ainda precisa ser observada no GitHub Actions. A última execução de CI conhecida antes desta rodada falhou em `cargo fmt --check` por formatação preexistente em `compilador/backend/src/lib.rs`; Clippy e testes não foram executados naquela execução. Portanto, não há base para declarar os itens concluídos nem a Fase 5 como 100%.
