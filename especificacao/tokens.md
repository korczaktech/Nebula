# Tokens e regras léxicas

## Codificação e posições

- Entrada UTF-8.
- Rejeitar sequências UTF-8 inválidas com diagnóstico e intervalo em bytes.
- Preservar offset inicial/final em bytes, linha e coluna.
- Colunas devem contar pontos de código Unicode; a apresentação pode calcular largura de grafemas.
- Identificadores são comparados em NFC; o texto original deve ser preservado para diagnósticos.

## Categorias de tokens

- Identificadores e palavras reservadas.
- Literais inteiros, decimais, texto, caractere, lógico e nulo.
- Operadores.
- Pontuação e delimitadores.
- Comentários.
- Quebras de linha.
- Fim de arquivo.

## Maior correspondência

O Lexer deve sempre selecionar o token válido mais longo. Exemplos: `>=` antes de `>`, `==` antes de `=`, `::` antes de `:`, `**` antes de `*`, `..` antes de `.`.

## Espaços e quebras

Espaços horizontais são ignorados entre tokens. Quebras de linha são preservadas para a regra de separação de instruções; dentro de parênteses e colchetes são tratadas como continuação. Tabulações são aceitas como espaço visual, sem significado de escopo. Indentação não gera tokens de bloco.

## Números

- Inteiro decimal: dígitos com sublinhados internos opcionais.
- Bases: prefixos `0x`, `0b`, `0o`.
- Decimal: parte fracionária opcional e expoente opcional.
- Sublinhado não pode iniciar, terminar ou aparecer consecutivamente.
- Overflow é verificado na análise semântica/constante, não mascarado pelo Lexer.

## Texto e caractere

- Texto entre aspas duplas.
- Caractere entre aspas simples e representando exatamente um escalar Unicode após escapes.
- Escapes mínimos propostos: `\\\\`, `\"`, `\'`, `\n`, `\r`, `\t`, `\0`, `\u{...}`.
- Literal não terminado é erro.
- Strings multilinha e interpolação não fazem parte do recorte inicial.

## Comentários

- `//` até o fim da linha.
- `/* ... */` até o primeiro fechamento.
- Comentário de bloco não aninhado na primeira versão.
- Comentário não pode ocultar o fechamento de um literal.

## Pontuação inicial

`()`, `{}`, `[]`, `,`, `;`, `:`, `.`.

## Operadores iniciais

`=`, `+`, `-`, `*`, `/`, `%`, `**`, `==`, `!=`, `<`, `<=`, `>`, `>=`, `!`, `..`, `::`.

Operadores lógicos textuais: `e`, `ou`, `nao`. Operadores `??` e `~>` permanecem reservados para estudo e não fazem parte da gramática-base até que sua semântica seja definida.
