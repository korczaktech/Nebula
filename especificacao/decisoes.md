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
| Delimitadores de expressão/dados | `()`, `[]` e chaves em construções de dados | Delimitadores agrupam expressões e valores; chaves não substituem blocos de instrução |
| Blocos | Indentação obrigatória e terminador `fim` | `INDENT`/`DEDENT` delimitam o corpo e `fim` encerra explicitamente a construção |
| Codificação | UTF-8 e NFC para identificadores | Comparação de nomes consistente |
| Memória | Modelo híbrido planejado | Caminho seguro e controle de baixo nível |
| Backends | Nativo e WebAssembly como alvos planejados | IR independente de plataforma |
| LLVM | Não será dependência do compilador | O backend próprio gera código diretamente em Rust |
| Toolchain da fundação | Rust 1.85.0 fixado e Cargo.lock versionado | Builds reproduzíveis |
| Autocompilação | Meta de longo prazo | Implementação inicial permanece em Rust |

## Decisões adotadas na Fase 3

| Tema | Decisão | Consequência |
|---|---|---|
| Análise semântica | Executada sobre AST já válida | A CLI interrompe a etapa se Lexer ou Parser falharem |
| Escopos | Escopos léxicos com símbolos locais e globais | Sombreamento interno permitido; duplicidade no mesmo escopo rejeitada |
| Funções | Assinaturas pré-declaradas | Chamadas a funções declaradas adiante são resolvidas |
| Aliases | Aliases de tipo no escopo do arquivo, resolvidos antes dos corpos | Ciclos e tipos desconhecidos são erros |
| Módulos | Ambiente explícito de módulos e exports | Sem pesquisa arbitrária no disco ou resolvedor de pacotes nesta fase |
| Tipos numéricos | Larguras inteiras explícitas são tipos distintos | Conversões seguem regras semânticas explícitas |
| Constantes | Imutáveis após inicialização | Reatribuição é erro semântico |
| Valores | Semântica por valor no interpretador de referência | Não há referências compartilhadas expostas pela linguagem nesta fase |
| IR | Operadores e instruções normalizados | Backends futuros não precisam interpretar a gramática |
| Interpretador de referência | Limites de passos e profundidade de chamadas | Testes não podem executar laços/recursões sem limite |
| CLI | Comandos verificar, analisar e ir têm responsabilidades separadas | Sintaxe, semântica e inspeção de IR são observáveis separadamente |

## Decisão normativa de blocos indentados

A sintaxe de blocos foi atualizada para a forma indentada obrigatória:

~~~nebula
funcao iniciar() -> vazio
    quando verdadeiro
        passar
    senao
        passar
    fim
fim
~~~

Regras:
- Todo bloco de instrução começa após uma quebra de linha e aumento de indentação.
- O Lexer emite `INDENT` e `DEDENT` para marcar os limites físicos dos corpos.
- Cada bloco completo termina com `fim`, depois do `DEDENT` correspondente.
- Blocos aninhados fecham do mais interno para o mais externo.
- Chaves não podem ser usadas como alternativa de bloco de instrução.
- Chaves permanecem permitidas em construções de dados, como `Pessoa{nome: "Ana"}`.
- Linhas vazias e comentários não alteram a pilha de indentação.
- Espaços são o padrão; tabulações na indentação são diagnosticadas.
- Dentro de parênteses e colchetes, quebras são continuação de linha; dentro de chaves, a indentação é suprimida, mas quebras podem separar campos.

A gramática completa e os detalhes de continuação de linha estão em `especificacao/gramatica.md` e `especificacao/tokens.md`.

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
4. Gramática futura de construções que ainda não fazem parte do Parser.
5. Modelo de concorrência, tarefas e atomics.
6. Sistema de pacotes, resolução de dependências e versionamento.
7. Estratégia geral de ligação, símbolos e relocação para alvos futuros.
8. Escopo e momento da autocompilação.
9. Licença de distribuição do projeto.

Questões abertas não alteram as produções normativas já aceitas pelo frontend.

## Decisões arquiteturais da reestruturação da Fase 4

| Tema | Decisão | Consequência |
|---|---|---|
| Backend nativo | Gerar instruções de máquina e executável diretamente em Rust | Não gerar C/C++ nem assembly textual |
| Dependências de toolchain | Não exigir compilador C/C++, LLVM, Cranelift, assembler ou linker externo | A CLI não delega a compilação nativa a outra ferramenta |
| Primeiro alvo | Linux x86-64, ELF64 | O backend rejeita alvos ainda não implementados |
| Runtime | Evolução independente do backend | Recursos de biblioteca padrão seguem o escopo de runtime |
| Otimizador | Implementado na IR | Transformações devem preservar a semântica |
| Testes do backend | Testes unitários Rust e integração executando os executáveis produzidos | Nenhum teste depende de código C++17 |
