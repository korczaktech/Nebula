# Tokens e regras léxicas

**Estado:** especificação léxica normativa alinhada ao Lexer do crate `nebula-sintaxe`. Palavras reservadas futuras são identificadas separadamente e não fazem parte das construções aceitas pelo Parser.

## 1. Codificação e posições

- A entrada DEVE ser UTF-8 válido. Sequências inválidas são erro de fonte, com intervalo em bytes quando disponível.
- O compilador DEVE preservar offsets inicial e final em bytes, com início inclusivo e fim exclusivo.
- Linha e coluna são base 1. A coluna conta pontos de código Unicode desde o início da linha; a interface PODE calcular largura visual de grafemas.
- Identificadores são comparados após normalização Unicode NFC. O texto original é preservado para diagnósticos.
- Finais de linha LF e CRLF são aceitos. CRLF representa uma única quebra lógica. CR isolado é rejeitado.

## 2. Espaços, quebras e indentação

- Espaço ASCII (U+0020) e tabulação horizontal (U+0009) são ignorados entre tokens.
- Código normativo DEVE usar espaços para indentação. Tabulações em posições de indentação geram diagnóstico `N1012`.
- A quebra de linha é preservada como token `NOVA_LINHA`, exceto quando suprimida pelas regras de agrupamento.
- Dentro de parênteses `()` e colchetes `[]`, a quebra física funciona como espaço e não separa instruções.
- Dentro de chaves, quebras de linha continuam disponíveis como separadores; a indentação não cria blocos sintáticos.
- `INDENT` e `DEDENT` não delimitam Function Body. A abertura e o fechamento sintáticos são `: {`, `fim` e `}`.
- Linhas vazias e linhas contendo apenas comentários não alteram o estado de indentação.
- Uma dedentação fora de delimitadores deve corresponder a uma coluna previamente aberta; caso contrário, o Lexer emite `N1013`.
- No fim do arquivo, níveis lexicais restantes são encerrados antes de `EOF`.
- A indentação é obrigatória como organização visual e deve ser consistente, mas nunca substitui o encerramento explícito do bloco.
- A forma antiga sem `: {` é rejeitada pelo Parser.

## 3. Identificadores e palavras reservadas

Identificador:
- Inicia com caractere Unicode da propriedade XID_Start ou sublinhado.
- Continua com caracteres XID_Continue ou sublinhado.
- É sensível a maiúsculas e minúsculas.
- É comparado em NFC.
- Não pode coincidir com palavra reservada.

### Palavras reconhecidas pelo Lexer e usadas na gramática

variavel, definir, mutavel, constante, funcao, classe, herda, se, quando, sendo, senao, para, dentro, enquanto, escolha, caso, padrao, pare, prossiga, passar, retorne, devolver, lambda, ceder, incluir, biblioteca, modulo, de, como, tente, erro, sempre, lancar, verifique, com, async, aguarda, global, externa, isto, novo, apagar, identico, nao, fora, e, ou, sim, verdadeiro, falso, nulo, iguais, tipoDe, instanciaDe, obter, vazio, exportar, construtor, estatico, privado, publico, protegido, seguro, critico, privilegio, montagem, tipo, estrutura, enumeracao, avaliar, cada, em, pulso, durante, repetir, alterar, fim, parar, encerrar, seguir.

### Palavras reservadas futuras

Palavras classificadas como reservadas futuras pelo Lexer são reconhecidas como tokens especiais, mas o Parser emite diagnóstico se forem usadas como construções ainda não implementadas. A classificação exata deve acompanhar o enum `TipoToken::ReservadaFutura` e os testes do Lexer.

Nomes de tipos como `inteiro`, `logico`, `texto`, `caractere`, `byte`, `u8`, `u16`, `u32`, `u64` e `vazio` são identificadores em posição de tipo, com a exceção do token reservado `vazio` reconhecido diretamente pelo Lexer.

## 4. Números

- Inteiro decimal: um ou mais dígitos decimais, com sublinhados somente entre dígitos.
- Bases não decimais: prefixos `0x/0X`, `0b/0B` e `0o/0O`, seguidos de ao menos um dígito válido para a base.
- Decimal: dígitos, ponto seguido obrigatoriamente de ao menos um dígito e expoente opcional; ou dígitos seguidos de expoente.
- Expoente: `e` ou `E`, sinal opcional e ao menos um dígito.
- Sublinhado não pode iniciar, terminar nem aparecer consecutivamente; também não pode ficar junto ao prefixo, ponto ou marcador de expoente.
- `1.` é token inteiro seguido de ponto, não literal decimal. Para decimal, escreva `1.0`.
- O Lexer identifica a forma do literal; faixa, overflow e conversões são verificados em fases posteriores.

## 5. Texto e caractere

- Texto usa aspas duplas. A forma simples não atravessa uma quebra física de linha.
- Caractere usa aspas simples e representa exatamente um escalar Unicode após a interpretação dos escapes.
- O Lexer também possui token específico para texto interpolado; sua estrutura interna e semântica devem permanecer alinhadas à implementação correspondente.
- Escapes reconhecidos:

~~~text
\\   \"   \'   \n   \r   \t   \0   \u{...}
~~~

- O escape Unicode contém um ou mais dígitos hexadecimais e deve representar um valor escalar Unicode válido.
- Escape desconhecido, sequência Unicode inválida e literal sem fechamento são erros.

## 6. Comentários

- Linha: `//` até antes da próxima quebra de linha.
- Bloco: `/* ... */`, encerrado pelo primeiro `*/`.
- Comentários de bloco não são aninhados.
- Comentário de bloco não terminado é erro.
- Marcadores de comentário dentro de literais não iniciam comentários.
- Linhas contendo apenas comentários não alteram indentação.

## 7. Pontuação e delimitadores

`(`, `)`, `{`, `}`, `[`, `]`, vírgula, ponto e vírgula, dois-pontos, ponto, arroba e os tokens estruturais `NOVA_LINHA`, `INDENT`, `DEDENT`, `FIM_BLOCO` e `EOF`.

## 8. Function Body e importações\n\nO Lexer reconhece a palavra reservada `async` como token próprio; `assinc` não é palavra reservada e é classificada como identificador. A estrutura `incluir(modulo: ..., biblioteca: ...)` é formada lexicalmente pelos tokens `INCLUIR`, `ABRE_PAR`, `MODULO`, `DOIS_PONTOS`, identificadores, `VIRGULA`, `BIBLIOTECA`, `DOIS_PONTOS` e `FECHA_PAR`.\n\nA abertura e o fechamento visual de Function Body são emitidos como `ABRE_CHAVE` e `FECHA_CHAVE`; a palavra `fim` é emitida como `FIM_BLOCO`. O Lexer preserva esses elementos e suas localizações, mas a exigência de um `fim` por corpo, sua posição antes de `}` e a validade das construções pertencem à validação sintática do Parser.\n\n## 9. Operadores

A implementação reconhece operadores de atribuição, aritméticos, relacionais, igualdade, lógicos, bit a bit, deslocamento, acesso e operadores opcionais. Entre as grafias reconhecidas estão:

~~~text
=  ->  +  -  *  /  //  %  **  +=  -=  *=  /=  //=  %=  **=
==  !=  !==  <  <=  >  >=  ..  !  ++  --
&&  ||  &  |  ^  ~  <<  >>  >>>  ??  ??=  ?.  ?  ::  ...
~~~

Operadores textuais incluem `e`, `ou`, `nao`, `dentro`, `fora`, `tipoDe` e `instanciaDe`. A precedência e associatividade são definidas em `gramatica.md`.

O Lexer aplica maior correspondência aos operadores compostos antes de seus prefixos.

## 10. Fim de arquivo e erros léxicos

O fluxo contém token de fim de arquivo. O Lexer diagnostica, no mínimo, UTF-8 inválido, caractere inesperado, literal não terminado, escape inválido, número malformado, comentário de bloco não terminado, CR isolado, tabulação em indentação e coluna de dedentação inexistente. Cada diagnóstico preserva a localização disponível.

## 11. Grafias portuguesas equivalentes

O Lexer aceita as grafias sem acento e suas formas ortográficas acentuadas quando implementadas como aliases:
- `funcao` e `função`;
- `senao` e `senão`;
- `enumeracao` e `enumeração`;
- `nao` e `não`;
- `publico` e `público`;
- `mutavel` e `mutável`.

As duas grafias de cada par produzem o mesmo token. A normalização NFC continua aplicada antes da classificação das palavras.
