# Plataformas e alvos de compilação

## Alvo inicial em implementação

- Linux x86-64.
- Formato executável ELF64.
- ABI de chamadas System V AMD64 para funções e parâmetros suportados.
- O backend escreve o arquivo ELF e as instruções de máquina diretamente em Rust.
- Não exige compilador C/C++, assembler, linker, LLVM ou Cranelift externos.

Este é o único alvo do backend nativo inicial. O código deve rejeitar explicitamente outros sistemas e arquiteturas até que existam implementações e testes próprios.

## Alvos planejados

- Nativo ARM64.
- Windows x86-64 e macOS ARM64/x86-64.
- WebAssembly.
- Sistemas embarcados conforme suporte de ABI e runtime.

## Requisitos arquiteturais

- AST e IR independentes de plataforma.
- Backend próprio por arquitetura/ABI.
- Runtime opcional quando possível; não deve ser pressuposto pelo backend de baixo nível.
- Convenções de chamada e layout de dados documentados por alvo.
- Diagnóstico claro quando um recurso não for suportado pelo alvo.
- Testes específicos por alvo; não anunciar compatibilidade sem execução em CI ou ambiente de validação.
- Nenhum alvo pode exigir uma toolchain de outra linguagem como etapa obrigatória de compilação Nebula.

## Meta da Fase 1

Definir a separação arquitetural e registrar alvos pretendidos. Nenhum backend é entregue nesta fase.
