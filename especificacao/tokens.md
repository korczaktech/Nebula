# Tokens e regras léxicas

**Estado:** especificação léxica normativa do subconjunto inicial da Nebula. Recursos reservados para versões futuras estão identificados separadamente e não fazem parte da gramática atual.

## 1. Codificação e posições

- A entrada DEVE ser UTF-8 válido. Sequências inválidas são erro de fonte, com intervalo em bytes quando disponível.
- O compilador DEVE preservar offsets inicial e final em bytes, com início inclusivo e fim exclusivo.
- Linha e coluna são base 1. A coluna conta pontos de código Unicode desde o início da linha; a interface PODE calcular largura visual de grafemas.
- Identificadores são comparados após normalização Unicode NFC. O texto original é preservado para diagnósticos.
- Finais de linha LF e CRLF são aceitos. CRLF representa uma única quebra lógica. CR isolado é rejeitado na versão inicial.

## 2. Espaços e quebras de linha

- Espaço ASCII (U+0020) e tabulação horizontal (U+0009) são ignorados entre tokens. Outros caracteres de espaço não são aceitos como separadores na versão inicial.
- A quebra de linha é preservada como token NOVA_LINHA.
- Dentro de parênteses () e colchetes [], a quebra de linha funciona como espaço e não separa instruções.
- Dentro de chaves {}, quebras de linha continuam podendo separar instruções.
- A indentação nunca cria ou fecha blocos e não altera o significado do programa.
- Uma quebra pode ser continuação quando a expressão anterior está incompleta, por exemplo após operador binário ou vírgula. A regra sintática de separação está em gramatica.md.
- Comentários de linha terminam antes da quebra de linha; a própria quebra continua disponível como separador.

## 3. Identificadores e palavras reservadas

Identificador:
- Inicia com caractere Unicode da propriedade XID_Start ou sublinhado.
- Continua com caracteres XID_Continue ou sublinhado.
- É sensível a maiúsculas e minúsculas.
- É comparado em NFC.
- Não pode coincidir com palavra reservada.

### Palavras reservadas da gramática inicial

variavel, constante, funcao, quando, senao, para, enquanto, devolver, incluir, como, verdadeiro, falso, nulo, fim, parar, seguir, e, ou, nao, publico, privado, tipo.

Os nomes de tipos como inteiro, logico, texto, caractere, byte e vazio são identificadores usados em posição de tipo nesta etapa; não são palavras reservadas léxicas.

### Palavras reservadas para estudo futuro

cada, tentar, capturar, finalmente, assinc, aguardar, estrutura, enumeracao.

Essas palavras ficam reservadas para evitar incompatibilidades futuras, mas **não são construções válidas na gramática inicial**. O Lexer pode classificá-las como palavras reservadas futuras; o Parser deve rejeitá-las como construções não suportadas, com diagnóstico claro. Não se deve sugerir que seus recursos já existem.

## 4. Números

- Inteiro decimal: um ou mais dígitos decimais, com sublinhados somente entre dígitos.
- Bases não decimais: prefixos 0x/0X, 0b/0B e 0o/0O, seguidos de ao menos um dígito válido para a base.
- Decimal: dígitos, ponto seguido obrigatoriamente de ao menos um dígito e expoente opcional; ou dígitos seguidos de expoente.
- Expoente: e ou E, sinal opcional e ao menos um dígito.
- Sublinhado não pode iniciar, terminar nem aparecer consecutivamente; também não pode ficar junto ao prefixo, ponto ou marcador de expoente.
- 1. é token inteiro seguido de ponto, não literal decimal. Para decimal, escreva 1.0.
- O Lexer identifica a forma do literal; faixa, overflow e conversões são verificados em fases posteriores.

## 5. Texto e caractere

- Texto usa aspas duplas e não pode atravessar uma quebra de linha física na versão inicial.
- Caractere usa aspas simples e representa exatamente um escalar Unicode após a interpretação dos escapes.
- Escapes aceitos, escritos exatamente como aparecem no código-fonte:

~~~text
\\   \\"   \\'   \\n   \\r   \\t   \\0   \\u{...}
~~~

- O escape Unicode contém um ou mais dígitos hexadecimais e deve representar um valor escalar Unicode válido.
- Escape desconhecido, sequência Unicode inválida e literal sem fechamento são erros.
- Interpolação e strings multilinha não fazem parte da gramática inicial.

## 6. Comentários

- Linha: // até antes da próxima quebra de linha.
- Bloco: /* ... */, encerrado pelo primeiro */.
- Comentários de bloco não são aninhados.
- Comentário de bloco não terminado é erro.
- Marcadores de comentário dentro de literais não iniciam comentários.

## 7. Pontuação e delimitadores

(, ), {, }, [, ], vírgula, ponto e vírgula, dois-pontos e ponto.

## 8. Operadores

=, +, -, *, /, %, **, ==, !=, <, <=, >, >=, !, .., ::.

Operadores lógicos textuais: e, ou, nao.

O Lexer aplica maior correspondência: >= antes de >, == antes de =, :: antes de :, ** antes de * e .. antes de .

?? e ~> são reservados para estudo, mas não são operadores da gramática inicial.

## 9. Fim de arquivo e erros léxicos

O fluxo contém um token de fim de arquivo. O Lexer deve diagnosticar, no mínimo, UTF-8 inválido, caractere inesperado, literal não terminado, escape inválido, número malformado e comentário de bloco não terminado. Cada diagnóstico deve preservar a localização disponível.
