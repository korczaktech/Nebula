# Registro de decisões da Nebula

Este documento separa decisões adotadas, metas e questões ainda abertas. Uma proposta não deve ser tratada como funcionalidade implementada.

## Decisões adotadas na fundação

| Tema | Decisão | Consequência |
|---|---|---|
| Nome e extensão | Nebula / `.nbx` | Arquivos-fonte usam a extensão oficial |
| Idioma | Português brasileiro como idioma principal | Palavras-chave, mensagens e documentação oficiais em português |
| Implementação inicial | Rust | Compilador começa em Rust |
| Modelo | Linguagem compilada e multiparadigma | Arquitetura com IR e backends |
| Tipagem | Estática por padrão, inferência controlada | Verificação de tipos antes da geração |
| Atribuição | `=` | Inicialização e reatribuição usam o mesmo operador |
| Igualdade | `==` | Não confundir com atribuição |
| Separação | Nova linha ou `;` | Separador explícito opcional quando a quebra de linha já é clara |
| Delimitadores | `()`, `{}`, `[]` aceitos como recursos de agrupamento | Não obrigar delimitadores redundantes |
| Blocos | Chaves ou terminador `fim` | Indentação não define escopo |
| Codificação | UTF-8 e NFC para identificadores | Comparação de nomes consistente |
| Memória | Modelo híbrido planejado | Caminho seguro e controle de baixo nível |
| Backends | Nativo e WebAssembly como alvos planejados | IR independente de plataforma |
| LLVM | Não é dependência obrigatória | Backend pode usar outras estratégias |
| Autocompilação | Meta de longo prazo | Implementação inicial permanece em Rust |

## Metas, não garantias

- Investigar desempenho que possa superar C++ em cargas específicas.
- Buscar otimização agressiva sem alterar a semântica.
- Atender desde aplicações comuns até kernels, drivers e sistemas embarcados.
- Construir uma biblioteca padrão modular.

Resultados de desempenho só podem ser anunciados com benchmarks reproduzíveis e comparações equivalentes.

## Questões deliberadamente abertas

1. Semântica completa de propriedade, empréstimos, mutabilidade e destruição.
2. Largura padrão de `inteiro` por plataforma e regras de ABI.
3. Semântica exata de overflow e ponto flutuante.
4. Gramática final de expressões multilinha e chamadas sem parênteses.
5. Tipos algébricos, genéricos, opcionais e inferência avançada.
6. Modelo de concorrência, tarefas e atomics.
7. Sistema de pacotes, resolução de dependências e versionamento.
8. Backend nativo e estratégia de ligação.
9. Escopo e momento da autocompilação.
10. Licença de distribuição do projeto.

Essas questões devem ser decididas antes de estabilizar as áreas correspondentes. A Fase 1 não deve inventar decisões irreversíveis para tópicos que ainda precisam de projeto e validação.
