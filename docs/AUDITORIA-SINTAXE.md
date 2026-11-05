# Auditoria de conformidade sintática — Nebula

**Repositório:** `korczaktech/Nebula`  
**Base examinada:** branch `main`  
**Documentos:** `NEBULA-SINTAXE.md` e `NEBULA-SINTAXE-2.md`  
**Implementação examinada:** Lexer, tokens, Parser, AST e testes de conformidade  
**Escopo:** reconhecimento léxico e sintático. Não certifica execução, ABI, geração de código, runtime ou hardware real.

## 1. Critério de classificação

- **Implementado no Lexer/Parser:** o código reconhece a construção e a representa na AST.
- **Parcial:** a forma básica é reconhecida, mas variantes ou restrições descritas não são cobertas integralmente.
- **Apenas sintaxe genérica:** o Parser aceita nomes, tipos genéricos, acesso a membros ou chamadas, mas não comprova que a API exista nem valida seu comportamento.
- **Planejado / sem validação semântica:** a documentação define o objetivo, mas não há implementação semântica comprovada no Lexer/Parser.
- **Não reconhecido:** a forma documentada é rejeitada ou não possui produção correspondente.

A aceitação sintática de `memoria.ler<T>()`, por exemplo, não prova que exista uma operação de memória implementada.

## 2. Fontes de implementação examinadas

- `compilador/sintaxe/src/lexer/token.rs`: catálogo de tokens.
- `compilador/sintaxe/src/lexer/scanner.rs`: palavras reservadas, operadores, literais, comentários, delimitadores e indentação.
- `compilador/sintaxe/src/lib.rs`: Parser, produções, recuperação e testes unitários.
- `compilador/sintaxe/src/ast/`: nós de declaração, instrução, expressão, tipo e padrões.
- `compilador/sintaxe/tests/conformidade.rs`: testes externos de conformidade.
- Testes internos em `compilador/sintaxe/src/lib.rs`.

O Parser contém testes específicos para produções normativas, alternativas, precedência, tipos, blocos indentados, estruturas, enumerações, padrões, operadores, atributos, recursos nativos e recuperação de erros. A existência desses testes foi inspecionada; esta auditoria remota não executou `cargo test`.

## 3. NEBULA-SINTAXE.md — conferência por seção

| Seção | Construções examinadas | Estado |
|---|---|---|
| 1 — Identidade e nomenclatura | Identificadores, UTF-8, ausência de acentos e normalização NFC | Implementado no Lexer, com diagnóstico para acentos e normalização |
| 2 — Regras obrigatórias | Delimitadores, indentação, blocos e `fim` | Implementado; blocos de instrução exigem indentação e `fim`; chaves não substituem blocos |
| 3 — Palavras reservadas | Palavras de controle, tipos, modificadores, contexto e aliases | Parcial; ver matriz de palavras reservadas abaixo |
| 4 — Condicionais | `se`, `sendo`, `senao` | Implementado; `se` e alias `quando`; cláusulas são associadas ao mesmo `fim` |
| 4 — Seleção | `escolha`, `caso`, `padrao` | Implementado com restrições: literal, variante `Enum::Variante`, payload nomeado e curinga; não aceita expressão arbitrária como padrão |
| 5 — Repetições | `para`, `enquanto`, `pare`, `prossiga`, `passar` | Implementado; `para` também possui forma C com cabeçalho entre parênteses |
| 6 — Variáveis e constantes | Declaração tipada, inferência e atribuição | Implementado sintaticamente; verificação de mutabilidade/tipos pertence à semântica |
| 7 — Tipos de dados | Tipos nomeados, lista, tupla e genéricos | Parcial; o Parser reconhece nomes, listas, tuplas, tipos de função e genéricos, mas não valida por si só todos os tipos citados |
| 8 — Operadores matemáticos | `+`, `-`, `*`, `/`, `\\\\`, `%`, `**` | Implementado no Lexer/Parser |
| 9 — Comparação | Igualdade, diferença, igualdade estrita e relacionais | Implementado no nível sintático; semântica exata dos operadores depende da etapa semântica |
| 10 — Lógica | `e`, `ou`, `nao`, `&&`, `||`, `!` | Implementado; `nao` é interpretado como literal falso quando está em posição de valor isolado e como unário quando seguido de expressão |
| 11 — Atribuições | `=`, `=+`, `=-`, `=*`, `=/`, `=\\\\`, `=%`, `=**`, `++`, `--` | Implementado; os aliases convencionais `+=`, `-= ` etc. não são a grafia documentada/implementada |
| 12 — Funções | Funções, parâmetros tipados, padrão, genéricos, retorno, lambda, seta e `ceder` | Implementado com limites; parâmetros são tipados; lambda e seta têm formas próprias; corpo usa bloco indentado |
| 13 — Classes e objetos | Classe, construtor, herança, métodos, modificadores e instanciação | Parcial; classe/herança e modificadores de função são reconhecidos; visibilidade de campos e semântica de objeto não são provadas pelo Parser |
| 14 — Importação/exportação | `incluir`, `biblioteca`, `modulo`, `de`, `como`, `exportar funcao` | Implementado sintaticamente; resolução/carregamento de módulos é outra etapa |
| 15 — Erros | `tente`, `erro`, `sempre`, `lancar`, `verifique` | Implementado sintaticamente; `tente` exige ao menos `erro` ou `sempre`; tipos e runtime de erro não são certificados aqui |
| 16 — Escopo | `global`, `externa`, `isto` | Implementado sintaticamente como declarações/nomes; resolução de escopo é semântica |
| 17 — Assíncrono | `assinc`, `aguarda` | Parcial; marcadores e expressão unária são reconhecidos, mas escalonamento/runtime assíncrono não é comprovado |
| 18 — Entrada/saída e arquivos | Nomes de funções, chamadas, bloco `com` | Apenas sintaxe genérica; as funções de I/O não são validadas pelo Parser como intrínsecos |
| 19–21 — Métodos | Acesso a membro e chamadas de texto, lista e dicionário | Apenas sintaxe genérica; o Parser reconhece `.` e chamadas, mas não garante que os métodos existam |
| 22 — Funções nativas | Catálogo de funções matemáticas, conversão, coleções, memória, texto e sistema | Planejado/API; nomes podem ser chamados sintaticamente, mas o catálogo não é implementado pelo Parser |
| 23 — Operadores adicionais | Ternário, `??`, `?.`, `??=`, `...`, bitwise e shifts | Implementado no nível léxico/sintático, com semântica a validar por tipo e backend |
| 24 — Strings | Aspas simples/duplas, triplas duplas, interpolação e escapes | Parcial; Lexer reconhece as formas e verifica estrutura da interpolação; a interpolação é tokenizada como literal e não é analisada como AST de expressão Nebula independente |
| 25 — Comentários | `#`, `//`, `/* ... */` | Implementado no Lexer |
| 26 — Precedência | Ordem e associatividade | Divergência documental corrigida nesta auditoria; a tabela foi alinhada aos níveis do Parser |
| 27 — Exemplo completo | Composição de classes, funções, controle e strings | Construções individuais reconhecidas conforme os limites acima; não equivale a teste de execução integral |
| 28 — Referências de nomenclatura | Aliases de palavras reservadas | Informativo; alguns aliases não são sinônimos exatos e são tratados separadamente no Lexer |
| 29 — EBNF preliminar | Programa, declarações, blocos e controle | Desatualizada/incompleta; não deve ser usada como gramática normativa. A referência normativa é `especificacao/gramatica.md` |
| 30 — Compilação e execução | Pipeline e comandos CLI | Fora do Lexer/Parser; a gramática não prova que cada comando CLI esteja disponível |
| 31 — Regras consolidadas | Resumo de regras | Parcial; deve ser interpretado junto da gramática normativa e deste relatório |
| 32 — Pontos a fechar | Semântica, tipos, memória, ABI e runtime | Itens de especificação/implementação posterior, não produções do Parser |

## 4. Palavras reservadas — estado observado

### Reconhecidas como tokens e usadas em construções

`variavel`, `constante`, `funcao`, `classe`, `herda`, `se`, `sendo`, `senao`, `para`, `dentro`, `enquanto`, `escolha`, `caso`, `padrao`, `pare`, `prossiga`, `passar`, `retorne`, `lambda`, `ceder`, `incluir`, `biblioteca`, `modulo`, `de`, `como`, `tente`, `erro`, `sempre`, `lancar`, `verifique`, `com`, `assinc`, `aguarda`, `global`, `externa`, `isto`, `novo`, `apagar`, `identico`, `nao`, `fora`, `e`, `ou`, `sim`, `verdadeiro`, `falso`, `nulo`, `iguais`, `tipoDe`, `instanciaDe`, `obter`, `definir`, `vazio`, `exportar`, `construtor`, `estatico`, `privado`, `publico`, `protegido`, `seguro`, `critico`, `privilegio`, `montagem`, `tipo`, `estrutura`, `enumeracao`.

### Reconhecidas como aliases ou formas adicionais

- `quando` também inicia condicional.
- `avaliar` também inicia seleção.
- `durante` e `repetir` também iniciam repetição condicional.
- `cada` com `em` inicia iteração sobre coleção.
- `pulso` inicia repetição por intervalo.
- `devolver`, `parar`, `encerrar` e `seguir` são aliases de controle.
- `mutavel` e `definir` são aliases de declaração de variável; `alterar` exige uma expressão de atribuição.
- `verdadeiro` e `falso` também são aceitos como literais lógicos.

### Reservadas, mas não disponíveis como construção independente

- `erro` e `sempre` são cláusulas de `tente`; fora do contexto, não são declarações comuns.
- `herda` é consumido no cabeçalho de classe.
- `obter` pode ser usado como nome/chamada, mas não há declaração própria de getter. `definir` é aceito como alias de declaração de variável; não é uma produção de setter de propriedade.
- `tentar`, `capturar` e `finalmente` são tokens reservados para o futuro e geram diagnóstico de construção não implementada.

## 5. Operadores — divergência de precedência corrigida

O Parser usa estes níveis, do mais forte para o mais fraco:

1. Pós-fixos: chamada, indexação, membro, acesso opcional.
2. Potenciação.
3. Unários.
4. Multiplicação/divisão/divisão inteira/resto.
5. Adição/subtração.
6. Deslocamentos.
7. Intervalo.
8. Relacionais/pertencimento.
9. Igualdade.
10. E bit a bit.
11. XOR bit a bit.
12. OU bit a bit.
13. E lógico.
14. Coalescência.
15. OU lógico.
16. Atribuição/ternário/propagação e seta em contexto de expressão.

O número interno de precedência do código é crescente para operadores mais fortes; a tabela apresentada no documento principal agora explicita essa convenção. Comparações relacionais encadeadas e intervalos encadeados são rejeitados pelo Parser. Potenciação associa à direita.

### Atribuição composta

O Scanner reconhece tanto a grafia Nebula com o sinal de atribuição antes do operador (`=+`, `=-`, `=*`, `=/`, `=\\`, `=%`, `=**`) quanto os aliases convencionais (`+=`, `-=`, `*=`, `/=`, `\\=`, `%=`, `**=`). Ambas as grafias geram os mesmos tokens de atribuição composta. A divisão inteira usa duas barras invertidas, não `//`.

## 6. NEBULA-SINTAXE-2.md — conferência por seção

| Seção | Construções examinadas | Estado |
|---|---|---|
| 1 — Objetivos e princípios | Regras gerais e requisitos de implementação | Documento de projeto; não é produção sintática |
| 2 — Níveis de acesso | `seguro`, `critico`, `privilegio sistema`, `montagem arquitetura` | Blocos contextuais reconhecidos; autorização e permissões não são verificadas pelo Parser |
| 3 — Tipos fundamentais | Tipos fixos, endereço, tamanho, ponteiros, vetor e bloco genéricos | Apenas sintaxe genérica de tipos; não há prova de layout/semântica de todos esses tipos |
| 4 — Alinhamento e layout | Atributos `@...`, declaração de estrutura e campos | Sintaxe de atributos e estrutura reconhecida; atributos não são validados semanticamente pelo Parser |
| 5 — Memória | Chamadas `memoria.*`, genéricos, argumentos nomeados e `?` | Apenas sintaxe genérica; não comprova API, segurança, alocação ou mapeamento |
| 6 — Ponteiros e referências | Tipos genéricos, chamada `ponteiro.de`, `&valor`, `*referencia` | Parcial; tipo genérico e operador prefixo `&` são analisáveis; dereferência prefixa `*referencia` não possui produção unária |
| 7 — Bits | `&`, `|`, `^`, `~`, shifts e chamadas `bits.*` | Operadores reconhecidos; funções são chamadas genéricas sem validação de API |
| 8 — CPU e registradores | Acesso qualificado e chamadas `cpu.*` | Apenas sintaxe genérica; não valida registradores por alvo |
| 9 — Instruções CPU/SIMD | Chamadas e acesso a membros | Apenas sintaxe genérica; intrínsecos e disponibilidade de instruções não comprovados |
| 10 — Barreiras | Caminhos de módulo e chamadas | Apenas sintaxe genérica; nenhuma garantia de ordenação de memória |
| 11 — Hardware/MMIO | Chamadas, tipos genéricos e argumentos nomeados | Apenas sintaxe genérica; sem validação de faixa, alinhamento, volatilidade ou privilégio |
| 12 — Portas I/O | Chamadas `porta.ler/escrever*` em bloco privilegiado | Forma de chamada e bloco contextual reconhecidos; instruções e arquitetura não implementadas pelo Parser |
| 13 — Interrupções | Funções manipuladoras e chamadas de registro | Apenas sintaxe genérica; sem ABI de interrupção ou restrições de contexto |
| 14 — DMA | Tipos, chamadas, argumentos nomeados e propagação `?` | Apenas sintaxe genérica; sem runtime/driver/validação DMA |
| 15 — Atômicas | Tipos genéricos e chamadas | Apenas sintaxe genérica; sem garantias atômicas ou modelo de memória |
| 16 — Cache | Chamadas e caminhos qualificados | Apenas sintaxe genérica; sem intrínsecos ou coerência |
| 17 — Tempo | Chamadas qualificadas | Apenas sintaxe genérica |
| 18 — Energia | Chamadas qualificadas | Apenas sintaxe genérica; não executa transições de energia |
| 19 — Subsistemas de hardware | Caminhos de módulos e chamadas | Catálogo de API planejada; nomes são analisáveis como membros/chamadas |
| 20 — Assembly embutido | Expressão `montagem arquitetura<T>(argumentos)` | Reconhecimento sintático parcial; não prova validação de Assembly nem geração de instruções |
| 21 — ABI e símbolos | Atributos e `externa funcao` | Sintaxe reconhecida; convenção/layout/símbolos não são validados pelo Parser |
| 22 — Seções do executável | `@secao` e funções | Sintaxe de atributo reconhecida; linker não é validado |
| 23–25 — Exemplos integrados | MMIO, interrupção e DMA | Exemplos são analisáveis em sua estrutura geral, mas chamadas/tipos são nomes genéricos e não demonstram funcionalidade |
| 26 — Otimização | Regras e restrições | Contrato de projeto, fora da análise sintática |
| 27 — Portabilidade | Capacidades e alvos | Contrato de projeto, fora da análise sintática |
| 28 — Perfis | `aplicacao`, `sistema`, `kernel`, `baremetal`, `biblioteca` | Nomes de perfil não são configurados/validados pelo Parser |
| 29 — Diagnósticos | Lista de erros de hardware/ABI | Diagnósticos propostos; Parser atual só emite diagnósticos léxicos/sintáticos que lhe cabem |
| 30 — Gramática adicional | Blocos contextuais, tipos genéricos, atributos e montagem | Parcial; blocos e atributos são reconhecidos; produção de montagem foi esclarecida no documento |
| 31 — Biblioteca padrão | Catálogo de módulos | Planejado; não prova APIs implementadas |
| 32 — Estratégia | Etapas de desenvolvimento | Plano, não gramática |
| 33 — Critérios | Condições de conclusão | Critérios, não implementação |
| 34 — Relação | Hierarquia entre documentos | Informativo |

## 7. Diferenças sintáticas concretas encontradas

1. **Precedência:** tabela do documento principal divergente do Parser. Corrigida para refletir os níveis e a ordem usados no código.
2. **Aliases de atribuição:** `+=`, `-=`, `*=`, `/=`, `//=`, `%=`, `**=` não são reconhecidos como aliases. A forma implementada é `=+`, `=-`, `=*`, `=/`, `=\\`, `=%`, `=**`.
3. **Dereferência:** `*referencia` aparece como possibilidade no documento nativo, mas não é aceita como operador unário pelo Parser. O próprio documento já a classifica como proposta a confirmar; permanece planejada.
4. **Interpolação:** o Lexer verifica delimitadores e conteúdo de `${...}`, mas o Parser recebe o texto interpolado como token literal. Não há AST independente para cada expressão interpolada.
5. **Getter/setter:** `obter` e `definir` são palavras reconhecidas, mas não existe uma produção de declaração de propriedade getter/setter.
6. **EBNF da seção 29 do documento principal:** é explicitamente preliminar e não cobre integralmente o Parser atual. A gramática normativa é `especificacao/gramatica.md`.
7. **EBNF de montagem no documento nativo:** a alternativa `| "fim"` estava malformada como produção isolada. O documento agora esclarece as duas formas contextuais distintas.

## 8. O que a auditoria não declara concluído

Esta conferência não certifica:
- existência ou comportamento de todas as funções nativas listadas;
- resolução de módulos/importações;
- inferência e compatibilidade de tipos em todos os casos;
- semântica de herança, visibilidade e propriedades;
- execução de tarefas assíncronas;
- interpolação com avaliação de expressões;
- segurança de ponteiros, alocação ou empréstimos;
- instruções CPU, SIMD, Assembly ou registradores;
- MMIO, portas, interrupções, DMA, cache, atomics ou energia;
- ABI, seções de executável, linker ou perfis bare-metal;
- geração de código ou execução no ZenithOS.

Esses itens exigem auditorias próprias das etapas semântica, IR, backend, runtime, biblioteca e testes de execução.

## 9. Testes e validação

Foram inspecionados os testes já existentes no Parser. Entre eles há testes para:
- produções normativas e combinações de gramática;
- palavras reservadas e aliases;
- tipos genéricos, tipos de função e tuplas;
- blocos indentados, `fim`, estruturas e enumerações;
- precedência, associatividade e operadores;
- atributos, contextos nativos e montagem;
- recuperação de erros e limites de aninhamento.

**Não foi executado `cargo test`, `cargo fmt`, `cargo clippy` ou CI nesta auditoria.** A inspeção do código e dos testes não substitui sua execução.

## 10. Conclusão

A conferência sintática documental está concluída: as construções dos dois documentos foram classificadas conforme o Lexer/Parser atual, as divergências concretas foram registradas e as duas especificações receberam referências explícitas para esta matriz.

A conclusão é sobre **conformidade documental e reconhecimento sintático**, não sobre implementação funcional de todas as APIs de Nebula Nativo.
