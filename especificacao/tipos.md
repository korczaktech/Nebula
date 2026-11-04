# Sistema de tipos — Nebula

A especificação normativa está em [linguagem-e-tipos.md](linguagem-e-tipos.md). Este documento descreve o estado verificável da implementação. Os critérios completos de aceite estão em [criterios-fase-5.md](criterios-fase-5.md).

## Modelo implementado

- Tipagem estática por padrão, com inferência local e determinística.
- Tipos explícitos nas interfaces de funções e variáveis anotadas.
- Sem coerção implícita geral entre texto, caractere, lógico e números.
- Tipos numéricos de larguras diferentes permanecem distintos em atribuições, parâmetros e retornos.
- Literais numéricos recebem tipo contextual somente quando o valor cabe exatamente no destino.
- Operações numéricas usam promoção determinística; mistura implícita de inteiros com e sem sinal é rejeitada.
- Overflow inteiro é verificado na IR para larguras assinadas e não assinadas de 8 a 128 bits.
- decimal32 é arredondado para IEEE-754 binary32 após cada operação na IR.

## Tipos reconhecidos

- vazio, logico, inteiro.
- inteiro8, inteiro16, inteiro32, inteiro64, inteiro128.
- inteiro_u8, inteiro_u16, inteiro_u32, inteiro_u64, inteiro_u128.
- decimal, decimal32, decimal64.
- byte, caractere, texto.
- Listas homogêneas [T], também nomeadas por `vetor<T>`.
- Matrizes como listas de listas homogêneas, por meio do alias `matriz<T>` (equivalente a `[[T]]`).
- Tuplas heterogêneas Tupla<T1, T2, ...> e expressões tupla, incluindo () e (valor,).
- Tipos nominais de estruturas e enumerações.
- Opcional<T> e Resultado<T, E>.
- Tipos de função funcao(T1, T2): Retorno, inclusive aninhados.
- Parâmetros genéricos em funções, com unificação estrutural e monomorfização de instâncias concretas na IR.

Aliases são resolvidos antes da análise dos corpos. Ciclos e destinos desconhecidos são rejeitados. Tipos genéricos que dependam de informação ausente — por exemplo, Opcional::Nenhum() sem tipo contextual — não podem ser usados para fabricar uma instância genérica concreta.

## Regras estáticas

- Condições de quando, enquanto, durante, repetir e para devem ser lógicas.
- O laço `cada nome em coleção` percorre listas, textos (elementos caractere) e intervalos numéricos; a variável de iteração é imutável dentro do corpo.
- `pulso(ms)` exige intervalo inteiro não negativo; o corpo é repetido até `parar`/`encerrar` ou retorno de função.
- `gerar(valor, quantidade)` produz lista com o valor repetido; quantidade negativa ou maior que 1.000.000 é rejeitada pelo interpretador.
- `em_paralelo(função1, função2, ...)` exige funções sem parâmetros e com tipo de retorno igual; executa com snapshots independentes da memória, devolvendo resultados na ordem dos argumentos. Não compartilha alterações entre as tarefas.
- Aritmética exige operandos numéricos; texto + texto concatena.
- Comparações relacionais exigem operandos numéricos.
- Igualdade estrutural é verificada para listas, tuplas, estruturas, enumerações, opcionais e resultados quando seus componentes implementam igualdade.
- Listas devem ser homogêneas; lista vazia exige tipo esperado explícito.
- Índices de listas aceitam tipos inteiros suportados; tipos decimais são rejeitados.
- Índices de tuplas precisam ser literais válidos e estar dentro dos limites conhecidos em compilação.
- Chamadas diretas e indiretas validam aridade, tipos dos argumentos e retorno.
- Funções com retorno não vazio devem devolver um valor em todos os caminhos condicionais reconhecidos pelo verificador.
- Literais e conversões numéricas explícitas são verificados contra a faixa do tipo de destino.

## Tipos compostos e funções

Estruturas e enumerações percorrem Lexer, Parser, AST, análise semântica, IR e interpretador. Construções de estruturas validam campos ausentes, duplicados, desconhecidos e incompatíveis. Enumerações validam variantes, payloads e correspondência exaustiva. Opcional<T> e Resultado<T,E> possuem construtores e correspondência de padrões executáveis no interpretador.

A IR calcula e armazena layouts físicos de estruturas e enumerações, incluindo tamanho, alinhamento, padding, deslocamentos e discriminantes. O runtime também calcula layouts para os tipos suportados, detectando recursão por valor e tipos sem representação física.

Valores de função sem captura de ambiente podem ser armazenados, retornados e chamados indiretamente pelo interpretador. O backend ELF x86-64 gera referências e chamadas indiretas para assinaturas escalares suportadas. Closures e captura de ambiente não estão implementadas.

## Limites atuais e suporte por alvo

O backend nativo implementado é Linux x86-64 / ELF64. Os laços `cada` e `pulso` e os builtins `gerar` e `em_paralelo` estão disponíveis no interpretador de referência, mas ainda não estão integrados ao backend ELF nativo; construções não suportadas são rejeitadas. Ele ainda não implementa ABI e geração nativa de agregados: estruturas, enumerações, tuplas, opcionais e resultados não podem ser passados ou retornados nativamente como agregados. A ABI nativa de inteiros de 128 bits também não está implementada.

O backend ELF x86-64 representa caracteres Unicode pelo ponto de código e valida o intervalo de escalares Unicode, excluindo a faixa de substitutos. Também possui operações SSE2 para decimal32 e decimal64 (aritmética básica, comparações e normalização binary32). Conversões explícitas entre inteiros de 8 a 64 bits e decimais são geradas nativamente: inteiro para decimal usa conversão SSE2, e decimal para inteiro verifica NaN e faixa antes de truncar em direção a zero. Conversões entre larguras inteiras também verificam sinal e faixa. A ABI de inteiros de 128 bits permanece indisponível no backend nativo. Recursos não suportados devem ser rejeitados pelo backend, nunca tratados como se estivessem implementados.

A inferência de retorno a partir do corpo da função e a inferência genérica qualificada por módulo permanecem indisponíveis. A política completa de cópia/movimentação, inicialização parcial e propriedade ainda pertence aos critérios de memória da Fase 5.

## Trabalho restante

- ABI e geração nativa de agregados, com passagem e retorno por função.
- ABI nativa de inteiros de 128 bits.
- Conversões explícitas entre inteiros e decimais com validação de faixa em cada alvo.
- Inferência de retorno quando permitida e inferência genérica qualificada por módulo.
- Regras completas de cópia, movimentação e inicialização parcial.
- Fixtures normativas integrais cobrindo gramática, tipos, semântica e execução.
- Backends adicionais e suas regras de layout/ABI.
