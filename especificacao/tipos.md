# Sistema de tipos — Fase 3

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

## Tipos compostos e decisões futuras

Vetores, tuplas, estruturas, enumerações, opcionais, resultados, referências, ponteiros, funções genéricas e tipos algébricos ainda não fazem parte do subconjunto semântico executável.

Continuam abertas para alvos futuros e para estabilização geral da linguagem:

- Largura de inteiro por alvo além do Linux x86-64 inicial.
- Regras de overflow específicas de outros alvos e modos.
- Semântica completa de ponto flutuante, NaN e conversões.
- Layout e alinhamento de tipos na ABI.
- Inferência genérica e tipos algébricos.
