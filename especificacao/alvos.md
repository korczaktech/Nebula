# Plataformas e alvos de compilação

## Alvos planejados

- Nativo x86-64.
- Nativo ARM64.
- WebAssembly.
- Sistemas embarcados conforme suporte de ABI e runtime.

## Requisitos arquiteturais

- AST e IR independentes de plataforma.
- Backend separado por arquitetura/ABI.
- Runtime opcional quando possível.
- Convenções de chamada e layout de dados documentados por alvo.
- Diagnóstico claro quando um recurso não for suportado pelo alvo.
- Testes específicos por alvo; não anunciar compatibilidade sem execução em CI ou ambiente de validação.

## Meta da Fase 1

Definir a separação arquitetural e registrar alvos pretendidos. Nenhum backend é entregue nesta fase.
