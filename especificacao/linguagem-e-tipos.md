# Especificação normativa da linguagem e do sistema de tipos Nebula

**Status:** fonte normativa canônica da Nebula. A gramática formal e as regras léxicas em `gramatica.md` e `tokens.md` são anexos técnicos subordinados a este contrato; `linguagem.md`, `tipos.md` e `operadores.md` são resumos de referência e não podem divergir dele. Em caso de conflito, este documento prevalece para a semântica e a ABI, e os anexos formalizam sua sintaxe. As regras observáveis são independentes de plataforma; a representação física e a ABI são específicas de cada alvo. A conformidade de cada recurso é registrada em `criterios-fase-5.md` e deve ser demonstrada por testes em todas as camadas aplicáveis. Uma regra normativa não significa, por si só, que todo backend já a implemente.

## 1. Princípios

1. A Nebula é estaticamente tipada. O tipo de cada expressão deve ser determinado em compilação, exceto quando uma operação explicitamente dinâmica fizer parte de uma biblioteca futura.
2. A inferência é local, determinística e não altera o tipo de uma variável depois da declaração.
3. Não existem conversões implícitas entre texto, caractere, lógico e tipos numéricos.
4. Uma conversão numérica que possa perder informação deve ser explícita ou rejeitada.
5. Um programa que viole uma regra estática não pode ser executado pelo interpretador nem gerar executável.
6. O interpretador e todos os backends devem observar a mesma semântica definida neste documento.
7. O compilador não pode aceitar silenciosamente uma construção que o backend selecionado não consegue representar.
8. A semântica observável da linguagem é independente do sistema operacional e da arquitetura hospedeira. Larguras inteiras, sinal, precisão decimal, codificação UTF-8 e regras de overflow são fixos pela linguagem. Um alvo pode impor limites de representação documentados, mas deve rejeitar recursos fora desses limites em vez de alterar seu significado. Endianness, offsets, padding, tamanho/alinhamento físicos e convenção de chamada pertencem à ABI versionada do alvo, nunca ao valor lógico de um tipo.

## 2. Tipos primitivos

Os tipos primitivos da linguagem são:

| Família | Tipos | Regra |
|---|---|---|
| Ausência de valor | `vazio` | Tipo de retorno sem valor; não pode ser variável, campo, elemento de lista, payload ou parâmetro |
| Lógico | `logico` | Valores `verdadeiro` e `falso` |
| Inteiros com sinal | `inteiro8`, `inteiro16`, `inteiro32`, `inteiro64`, `inteiro128` | Complemento de dois; faixas determinadas pela largura |
| Inteiro padrão | `inteiro` | Tipo inteiro com sinal de exatamente 64 bits em todos os alvos; não depende da largura da máquina hospedeira |
| Inteiros sem sinal | `inteiro_u8`, `inteiro_u16`, `inteiro_u32`, `inteiro_u64`, `inteiro_u128` | Faixa de zero a 2^N−1; não admite valor negativo |
| Ponto flutuante | `decimal32`, `decimal64` | IEEE 754 binary32 e binary64, respectivamente |
| Decimal padrão | `decimal` | Alias semântico de `decimal64` |
| Byte | `byte` | Unidade sem sinal de 8 bits; conversão para inteiro deve ser explícita |
| Caractere | `caractere` | Um escalar Unicode válido, não um byte |
| Texto | `texto` | Sequência UTF-8 válida; indexação por caractere deve respeitar escalares Unicode |

A faixa de um inteiro com sinal de N bits é −2^(N−1) até 2^(N−1)−1. A faixa sem sinal é 0 até 2^N−1. Literais inteiros são analisados sem truncamento; o compilador deve rejeitar valores que não caibam no tipo de destino.

### 2.1 Ponto flutuante

Operações de ponto flutuante seguem IEEE 754. NaN, infinito positivo, infinito negativo, zero positivo e zero negativo são valores distintos conforme IEEE 754. Comparações ordenadas com NaN são falsas; igualdade com NaN é falsa; desigualdade com NaN é verdadeira. Conversões de ponto flutuante para inteiro exigem conversão explícita e devem verificar finitude e faixa antes de converter.

### 2.2 Overflow e divisão

Operações inteiras verificadas não podem produzir wraparound silencioso. Overflow, divisão por zero e resto por zero devem produzir erro de execução definido pela linguagem. Uma operação wrapping, saturating ou unchecked só poderá existir com nome e contrato explícitos na biblioteca padrão. Divisão inteira trunca em direção a zero. O caso mínimo dividido por −1 é overflow.

## 3. Tipos compostos

### 3.1 Listas e vetores

A sintaxe de tipo de lista é `[T]`. Todos os elementos de uma lista possuem exatamente o mesmo tipo. Listas vazias exigem anotação ou contexto de tipo inequívoco. O acesso por índice é verificado em execução; índices negativos e índices fora do limite produzem erro definido, nunca acesso inválido à memória. `Vetor<T>` é uma grafia genérica equivalente a `[T]`; `Matriz<T>` equivale a `[[T]]`, isto é, uma lista de listas, sem garantia de que todas as linhas tenham o mesmo comprimento. Essas grafias preservam as regras de tipo e movimentação das listas.

### 3.2 Tuplas

Uma tupla é heterogênea e posicional. A sintaxe de valor é `(a, b)`; tupla unitária exige vírgula (`(a,)`); `()` representa a tupla vazia. A sintaxe de tipo correspondente é `(T1, T2)`, `(T,)` ou `()`. Parênteses em torno de um único tipo sem vírgula apenas agrupam esse tipo e não criam uma tupla. O tipo preserva ordem, quantidade e tipo de cada posição. Um acesso por índice literal válido pode ser resolvido estaticamente; acesso dinâmico deve preservar verificação de limites em execução.

### 3.3 Estruturas

Forma normativa:

```nebula
estrutura Pessoa: {
    nome: texto
    idade: inteiro

}

variavel pessoa: Pessoa = Pessoa { nome: "Ana", idade: 30 }
```

Cada campo tem nome único e tipo declarado. A construção exige todos os campos obrigatórios exatamente uma vez; campos desconhecidos, ausentes ou duplicados são erros. Acesso é feito por membro (`pessoa.nome`). A ordem declarada dos campos determina a ordem lógica, mas não autoriza assumir layout de memória sem consultar a regra de ABI do alvo. Estruturas são tipos nominais: dois tipos com campos iguais, mas nomes diferentes, não são intercambiáveis implicitamente.

### 3.4 Enumerações

Forma normativa:

```nebula
enumeracao Estado: {
    Inativo
    Ativo
    Falha(texto)

}
```

Uma enumeração é uma união discriminada. Cada variante tem discriminante único e pode carregar zero ou mais valores tipados. Na ausência de discriminantes explícitos, os valores são atribuídos em ordem de declaração, começando em zero; essa numeração é interna e não constitui conversão implícita para inteiro. A construção deve nomear uma variante existente e fornecer os argumentos exigidos. A inspeção de variantes deve ser exaustiva; qualquer padrão não coberto gera diagnóstico estático. O compilador não pode tratar uma enumeração como inteiro sem conversão explícita.

#### 3.4.1 Correspondência de padrões

A instrução `escolha` inspeciona uma enumeração e executa o corpo do primeiro caso correspondente:

```nebula
escolha resultado: {
    caso Resultado::Sucesso(valor): {
        exibir(valor)

    }
    caso Resultado::Falha(mensagem): {
        exibir(mensagem)

    }

}
```

Cada caso nomeia a enumeração e a variante. Os identificadores entre parênteses vinculam, na ordem declarada, os valores do payload ao escopo do corpo. Variantes sem payload não recebem parênteses nem vínculos. O padrão curinga `caso _` cobre todos os valores ainda não cobertos por casos anteriores; casos posteriores a um curinga são inalcançáveis e devem ser rejeitados na análise semântica. Sem curinga, todas as variantes declaradas precisam aparecer exatamente uma vez. Variantes inexistentes, repetidas, de outra enumeração, payloads com aridade incorreta e escolhas não exaustivas são erros estáticos. O interpretador executa a correspondência. O backend ELF x86-64 também gera construção e correspondência de enumerações, `Opcional` e `Resultado` quando o layout cabe na ABI própria: tag em um registrador e, no máximo, um payload escalar em outro. Payloads múltiplos ou maiores que um registrador continuam rejeitados.

### 3.5 Opcionais e resultados

- `Opcional<T>` representa presença ou ausência de um valor: `Opcional::Algum(valor)` e `Opcional::Nenhum`.
- `Resultado<T, E>` representa sucesso ou falha: `Resultado::Sucesso(valor)` e `Resultado::Falha(erro)`.
- `Nenhum` e variantes sem payload não recebem parênteses; variantes com payload exigem exatamente a quantidade e os tipos declarados.
- Os parâmetros genéricos devem ser preservados na construção, inferência, retorno, armazenamento e chamada.
- A inferência de parâmetros genéricos é transacional: todos os componentes de uma unificação composta precisam concordar; se qualquer componente falhar ou permanecer sem tipo concreto, nenhuma vinculação parcial pode ser aproveitada. Lacunas de inferência não são convertidas silenciosamente em tipos concretos.
- A extração de um valor exige correspondência de padrão ou operação explícita que trate o caso ausente/falho.
- A propagação antecipada de falha só pode ser introduzida por sintaxe documentada e deve preservar o tipo de erro declarado.
- Não existe conversão implícita entre `Opcional<T>` e `Resultado<T,E>`.

### 3.6 Aliases de tipo

A declaração `tipo Nome = Tipo` cria um alias transparente, não um novo tipo nominal. O alias pode ser usado em qualquer posição que aceite o tipo de destino, inclusive em tipos genéricos aninhados, campos, parâmetros e retornos. Aliases são resolvidos antes da análise dos corpos e preservam a identidade semântica do tipo de destino. Referências desconhecidas e ciclos diretos ou indiretos são erros estáticos. Um alias não pode alterar layout, alinhamento, representação, igualdade, capacidade de cópia ou ABI do tipo que nomeia.

### 3.7 Intervalos

O operador `inicio..fim` cria um intervalo numérico fechado, incluindo os dois limites. O tipo do intervalo é inferido a partir do tipo dos limites, que devem ser numericamente compatíveis e possuir o mesmo tipo após a verificação semântica. Texto, caracteres, valores lógicos, agregados e tipos de função não podem ser limites.

A forma de tipo é `Intervalo<T>`, em que `T` é o tipo numérico dos limites; ela pode ser usada em anotações e assinaturas, além da inferência a partir de expressões. A construção verifica a compatibilidade e a faixa dos dois valores; não pode reinterpretar um intervalo inteiro como decimal nem aceitar limites fora da faixa de `T`. Para `decimal32`, cada limite é arredondado para IEEE-754 binary32. Valores não finitos não podem ser usados por iteração que exija progressão finita. Para tipos inteiros sem sinal, a representação deve preservar todo o domínio de `T`, inclusive `inteiro_u128`, sem converter os limites para uma representação assinada que perca valores.

A instrução `cada` percorre os valores do intervalo em passos unitários, na direção do limite final, incluindo-o quando alcançável. Intervalos decimais usam passo de uma unidade da representação decimal e exigem limites finitos. A implementação não deve prometer precisão decimal exata além da representação IEEE-754 adotada.

## 4. Funções e genéricos

Uma função possui nome, parâmetros tipados, retorno declarado ou inferido quando permitido, e corpo. Tipos de função são definidos pela sequência ordenada dos tipos dos parâmetros e pelo tipo de retorno. A aridade e cada tipo de argumento são verificados antes da execução.

Parâmetros genéricos são variáveis de tipo com escopo limitado à declaração. A inferência usa unificação estrutural determinística, inclusive dentro de listas, intervalos, opcionais, resultados, tuplas, assinaturas de função e referências. Ao inferir uma referência, o compilador preserva e exige a mesma mutabilidade (`&T` compartilhada ou `&mutavel T` exclusiva); não há coerção entre essas duas formas. Um mesmo parâmetro genérico deve receber o mesmo tipo em todas as ocorrências da instância. Quando não houver informação suficiente, o compilador deve solicitar anotação em vez de escolher um tipo arbitrário. A inferência de uma chamada é uma transação única que abrange todos os argumentos e, quando disponível, o tipo esperado do retorno: se qualquer unificação falhar, nenhuma vinculação parcial de parâmetro genérico pode ser usada para formar a instância.

A monomorfização gera uma instância concreta para cada conjunto distinto de argumentos de tipo utilizado. A chave de instância inclui a identidade da função e todos os tipos concretos, incluindo tipos aninhados. Recursão genérica deve reutilizar a instância já registrada e não pode gerar expansão infinita por reentrada. Erros de inferência devem apontar a declaração e as ocorrências incompatíveis.

### 4.1 Tipos de função

A forma de tipo é `funcao(T1, T2, ...) -> Retorno`; os parâmetros podem conter outros tipos de função e tipos genéricos. O retorno pode ser `vazio`. A assinatura preserva a ordem e a identidade dos tipos. O layout de um valor de função é um ponteiro de código de 8 bytes, alinhado a 8 bytes, no ABI inicial x86-64. Valores de função sem captura de ambiente são representados como ponteiros de código. A IR e o interpretador de referência suportam armazenamento, retorno e chamadas indiretas desses valores; o backend ELF x86-64 também gera referências e chamadas indiretas para assinaturas escalares suportadas. Closures e captura de ambiente não fazem parte do suporte atual. Ponteiros de função podem ser armazenados em estruturas/tuplas de até 16 bytes quando ocupam os offsets de registrador suportados pela ABI própria; agregados maiores ou com layouts não classificados continuam rejeitados.

## 5. Conversões e promoção

1. Não há coerção implícita entre tipos inteiros de larguras diferentes em atribuições, parâmetros ou retornos.
2. Literais numéricos sem contexto recebem o tipo padrão da família; o contexto pode selecionar um tipo de largura explícita apenas quando o valor couber exatamente.
3. A promoção aritmética é definida pela tabela abaixo e independe da largura da máquina hospedeira. A regra é aplicada a cada par de operandos antes da operação e também às comparações numéricas.
4. Dois inteiros assinados promovem para o tipo assinado de maior largura; dois inteiros sem sinal promovem para o tipo sem sinal de maior largura. `byte` participa como inteiro sem sinal de 8 bits. `inteiro` participa como inteiro assinado de 64 bits.
5. Para um par misto assinado/sem sinal, o resultado é o menor tipo assinado disponível que comporte integralmente os dois domínios: sua largura precisa ser maior que a largura do operando sem sinal e não menor que a largura do operando assinado. Assim, `inteiro8 + inteiro_u8` resulta em `inteiro16`, e `inteiro64 + inteiro_u64` resulta em `inteiro128`. Se não existir tipo assinado capaz de representar ambos os domínios, a expressão é rejeitada estaticamente; em particular, `inteiro128` combinado com `inteiro_u128` não possui promoção válida.
6. `decimal64` domina qualquer operação numérica que o contenha. Dois operandos `decimal32` resultam em `decimal32`. Um `decimal32` combinado com inteiro de até 16 bits resulta em `decimal32`; combinado com inteiro de 32 bits ou mais resulta em `decimal64`. A promoção para decimal segue IEEE-754 e pode arredondar valores inteiros que excedam a precisão exata do formato.
7. Conversões explícitas verificam sinal, faixa e finitude. Se o valor não couber, a conversão falha de forma definida. As conversões numéricas explícitas são chamadas pelo nome do tipo (`inteiro`, `inteiro8`, `inteiro16`, `inteiro32`, `inteiro64`, `inteiro128`, `inteiro_u8`, `inteiro_u16`, `inteiro_u32`, `inteiro_u64`, `inteiro_u128`, `byte`, `decimal`, `decimal32` e `decimal64`). Conversões de decimal para inteiro truncam em direção a zero e verificam a faixa antes de produzir o valor. Conversões de inteiro para decimal seguem o arredondamento IEEE-754 do destino.
8. Conversões entre `decimal32` e `decimal64` seguem as regras IEEE 754 e devem ser explícitas quando houver perda potencial. Operações cujo tipo resultante é `decimal32` são arredondadas para IEEE-754 binary32 após cada operação, não apenas ao armazenar o resultado final.
9. A promoção nunca altera o tipo declarado de uma variável, parâmetro, campo, retorno ou elemento de lista; ela só determina o tipo temporário da expressão. Atribuições, argumentos e retornos continuam exigindo compatibilidade exata, salvo literal contextual cujo valor caiba exatamente no destino.

## 6. Igualdade, cópia e movimentação

Igualdade só é válida para tipos que implementem igualdade. A igualdade de tuplas e estruturas é componente a componente quando todos os componentes implementam igualdade. A igualdade de enumerações exige mesma variante e igualdade de todos os payloads. Listas comparam comprimento e elementos na ordem. A implementação atual valida essa capacidade para tipos primitivos, listas, tuplas, estruturas, enumerações, opcionais, resultados e intervalos; tipos de função, `vazio` e parâmetros genéricos sem restrição não implementam igualdade.

Tipos escalares primitivos são copiáveis, incluindo `inteiro8`, todas as larguras inteiras assinadas/sem sinal, `byte`, `logico`, decimais, `caractere` e valores de função sem captura. Tuplas, estruturas, enumerações, `Opcional`, `Resultado` e `Intervalo` derivam a capacidade de cópia recursivamente: são copiáveis somente quando todos os seus componentes/payloads são copiáveis. Listas e texto permanecem move-only nesta versão. Ciclos de representação não podem produzir uma derivação de cópia. A derivação não pode ser inferida apenas pelo tamanho físico do valor. A chamada `copiar(valor)` solicita explicitamente uma cópia de um valor que já derivou a capacidade de cópia; ela rejeita tipos move-only. Implementações customizadas/opt-in de cópia para tipos definidos pelo usuário ficam explicitamente fora do escopo estável 1.0: sem empréstimos e referências com contrato completo, um hook de cópia poderia duplicar ownership ou criar aliasing inválido. Tipos definidos pelo usuário seguem exclusivamente a derivação recursiva descrita acima. O compilador deve rejeitar uso após movimentação e não pode inferir cópia apenas porque o valor cabe em um registrador.

Nas estruturas, mover um campo não copiável move somente o caminho daquele campo. Os campos irmãos não movidos continuam acessíveis. A leitura do campo movido e o uso da estrutura inteira enquanto houver campos movidos são erros. Atribuir novamente ao caminho movido o reinicializa; atribuir uma estrutura inteira reinicializa todos os caminhos. Em fluxos condicionais, a junção conserva a união dos caminhos movidos em qualquer ramo. A regra se aplica também a caminhos aninhados de campos nomeados. A implementação atual cobre campos nomeados de estruturas e posições de tuplas com índice literal: os demais campos/posições permanecem acessíveis, a posição movida pode ser reatribuída e a junção entre ramos conserva a união dos caminhos movidos. A extração de elemento não copiável por índice literal de lista move somente a posição selecionada; os demais elementos permanecem acessíveis e a posição pode ser reinicializada. Índices dinâmicos ainda movem conservadoramente a lista inteira.

Qualquer tipo desconhecido dentro de um tipo composto (lista, intervalo, opcional, resultado, tupla ou assinatura de função) invalida a declaração que o utiliza. A validação percorre a árvore completa do tipo e não pode aceitar um tipo parcialmente resolvido.

A instrução `escolha` consome enumerações, `Opcional` e `Resultado` não copiáveis; os vínculos de payload transferem os valores para o escopo do caso, e o valor algébrico original não pode ser reutilizado após a correspondência. Movimentação parcial de payloads sem consumir o agregado e inicialização parcial geral ficam explicitamente fora do escopo estável 1.0. O contrato escolhe o consumo integral para manter a validade da variante e evitar estados parcialmente inicializados até que a linguagem tenha empréstimos/padrões por referência completos.

## 7. Layout e ABI

O layout de memória é definido por alvo e versão de ABI. Cada tipo deve expor tamanho, alinhamento e representação. Campos de estruturas seguem alinhamento natural do tipo, com padding determinístico; o tamanho final é múltiplo do alinhamento da estrutura. Tuplas seguem regra equivalente à de estruturas anônimas. Enumerações usam discriminante e payload com representação documentada, incluindo tamanho e alinhamento. O layout não pode depender do compilador Rust que hospeda o compilador Nebula.

#### Layout escalar inicial — Linux x86-64

| Tipo | Tamanho | Alinhamento | Representação |
|---|---:|---:|---|
| `vazio` | 0 bytes | 1 byte | ausência de valor |
| `logico`, `byte`, `inteiro8`, `inteiro_u8` | 1 byte | 1 byte | escalar |
| `inteiro16`, `inteiro_u16` | 2 bytes | 2 bytes | escalar |
| `inteiro32`, `inteiro_u32`, `decimal32` | 4 bytes | 4 bytes | escalar IEEE-754 ou inteiro |
| `inteiro`, `inteiro64`, `inteiro_u64`, `decimal`, `decimal64`, `caractere`, ponteiro de função | 8 bytes | 8 bytes | escalar/ponteiro |
| `inteiro128`, `inteiro_u128` | 16 bytes | 16 bytes | dois words de 64 bits |
| `texto` | 16 bytes | 8 bytes | ponteiro + comprimento em bytes |

Aliases preservam exatamente o layout do tipo de destino. `Lista<T>` e `Intervalo<T>` não possuem ABI nativa neste backend e são rejeitados em assinaturas/valores que exijam representação ELF. `Opcional<T>` usa discriminante de 1 byte, payload no próximo offset alinhado ao alinhamento de `T` e tamanho final arredondado; `Resultado<T,E>` usa discriminante de 1 byte e reserva o maior payload, alinhado pelo maior alinhamento entre `T` e `E`. Enumerações usam o layout calculado da declaração, com discriminante e payload; a ABI ELF atual restringe o total a 16 bytes e o payload a um escalar de até 8 bytes.

A ABI nativa Linux x86-64 da Nebula é própria e não promete compatibilidade direta com ABI C. Inteiros assinados e sem sinal de 128 bits usam dois registradores inteiros consecutivos: parte baixa no primeiro registrador de argumento e parte alta no seguinte; retornos usam `rdx:rax`. O backend implementa passagem e retorno de `inteiro128`/`inteiro_u128` em dois registradores, soma, subtração, comparação, negação assinada, multiplicação com verificação de overflow e divisão/resto por algoritmo restaurador de 128 iterações. Divisão por zero e overflow do quociente são erros de execução. Conversões entre inteiros de 128 bits e larguras de até 64 bits verificam extensão, sinal e faixa. Conversões nativas entre inteiros de 128 bits e `decimal32`/`decimal64` usam SSE2 e decomposição em metades de 64 bits; conversões de decimal para inteiro truncam em direção a zero e rejeitam valores não finitos ou fora da faixa. Testes Linux executam ELF gerados para operações assinadas/sem sinal e conversões, incluindo operandos acima de 64 bits. A ABI continua sendo a ABI própria Nebula, sem alegar compatibilidade com ABI C. Estruturas e tuplas de até 16 bytes podem usar um ou dois registradores quando seus campos ocupam os offsets 0 e 8 (ou um único campo de 16 bytes). Estruturas e tuplas maiores que 16 bytes são armazenadas em memória: argumentos são passados indiretamente por endereço e retornos usam um destino oculto fornecido pelo chamador. A ABI nativa x86-64 executa esses caminhos para estruturas e tuplas locais, parâmetros e retornos; testes ELF de integração verificam estruturas e tuplas de 24 bytes ponta a ponta. Agregados grandes em variáveis globais e retorno agregado da função de entrada `principal` continuam rejeitados explicitamente. Enumerações usam discriminante e, no contrato nativo atual, aceitam no máximo um payload escalar de até um registrador; `Opcional` e `Resultado` seguem o mesmo limite de payload. Variantes com payloads múltiplos, payload agregado maior que um registrador ou enumerações maiores que 16 bytes são rejeitadas antes da geração do ELF. O tag nativo é determinístico e interno à ABI Nebula. Valores escalares `decimal32` e `decimal64` são transportados em um registrador inteiro do ABI próprio; `decimal32` chega ao registrador já arredondado para binary32 e representado no formato escalar interno usado pelo backend. Esses valores também podem ocupar campos de estruturas/tuplas nos offsets de registrador suportados e payloads escalares de enumerações, `Opcional` e `Resultado`. A classificação em registradores não declara compatibilidade com ABI C nem substitui o layout físico do tipo. Acesso e atribuição direta a campos, construção, passagem, retorno e correspondência de padrões só são aceitos nos formatos descritos. Um backend sem suporte a uma representação ou ABI deve rejeitar o programa com diagnóstico, nunca reinterpretar agregado como inteiro de 64 bits.

### 7.1 Representação inicial de tipos de função

No ABI inicial Linux x86-64, o descritor físico de um tipo de função ocupa um ponteiro de código de 8 bytes, alinhado a 8 bytes. A assinatura mantém a ordem e os tipos dos parâmetros e o tipo de retorno. Esse descritor é compatível com valores de função sem captura de ambiente. A IR e o interpretador suportam esses valores e chamadas indiretas; o backend ELF x86-64 também gera chamadas indiretas para assinaturas escalares suportadas. Closures e captura de ambiente permanecem indisponíveis. O backend aceita assinaturas agregadas apenas nos limites de layout descritos na seção ABI: estruturas/tuplas de até 16 bytes e tipos algébricos com no máximo um payload escalar. Closures e agregados fora desses limites continuam rejeitados.

## 8. Diagnósticos

Os diagnósticos devem ter código estável, severidade, mensagem, intervalo de fonte e, quando possível, indicação da declaração relacionada. Devem distinguir pelo menos: nome/tipo desconhecido, tipos incompatíveis, aridade incorreta, inferência ambígua, literal fora da faixa, conversão inválida, campo ausente/duplicado/desconhecido, variante inexistente, padrão não exaustivo, acesso inválido e recurso não suportado pelo backend.

## 9. Precedência e avaliação

A tabela abaixo é normativa; níveis maiores vinculam primeiro. Operadores no mesmo nível são associativos à esquerda, exceto quando indicado. A implementação do Parser usa precedência Pratt equivalente.

| Nível | Operadores/construções | Associatividade |
|---:|---|---|
| 14 | chamada, indexação, acesso a membro `.` e caminho `::` | encadeamento à esquerda |
| 10 | potência `**` | à direita |
| 9 | prefixos `+`, `-`, `!`, `não` | prefixa; potência vincula mais forte |
| 8 | multiplicação `*`, divisão `/`, resto `%` | à esquerda |
| 7 | adição `+`, subtração `-` | à esquerda |
| 6 | intervalo `..` | não encadeável |
| 5 | comparações `<`, `<=`, `>`, `>=` | não encadeáveis |
| 4 | igualdade `==`, diferença `!=` | à esquerda |
| 3 | conjunção `e` | à esquerda, curto-circuito |
| 2 | disjunção `ou` | à esquerda, curto-circuito |
| 1 | atribuição `=`, `<-` | à direita |

A avaliação de chamadas e operadores binários é da esquerda para a direita, salvo quando a tabela declarar associatividade à direita. Curto-circuito lógico não avalia o operando direito quando o resultado já é determinado pelo esquerdo. A ordem de avaliação não pode ser alterada por otimizações quando houver efeitos observáveis. Comparações relacionais e operadores de intervalo encadeados são erros sintáticos; agrupamento explícito com parênteses é permitido.

## 10. Propriedade, referências, empréstimos e segurança

Esta seção define o contrato de propriedade da linguagem. A presença de uma regra normativa não significa que todas as camadas já a implementem; a conformidade deve ser comprovada conforme a seção de conformidade.

### 10.1 Propriedade e movimentação

1. Cada valor não copiável possui exatamente um proprietário. A atribuição de um valor não copiável transfere a propriedade; o identificador de origem fica indisponível até ser reinicializado integralmente.
2. Tipos escalares e agregados cujos componentes sejam copiáveis podem ser copiados por valor. Tipos que possuam recursos exclusivos, referências exclusivas ou componentes não copiáveis são movidos.
3. Movimentação parcial só é permitida para campos de estruturas e posições de tuplas quando o verificador consegue provar a projeção estática. O agregado não pode ser lido ou movido enquanto uma projeção obrigatória estiver não inicializada.
4. A escrita pode reinicializar uma projeção movida. A leitura do agregado inteiro só volta a ser válida quando todas as projeções obrigatórias estiverem inicializadas.
5. Uma movimentação, escrita ou destruição não pode ocorrer enquanto existir empréstimo ativo sobre lugar sobreposto.

### 10.2 Tipos e expressões de referência

A sintaxe normativa é:

| Forma | Significado |
|---|---|
| `&T` | Tipo de referência compartilhada para `T` |
| `&mutavel T` | Tipo de referência exclusiva e mutável para `T` |
| `&valor` | Cria empréstimo compartilhado |
| `&mutavel valor` | Cria empréstimo exclusivo |
| `*referencia` | Acessa o valor referenciado |

Uma referência não possui o valor referenciado. Seu tempo de vida não pode exceder o do proprietário. Referências compartilhadas permitem somente leitura; referências exclusivas permitem leitura e escrita e exigem ausência de qualquer outro empréstimo sobreposto. É permitido manter vários empréstimos compartilhados simultâneos. Não é permitido manter dois empréstimos exclusivos, nem combinar empréstimo exclusivo com compartilhado sobre lugares sobrepostos.

A palavra `mutavel` qualifica o empréstimo, não altera a mutabilidade declarada do proprietário. A criação de referência a uma variável imutável é sempre compartilhada. A criação de referência exclusiva exige que o lugar de origem seja mutável.

### 10.3 Vidas úteis e fluxo de controle

1. A vida útil de uma referência começa na criação do empréstimo e termina após seu último uso comprovável no fluxo de controle. O compilador deve encerrar empréstimos após o último uso, e não apenas no fim lexical do bloco.
2. O verificador acompanha estado de propriedade e empréstimos em cada caminho de controle. Na junção de caminhos, uma variável só é considerada inicializada se estiver inicializada em todos os caminhos alcançáveis; um empréstimo é considerado ativo se puder permanecer ativo em qualquer caminho.
3. Uma referência não pode escapar do escopo do proprietário, inclusive por retorno, armazenamento em estrutura, captura ou chamada indireta.
4. Laços exigem análise de estado estável: uma movimentação ou empréstimo que conflite com a próxima iteração deve ser rejeitado, salvo se todos os caminhos reinicializarem/encerrarem o estado antes da iteração seguinte.
5. A verificação estática deve rejeitar referência pendente, uso após movimentação, movimentação/escrita durante empréstimo incompatível, aliasing mutável e uso de valor parcialmente inicializado.

### 10.4 Destruição determinística

1. Valores proprietários são destruídos automaticamente ao sair do escopo em que foram inicializados, em ordem inversa à inicialização (LIFO).
2. Empréstimos terminam antes da destruição do proprietário. Uma referência não pode permanecer viva após o encerramento do escopo do proprietário.
3. A destruição deve ocorrer em saída normal, retorno, propagação de erro e encerramento antecipado de laço. A IR deve representar explicitamente as ações de destruição para que interpretador e backends observem a mesma ordem.
4. Se a destruição de um recurso falhar, as demais destruições pendentes ainda devem ser executadas; a primeira falha é preservada como erro de encerramento. Durante propagação de erro, a falha de destruição não pode apagar silenciosamente a causa original.
5. A inicialização parcial destrói somente os campos efetivamente inicializados, em ordem inversa à inicialização. Um campo movido não é destruído novamente pelo agregado.

### 10.5 Operações inseguras

Código seguro não pode executar operações que dependam de validade de ponteiro não comprovada, alinhamento, inicialização, aliasing ou sincronização não garantidos pelo sistema de tipos.

As operações abaixo exigem contexto inseguro explícito:

- desreferenciar ponteiro bruto ou converter inteiro em endereço;
- criar referência a partir de endereço bruto;
- acessar memória fora de uma alocação ou sem prova estática de limites;
- reinterpretar representação, layout, alinhamento ou tipo de memória;
- chamar função externa cuja assinatura/contrato não tenha sido verificado pelo compilador;
- executar instrução de montagem ou operação privilegiada;
- liberar manualmente uma alocação enquanto houver referência ou empréstimo ativo;
- declarar ou acessar região de memória mapeada por dispositivo.

Os blocos contextuais `critico`, `privilegio` e `montagem` são os únicos contextos que podem autorizar essas operações, conforme suas permissões: `critico` autoriza operações de memória explicitamente listadas; `privilegio` exige alvo/suporte privilegiado e autorização do sistema; `montagem` exige alvo de arquitetura compatível. Um bloco contextual não transforma automaticamente uma operação inválida em válida.

Cada operação insegura deve ser validada contra suas obrigações específicas: origem e validade do endereço, tamanho e alinhamento, inicialização, exclusividade/aliasing, limites da alocação, validade da ABI e requisitos do alvo. A ausência de prova ou de obrigação satisfeita é erro estático. O compilador não pode inferir segurança apenas pela presença de um bloco contextual.

### 10.6 Alocação e coleta

A alocação dinâmica é explícita e pertence a um alocador documentado. Toda alocação possui proprietário e deve ser liberada automaticamente pelo encerramento do proprietário ou por liberação explícita validada. A liberação manual não pode ocorrer enquanto referências estiverem vivas. A Nebula não exige coletor global de lixo; custos de alocação, liberação, contagem ou sincronização devem ser documentados por API e alvo.

## 11. Conformidade da implementação


Esta especificação é normativa; a implementação pode estar em estágio diferente. Um recurso só é considerado implementado quando percorre as camadas que lhe são aplicáveis: Lexer/Parser, AST, análise semântica, IR, interpretador, backend e testes de conformidade. O interpretador de referência deve preservar a semântica da linguagem. Cada backend deve implementar a representação declarada para seu alvo ou emitir diagnóstico explícito antes de gerar um artefato. O estado verificável, os limites por alvo e as evidências de CI permanecem registrados em `criterios-fase-5.md` e `tipos.md`.
