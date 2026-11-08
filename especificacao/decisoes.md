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
| Delimitadores | `()`, `[]`, `{}` e `<>` conforme o contexto gramatical | Cada delimitador possui função própria; `{}` delimita Function Body e também expressões de dados quando a gramática assim define |
| Blocos | Function Body `: { ... }` | As chaves delimitam os corpos; indentação obrigatória valida a estrutura e `fim` é identificador comum |
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

## Decisão normativa de Function Body

A sintaxe normativa de blocos usa Function Body explícito, iniciado por `: {` e encerrado pela chave correspondente `}`:

~~~nebula
funcao iniciar(): {
    quando verdadeiro: {
        passar
    }
    senao: {
        passar
    }
}
~~~

Regras:
- Todo corpo executável ou estrutural abre com `: {` e fecha com `}`.
- Cada bloco aninhado possui sua própria chave de fechamento.
- O Lexer preserva as chaves como tokens distintos; o Parser encerra o corpo ao consumir a chave `}` correspondente.
- A indentação é obrigatória e deve refletir o aninhamento das chaves; ela não substitui os delimitadores.
- A palavra `fim` não é reservada, não fecha blocos e é classificada como identificador comum.
- A sintaxe antiga baseada somente em indentação é rejeitada.
- Chaves em expressões de objetos e agregados continuam sendo interpretadas conforme o contexto gramatical de dados.
- Linhas vazias e comentários não alteram o pareamento das chaves.
- Espaços são o padrão; tabulações na indentação são diagnosticadas.

A gramática normativa completa está em `especificacao/gramatica.md`, `especificacao/gramatica-blocos.md` e `especificacao/tokens.md`.

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
