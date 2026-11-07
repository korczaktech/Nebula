## Referência do sistema de tipos — Nebula

A única fonte normativa é `linguagem-e-tipos.md`. Este documento resume o comportamento implementado e separa suporte do compilador de suporte de cada backend. Em caso de divergência, prevalece o contrato normativo.


A definição normativa única está em [linguagem-e-tipos.md](linguagem-e-tipos.md). Este documento resume o comportamento implementado e separa suporte do compilador de suporte de cada backend. A presença de um tipo na linguagem não significa que toda representação seja aceita por todos os alvos.

## 1. Modelo de tipos

- Tipagem estática: toda expressão recebe um tipo antes da execução.
- Inferência local e determinística; o tipo de uma variável não muda depois da declaração.
- Tipos de interfaces (parâmetros, retornos e variáveis anotadas) são verificados estaticamente.
- Não há coerção implícita geral entre texto, caractere, lógico e números.
- Tipos inteiros de larguras diferentes permanecem distintos em atribuições, parâmetros e retornos.
- Literais numéricos recebem o tipo contextual somente quando o valor cabe exatamente no destino.
- Inferência genérica usa unificação estrutural; conflitos ou parâmetros sem tipo concreto impedem a criação da instância.
- Aliases são resolvidos antes da análise dos corpos; ciclos e destinos desconhecidos são rejeitados.

## 2. Tipos primitivos

| Família | Grafias | Semântica |
|---|---|---|
| Sem valor | `vazio` | Retorno sem valor; não é tipo de variável comum |
| Lógico | `logico` | `verdadeiro` ou `falso` |
| Inteiros assinados | `inteiro8`, `inteiro16`, `inteiro32`, `inteiro64`, `inteiro128` | Complemento de dois, com faixa determinada pela largura |
| Inteiro padrão | `inteiro` | Alias semântico de `inteiro64` |
| Inteiros sem sinal | `inteiro_u8`, `inteiro_u16`, `inteiro_u32`, `inteiro_u64`, `inteiro_u128` | Faixa de zero a 2^N−1 |
| Ponto flutuante | `decimal32`, `decimal64` | IEEE-754 binary32 e binary64 |
| Decimal padrão | `decimal` | Alias semântico de `decimal64` |
| Byte | `byte` | Inteiro sem sinal de 8 bits, distinto dos demais inteiros para conversão |
| Caractere | `caractere` | Um escalar Unicode válido |
| Texto | `texto` | Sequência UTF-8 válida |

Inteiros usam operações verificadas: overflow, divisão por zero, resto por zero e divisão do menor valor assinado por −1 são erros definidos, não wraparound silencioso. Conversões explícitas verificam sinal e faixa. Inteiros sem sinal preservam todo o domínio até `inteiro_u128`.

Operações `decimal32` são normalizadas para binary32 depois de cada operação. `decimal64` segue binary64. Conversões decimais para inteiros verificam finitude e faixa e truncam em direção a zero.

Texto é UTF-8; comprimento e indexação por caractere contam escalares Unicode, não bytes. Caractere rejeita valores fora do intervalo de escalares Unicode, inclusive substitutos. A concatenação de texto funciona no interpretador de referência. No backend ELF, literais e operações textuais implementadas são suportados, mas concatenação dinâmica permanece explicitamente rejeitada enquanto não houver alocador nativo integrado.

## 3. Tipos compostos e algébricos

- **Lista `[T]`:** sequência homogênea. `Vetor<T>` é alias de `[T]`; `Matriz<T>` é alias de `[[T]]`. Lista vazia exige tipo contextual.
- **Tupla `(T1, T2, ...)`:** agregado posicional heterogêneo. `()` é a tupla vazia, `(T,)` é tupla unitária e `(T1, T2)` é composta. Parênteses sem vírgula apenas agrupam um tipo. `Tupla<T1, T2, ...>` também é aceita para tuplas não vazias.
- **Estrutura:** tipo nominal definido pelo usuário, com campos nomeados. Campos duplicados, ausentes, desconhecidos ou incompatíveis são rejeitados. A construção usa `Nome { campo: valor }`.
- **Alias:** `tipo Nome = Tipo` cria um nome transparente para o destino; ciclos e destinos desconhecidos são rejeitados, sem alterar identidade, layout ou ABI.
- **Enumeração:** união discriminada nominal, com variantes e payload posicional de zero ou mais valores. A correspondência é exaustiva; variantes inválidas, repetidas, payloads com aridade errada e escolhas incompletas são rejeitados.
- **Opcional:** `Opcional<T>`, com `Opcional::Algum(valor)` e `Opcional::Nenhum`.
- **Resultado:** `Resultado<T, E>`, com `Resultado::Sucesso(valor)` e `Resultado::Falha(erro)`.
- **Intervalo:** `Intervalo<T>`, numérico e fechado; limites precisam pertencer ao mesmo tipo numérico após normalização.
- **Tipo de função:** `funcao(T1, T2): Retorno`; valores de função sem captura podem ser armazenados e chamados indiretamente.

Estruturas, enumerações, opcionais, resultados, listas e tuplas percorrem a sintaxe, AST, análise semântica e IR. O interpretador de referência constrói e executa esses valores e suas operações suportadas, incluindo correspondência de padrões. A IR mantém tipos e layouts compostos; o runtime Rust também valida descrições de tipos e calcula tamanho, alinhamento, padding e tags.

A capacidade de cópia de tuplas, estruturas, enumerações, opcionais, resultados e intervalos é derivada recursivamente dos componentes. Texto e listas são move-only. A análise semântica acompanha uso após movimentação e movimentação parcial de campos de estruturas, posições de tuplas e índices literais de listas, dentro dos limites descritos na especificação normativa.

## 4. Regras de expressões e operações

- Condições de `quando`, `enquanto`, `durante`, `repetir` e construções condicionais exigem `logico`.
- Aritmética exige operandos numéricos compatíveis; `texto + texto` concatena no interpretador.
- Comparações relacionais exigem operandos numéricos. Igualdade estrutural compara componentes quando todos implementam igualdade.
- Listas devem ser homogêneas; índices são inteiros e têm limites verificados em execução.
- Índices de tuplas devem ser literais inteiros válidos, conhecidos em compilação.
- Chamadas diretas e indiretas verificam aridade, tipos dos argumentos e retorno.
- Conversões numéricas são explícitas e verificam faixa.
- Um recurso não implementado no backend selecionado deve produzir diagnóstico; não pode ser substituído por uma representação aproximada.

## 5. Suporte por camada e alvo

### Frontend, semântica, IR e interpretador

O Lexer/Parser/AST reconhece as declarações e expressões previstas na gramática normativa. A análise semântica valida nomes, tipos, campos, variantes, payloads, intervalos, igualdade, inferência genérica e regras de movimentação cobertas pelos testes. A IR normaliza tipos e operações e rejeita valores incompatíveis. O interpretador Rust é a referência executável para recursos ainda não disponíveis no backend nativo.

### Backend nativo atual

O único backend nativo implementado é Linux x86-64 / ELF64, com geração direta em Rust e sem compilador C/C++, LLVM, assembler ou linker externo.

- Inteiros assinados e sem sinal de 8 a 128 bits, inclusive ABI de parâmetros/retornos de 128 bits em dois registradores e aritmética verificada.
- Decimais binary32/binary64, operações SSE2 e conversões explícitas com verificação de faixa.
- Byte e caractere Unicode com validação do domínio.
- Literais de texto, exibição e operações textuais nativas documentadas na especificação normativa; concatenação dinâmica continua rejeitada.
- Estruturas e tuplas de até 16 bytes nos formatos de campo suportados.
- Enumerações, opcionais e resultados com tag e no máximo um payload escalar.
- Referências e chamadas indiretas de funções para assinaturas suportadas.

O backend rejeita agregados maiores que 16 bytes, enumerações/opcionais/resultados com payloads múltiplos ou não escalares e outros formatos sem classificação ABI implementada. Listas, operações que dependem do runtime nativo e recursos da biblioteca padrão ainda não integrados ao executável nativo também são rejeitados. A ABI é própria da Nebula e não promete compatibilidade com ABI C.

## 6. Fixtures e validação

A cobertura automatizada inclui:

- `compilador/cli/fixtures/tipos/matriz-primitivos.nbx`: tipos escalares, larguras, limites, Unicode, byte e retorno vazio.
- `compilador/cli/fixtures/tipos/compostos.nbx`: estruturas, enumerações, opcionais, resultados, tuplas, vetores e matrizes.
- `conversoes-validas.nbx` e `conversao-invalida.nbx`: conversões e limites.
- `genericos-aninhados.nbx`, `genericos-funcao.nbx` e `genericos-invalidos.nbx`: inferência estrutural.
- `abi-estrutura-pendente.nbx` e `abi-enum-payloads-multiplos.nbx`: casos que o backend deve rejeitar explicitamente.

A CI do workspace Rust valida compilação, formatação, Clippy e testes nos runners configurados. Isso não transforma alvos sem backend em alvos executáveis nem substitui testes de execução no alvo nativo. O status atualizado e as limitações conhecidas estão em [criterios-fase-5.md](criterios-fase-5.md).
