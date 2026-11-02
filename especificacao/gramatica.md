# Gramática inicial da Nebula

**Estado:** gramática normativa do subconjunto sintático inicial. O Lexer, a AST e o Parser estão implementados no crate `nebula-sintaxe`; os critérios e a validação da implementação estão em `especificacao/criterios-fase-2.md`.

## 1. Notação

- = define uma produção; | representa alternativa.
- [ ... ] indica elemento opcional; { ... } indica zero ou mais repetições.
- Terminais aparecem entre aspas.
- Nomes em maiúsculas representam classes de tokens descritas em tokens.md.
- EOF representa fim de arquivo.
- A gramática descreve sintaxe, não validação de tipos, nomes, alvos de atribuição ou fluxo de controle.

## 2. Arquivo e separadores

~~~ebnf
arquivo          = { separador }, [ item, { separador, item } ], { separador }, EOF ;
item             = declaracao | instrucao ;
separador        = NOVA_LINHA | ";" ;
~~~

Um separador é obrigatório entre dois itens independentes. Separadores repetidos e linhas em branco são permitidos. Um separador imediatamente antes de }, fim ou EOF é permitido.

A quebra de linha pode ser tratada como continuação, e não como separador, quando a expressão ainda estiver incompleta: depois de operador binário, vírgula ou antes de um operador que complete a expressão anterior. Dentro de () e [], quebras são espaços. A indentação não tem significado sintático.

Uma quebra de linha imediatamente antes de senao continua a estrutura condicional anterior; não separa o senao do bloco que o antecede.

## 3. Declarações

~~~ebnf
declaracao           = declaracao_variavel
                     | declaracao_constante
                     | declaracao_funcao
                     | declaracao_tipo
                     | declaracao_modulo ;

declaracao_variavel  = "variavel", IDENTIFICADOR,
                       [ ":", tipo ], "=", expressao ;
declaracao_constante = "constante", IDENTIFICADOR,
                       [ ":", tipo ], "=", expressao ;

declaracao_funcao    = [ "publico" | "privado" ], "funcao",
                       IDENTIFICADOR, "(", [ lista_parametros ], ")",
                       [ ":", tipo ], bloco ;
lista_parametros     = parametro, { ",", parametro } ;
parametro            = IDENTIFICADOR, ":", tipo ;

declaracao_tipo      = "tipo", IDENTIFICADOR, "=", tipo ;
tipo                 = caminho_tipo | "[", tipo, "]" ;
caminho_tipo         = IDENTIFICADOR, { "::", IDENTIFICADOR } ;

declaracao_modulo    = "incluir", caminho_modulo,
                       [ "como", IDENTIFICADOR ] ;
caminho_modulo       = IDENTIFICADOR, { "::", IDENTIFICADOR } ;
~~~

O tipo de retorno de uma função é vazio quando omitido. A gramática admite nomes de tipos como identificadores; a existência e compatibilidade de cada tipo serão verificadas semanticamente em fase posterior.

## 4. Blocos e instruções

~~~ebnf
bloco                = bloco_chaves | bloco_fim ;
bloco_chaves         = { NOVA_LINHA }, "{", { separador }, [ item, { separador, item } ],
                       { separador }, "}" ;
bloco_fim            = NOVA_LINHA, { separador },
                       [ item, { separador, item } ],
                       { separador }, "fim" ;

instrucao            = declaracao_variavel
                     | declaracao_constante
                     | expressao
                     | instrucao_quando
                     | instrucao_enquanto
                     | instrucao_para
                     | instrucao_devolver
                     | instrucao_controle ;

instrucao_quando     = "quando", condicao, bloco,
                       [ { NOVA_LINHA }, "senao",
                         ( bloco | instrucao_quando ) ] ;
condicao             = expressao ;

instrucao_enquanto   = "enquanto", condicao, bloco ;
instrucao_para       = "para", "(",
                       [ declaracao_variavel | expressao ], ";",
                       [ expressao ], ";",
                       [ expressao ], ")", bloco ;

instrucao_devolver   = "devolver", [ expressao ] ;
instrucao_controle   = "parar" | "seguir" ;
~~~

Para quando e enquanto, parênteses ao redor da condição são opcionais quando o início do bloco delimita a expressão sem ambiguidade. A forma com parênteses continua válida e recomendada quando melhora a leitura. O cabeçalho de para mantém parênteses e dois pontos e vírgulas obrigatórios para separar inicialização, condição e atualização.

Blocos podem usar chaves ou o terminador fim. Cada bloco iniciado na forma fim exige seu próprio fim; blocos aninhados são fechados do mais interno para o mais externo. O senao associa-se ao quando aberto mais próximo que ainda não tenha senao.

## 5. Expressões

~~~ebnf
expressao           = atribuicao ;
atribuicao          = logica_ou, [ "=", atribuicao ] ;
logica_ou           = logica_e, { "ou", logica_e } ;
logica_e            = igualdade, { "e", igualdade } ;
igualdade           = comparacao, { ( "==" | "!=" ), comparacao } ;
comparacao          = intervalo,
                      [ ( "<" | "<=" | ">" | ">=" ), intervalo ] ;
intervalo           = aditiva, [ "..", aditiva ] ;
aditiva             = multiplicativa, { ( "+" | "-" ), multiplicativa } ;
multiplicativa      = unaria, { ( "*" | "/" | "%" ), unaria } ;
unaria              = ( "!" | "nao" | "-" | "+" ), unaria | potencia ;
potencia            = posfixa, [ "**", unaria ] ;
posfixa             = primaria, { chamada | indexacao | acesso_membro } ;
chamada             = "(", [ lista_argumentos ], ")" ;
lista_argumentos    = expressao, { ",", expressao } ;
indexacao           = "[", expressao, "]" ;
acesso_membro       = ( "." | "::" ), IDENTIFICADOR ;
primaria             = IDENTIFICADOR | literal | "(", expressao, ")"
                     | lista_literal ;
lista_literal       = "[", [ lista_argumentos ], "]" ;
~~~

A atribuição e a potência associam à direita. Operadores aritméticos, lógicos e de igualdade em sequência associam à esquerda. Comparações relacionais encadeadas não são permitidas: uma produção de comparação contém no máximo um operador relacional. A análise semântica validará se o lado esquerdo de uma atribuição é modificável.

Precedência da maior para a menor: chamada/indexação/acesso; potência; unários; multiplicação/divisão/resto; adição/subtração; intervalo; comparação; igualdade; E; OU; atribuição.

## 6. Literais

~~~ebnf
literal = INTEIRO | DECIMAL | TEXTO | CARACTERE
        | "verdadeiro" | "falso" | "nulo" ;
~~~

As formas numéricas, escapes e restrições de texto/caractere estão definidas em tokens.md.

## 7. Escopo desta gramática

Fazem parte do subconjunto sintático inicial: variáveis, constantes, funções, parâmetros tipados, aliases de tipo, inclusão de módulos, blocos, condicionais, laços enquanto e para, retorno, controle de laço, chamadas, listas, indexação, acesso a membros, operadores e literais descritos acima.

As palavras reservadas futuras listadas em tokens.md não possuem produções nesta gramática. Estruturas, enumerações, tratamento de exceções, funções assíncronas, interpolação, strings multilinha, chamadas sem parênteses e declarações de função sem lista de parâmetros explícita ficam fora do subconjunto inicial.

## 8. Casos de referência

Forma com delimitadores explícitos:

~~~nebula
funcao principal(): vazio {
    variavel nome: texto = "Nebula"
    constante limite: inteiro = 10
    quando limite > 0 {
        exibir(nome)
    } senao {
        exibir("vazio")
    }
}
~~~

Forma de bloco com terminador:

~~~nebula
funcao principal(): vazio
    variavel ativo = verdadeiro
    enquanto ativo
        ativo = falso
    fim
fim
~~~

Forma com separadores explícitos:

~~~nebula
variavel total = 10; constante taxa = 2; exibir(total * taxa)
~~~

Esses exemplos são casos de referência; a CLI `nebula verificar` permite executá-los contra a implementação do Parser.
