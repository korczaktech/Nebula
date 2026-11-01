# Gramática inicial da Nebula

**Estado:** gramática de trabalho da Fase 1. Notação EBNF simplificada; detalhes léxicos estão em `tokens.md`. Produções marcadas como provisórias devem ser consolidadas antes da Fase 2.

## 1. Convenções

- `A = B` define uma produção.
- `|` representa alternativa.
- `[ ... ]` indica opcionalidade na metassintaxe.
- `{ ... }` indica repetição na metassintaxe.
- Terminais aparecem entre aspas.
- `IDENTIFICADOR`, `INTEIRO`, `TEXTO` e outros nomes em maiúsculas são classes de tokens.

## 2. Unidade de compilação

```ebnf
arquivo          = { separador } { ( declaracao | instrucao ) { separador } } ;
separador        = terminador_linha | ";" ;
terminador_linha = NOVA_LINHA ;
declaracao       = declaracao_variavel
                 | declaracao_constante
                 | declaracao_funcao
                 | declaracao_tipo
                 | declaracao_modulo ;
```

A implementação deve preservar as quebras de linha. Uma quebra separa instruções somente quando não estiver dentro de agrupamento e a instrução anterior estiver sintaticamente completa. O parser não deve depender da indentação.

## 3. Declarações

```ebnf
declaracao_variavel  = "variavel" IDENTIFICADOR ( ":" tipo [ "=" expressao ] | "=" expressao ) ;
declaracao_constante = "constante" IDENTIFICADOR [ ":" tipo ] "=" expressao ;
declaracao_funcao    = [ "publico" | "privado" ] "funcao" IDENTIFICADOR
                       "(" [ lista_parametros ] ")" [ ":" tipo ] bloco ;
lista_parametros     = parametro { "," parametro } ;
parametro            = IDENTIFICADOR ":" tipo ;
declaracao_tipo      = "tipo" IDENTIFICADOR "=" tipo ;
tipo                 = IDENTIFICADOR [ "<" lista_tipos ">" ]
                     | "[" tipo "]"
                     | "?" tipo
                     | "(" [ lista_tipos ] ")" "->" tipo ;
lista_tipos          = tipo { "," tipo } ;
declaracao_modulo    = "incluir" caminho_modulo [ "como" IDENTIFICADOR ] ;
caminho_modulo       = IDENTIFICADOR { "::" IDENTIFICADOR } ;
```

## 4. Blocos e instruções

```ebnf
bloco          = bloco_chaves | bloco_fim ;
bloco_chaves   = "{" { separador } { ( instrucao | declaracao ) { separador } } "}" ;
bloco_fim      = terminador_linha { separador } { ( instrucao | declaracao ) { separador } } "fim" ;
instrucao      = declaracao_variavel
               | expressao
               | instrucao_se
               | instrucao_enquanto
               | instrucao_para
               | instrucao_devolver
               | instrucao_controle ;
instrucao_se   = "quando" ( "(" expressao ")" | expressao ) bloco
                 [ "senao" ( bloco | instrucao_se ) ] ;
instrucao_enquanto = "enquanto" ( "(" expressao ")" | expressao ) bloco ;
instrucao_para = "para" "(" [ declaracao_variavel | expressao ] ";"
                 [ expressao ] ";" [ expressao ] ")" bloco ;
instrucao_devolver = "devolver" [ expressao ] ;
instrucao_controle = "parar" | "seguir" ;
```

**Nota de resolução:** a forma sem parênteses em `quando` e `enquanto` é provisória e só será válida se a expressão puder ser delimitada sem ambiguidade. A forma canônica inicial usa parênteses e blocos explícitos. A alternativa `bloco_fim` termina em `fim`; blocos aninhados precisam de um `fim` para cada nível.

## 5. Expressões

```ebnf
expressao       = atribuicao ;
atribuicao      = logica_ou [ "=" atribuicao ] ;
logica_ou       = logica_e { "ou" logica_e } ;
logica_e        = igualdade { "e" igualdade } ;
igualdade       = comparacao { ( "==" | "!=" ) comparacao } ;
comparacao      = intervalo { ( "<" | "<=" | ">" | ">=" ) intervalo } ;
intervalo       = aditiva [ ".." aditiva ] ;
aditiva         = multiplicativa { ( "+" | "-" ) multiplicativa } ;
multiplicativa  = potencia { ( "*" | "/" | "%" ) potencia } ;
potencia        = unaria [ "**" potencia ] ;
unaria          = ( "!" | "nao" | "-" | "+" ) unaria | posfixa ;
posfixa         = primaria { chamada | indexacao | acesso_membro } ;
chamada         = "(" [ lista_argumentos ] ")" ;
lista_argumentos= expressao { "," expressao } ;
indexacao       = "[" expressao "]" ;
acesso_membro   = "." IDENTIFICADOR | "::" IDENTIFICADOR ;
primaria        = IDENTIFICADOR | literal | "(" expressao ")"
                | lista_literal ;
lista_literal   = "[" [ lista_argumentos ] "]" ;
```

A precedência é da produção mais externa para a mais interna: atribuição, OU, E, igualdade, comparação, intervalo, soma, multiplicação, potência, unários e pós-fixos. Atribuição é associativa à direita; potência é associativa à direita; as demais cadeias aritméticas e lógicas são associativas à esquerda, salvo comparação encadeada, que não é permitida na primeira versão.

## 6. Literais

```ebnf
literal = INTEIRO | DECIMAL | TEXTO | CARACTERE
        | "verdadeiro" | "falso" | "nulo" ;
```

## 7. Separação de instruções

1. `;` sempre encerra explicitamente a instrução atual fora de literais e comentários.
2. Uma quebra de linha encerra uma instrução quando o parser reconhece um ponto de conclusão válido.
3. Quebras após operadores binários, vírgulas ou antes de um operador que continua a expressão não encerram a instrução.
4. Quebras dentro de `()` e `[]` não encerram instruções.
5. O compilador deve emitir erro de sintaxe quando duas interpretações permanecerem possíveis.
6. Chaves e `fim` delimitam blocos; indentação é ignorada semanticamente.

## 8. Ambiguidades conhecidas a resolver antes da implementação

- Expressões condicionais sem parênteses.
- Declarações de funções sem `()`.
- Desambiguação entre comparação e continuação de linha.
- Sintaxe final de tipos compostos e genéricos.
- Literais multilinha e interpolação de texto.
- Escapes de identificadores reservados.

A gramática é suficiente para orientar a fundação, mas não autoriza declarar o parser implementado.
