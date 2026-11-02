# Sistema de tipos — Nebula

A especificação normativa única está em [linguagem-e-tipos.md](linguagem-e-tipos.md). Este arquivo registra a implementação atual e suas limitações verificadas; em caso de divergência, a especificação define o contrato-alvo e os critérios da Fase 5 registram o que ainda não está conforme.


## Modelo implementado

- Tipagem estática por padrão.
- Inferência local e determinística em declarações sem anotação.
- Tipos explícitos nas interfaces de funções e variáveis anotadas.
- Não há coerção implícita geral entre texto, números e lógicos.
- Tipos numéricos de larguras diferentes são distintos; variáveis e expressões não literais exigem tipos compatíveis. Literais inteiros podem receber tipos inteiros anotados quando cabem na faixa; literais decimais podem receber tipos decimais anotados. Não há coerção contextual de literal inteiro para tipo decimal. Operações aritméticas entre operandos numéricos usam promoção definida pela análise.

## Tipos reconhecidos pela análise semântica

- logico
- inteiro (tipo inteiro padrão; no alvo inicial Linux x86-64, signed de 64 bits)
- inteiro8, inteiro16, inteiro32, inteiro64, inteiro128
- decimal e decimal64
- decimal32
- byte
- caractere
- texto
- vazio
- listas homogêneas anotadas como [tipo] ou inferidas a partir dos elementos.
- construtores genéricos em assinaturas: `Opcional<T>`, `Resultado<T, E>` e `Tupla<T1, T2, ...>`, com argumentos recursivos;
- parâmetros de tipo em funções genéricas, com inferência por unificação estrutural e monomorfização de instâncias concretas na IR;
- tuplas heterogêneas em expressões `(valor1, valor2, ...)`, incluindo tuplas unitárias `(valor,)` e vazias `()`; acesso por índice literal é verificado estaticamente e executado pelo interpretador.

Aliases de tipo são resolvidos antes da análise dos corpos e ciclos ou destinos desconhecidos são rejeitados.

## Regras estáticas iniciais

- Condições de quando, enquanto e para devem ser lógicas.
- Aritmética exige operandos numéricos; texto + texto concatena.
- Comparações relacionais exigem operandos numéricos.
- Igualdade exige tipos compatíveis.
- Listas devem ser homogêneas; uma lista vazia exige tipo esperado explícito para permitir inferência.
- Índices de listas aceitam qualquer tipo inteiro suportado (`inteiro`, larguras explícitas e `byte`); tipos decimais são rejeitados.
- Chamadas validam existência da função, aridade, tipos dos argumentos e tipo de retorno.
- Literais inteiros e decimais são verificados contra os limites do tipo de destino; valores fora da faixa são rejeitados.
- Funções com retorno não vazio devem devolver um valor em todos os caminhos condicionais reconhecidos pelo verificador.

## Limites atuais dos tipos compostos

`Tupla<T...>` possui valores heterogêneos no interpretador e lowering próprio na IR; o backend ELF ainda não implementa layout/ABI de agregados. `Opcional<T>` e `Resultado<T,E>` continuam limitados à resolução de tipos em assinaturas: seus construtores, propagação e padrões de correspondência ainda não estão implementados. A IR infere o retorno de funções comuns e genéricas, inclusive chamadas genéricas aninhadas, chamadas genéricas usadas em inicializadores locais e tipos de índices literais de tuplas; esses cenários têm testes de execução. A inferência ainda não cobre valores de função de primeira classe, chamadas genéricas qualificadas por módulo nem inferência de retorno a partir do corpo da função. Estruturas, enumerações e tipos definidos pelo usuário ainda não estão implementados. Inteiros sem sinal definidos na especificação normativa ainda não estão reconhecidos pela análise semântica. O sistema de tipos, portanto, permanece parcial.

Continuam abertas para alvos futuros e para estabilização geral da linguagem:

- Largura de inteiro por alvo além do Linux x86-64 inicial.
- Regras de overflow específicas de outros alvos e modos.
- Semântica completa de ponto flutuante, NaN e conversões.
- Layout e alinhamento de tipos na ABI.
- Inferência e instanciação de funções genéricas, construtores e padrões de tipos algébricos.
- Representação executável e layout ABI de opcionais, resultados e tuplas.
