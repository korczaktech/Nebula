# Processo de lançamento da Nebula

## Situação

Nenhuma versão 1.0 é autorizada por este documento. O lançamento está bloqueado até que os critérios obrigatórios da Fase 5 estejam implementados, testados e auditados. A licença do projeto também precisa de decisão explícita dos mantenedores antes de distribuir binários ou fontes como versão estável.

## Versionamento

A partir da primeira versão estável, será usado SemVer:

- MAJOR: incompatibilidade em contrato público estável.
- MINOR: funcionalidade compatível.
- PATCH: correção compatível.

Tags de pré-lançamento devem usar sufixos SemVer, por exemplo `1.0.0-rc.1`. Tags estáveis devem seguir `vMAJOR.MINOR.PATCH`.

## Portão obrigatório

Antes de criar uma tag estável:

1. Todos os critérios obrigatórios de `especificacao/criterios-fase-5.md` devem estar aprovados ou formalmente retirados do escopo 1.0.
2. CI deve passar em todos os alvos declarados suportados.
3. Exemplos documentados devem compilar e executar.
4. Auditoria de dependências, licença e supply chain deve estar registrada.
5. Changelog e notas de migração devem estar atualizados.
6. Checksums devem ser publicados junto dos artefatos.
7. Assinaturas criptográficas exigem chave de release protegida; não usar chave ou segredo embutido no repositório.
8. A licença de distribuição deve estar definida e revisada.

## Artefatos atualmente possíveis

O backend validado gera ELF64 para Linux x86-64. Portanto, esse é o único artefato nativo que pode ser produzido nesta etapa. Windows, macOS, ARM64 e WebAssembly não podem aparecer como downloads suportados.

## Instalação

Até haver instaladores testados, a instalação de desenvolvimento usa Rust 1.85.0 e Cargo:

```sh
cargo build --locked --release -p nebula-cli
```

O binário de desenvolvimento é produzido em `target/release/nebula`. Não há ainda pacote de sistema, instalador universal ou assinatura de release.
