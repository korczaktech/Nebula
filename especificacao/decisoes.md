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
| LLVM | Não será dependência do compilador | O backend próprio gera código diretamente em Rust |
| Toolchain da fundação | Rust 1.85.0 fixado e Cargo.lock versionado | Builds da fundação usam versão e resolução de dependências reproduzíveis |
| Autocompilação | Meta de longo prazo | Implementação inicial permanece em Rust |

## Decisões adotadas na Fase 3

| Tema | Decisão | Consequência |
|---|---|---|
| Análise semântica | Executada sobre AST já válida | A CLI interrompe a etapa se Lexer ou Parser falharem |
| Escopos | Escopos léxicos com símbolos locais e globais | Sombreamento interno permitido; duplicidade no mesmo escopo rejeitada |
| Funções | Assinaturas pré-declaradas | Chamadas a funções declaradas adiante são resolvidas |
| Aliases | Aliases de tipo no escopo do arquivo, resolvidos antes dos corpos | Ciclos e tipos desconhecidos são erros |
| Módulos | Ambiente explícito de módulos e exports | Sem pesquisa arbitrária no disco ou resolvedor de pacotes nesta fase |
| Tipos numéricos | Larguras inteiras explícitas são tipos distintos; literais inteiros e decimais só são contextualizados dentro da mesma família numérica quando cabem no destino | Variáveis e expressões não literais não recebem conversão implícita geral |
| Constantes | Imutáveis após inicialização | Reatribuição é erro semântico |
| Valores | Semântica por valor no interpretador de referência | Não há referências compartilhadas expostas pela linguagem nesta fase |
| IR | Operadores e instruções normalizados | Backends futuros não precisam interpretar a gramática |
| Interpretador de referência | Limites de passos e profundidade de chamadas | Testes não podem executar laços/recursões sem limite |
| CLI | Comandos verificar, analisar e ir têm responsabilidades separadas | Sintaxe, semântica e inspeção de IR são observáveis separadamente |

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


## Decisões arquiteturais da reestruturação da Fase 4

| Tema | Decisão | Consequência |
|---|---|---|
| Backend nativo | Gerar instruções de máquina e executável diretamente em Rust | Não gerar C/C++ nem assembly textual |
| Dependências de toolchain | Não exigir compilador C/C++, LLVM, Cranelift, assembler ou linker externo | A CLI não delega a compilação nativa a outra ferramenta |
| Primeiro alvo | Linux x86-64, ELF64 | O backend rejeita alvos ainda não implementados |
| Runtime | Não antecipar sua implementação | Recursos de biblioteca padrão e I/O aguardam a etapa definida |
| Otimizador | Implementar somente após os recursos-base do compilador | Não oferecer níveis de otimização delegados a ferramentas externas |
| Testes do backend | Testes unitários Rust e integração executando os executáveis produzidos | Nenhum teste depende de código C++17 |

O backend nativo inicial já gera código de máquina e ELF64 para o subconjunto documentado de Linux x86-64. A validação automatizada final ainda está pendente; tipos, instruções, recursos de memória e alvos fora desse subconjunto continuam adiados.
