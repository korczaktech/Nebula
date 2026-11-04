# Gramática da Nebula

**Estado:** especificação normativa do frontend sintático. Esta gramática descreve o subconjunto aceito pelo Lexer e pelo Parser do crate `nebula-sintaxe`. Regras de tipos, nomes, mutabilidade e fluxo de controle que dependam de contexto pertencem à análise semântica.

## 1. Notação

- `=` define uma produção; `|` representa alternativas.
- `[ ... ]` indica elemento opcional; `{ ... }` indica zero ou mais repetições.
- Terminais aparecem entre aspas.
- Nomes em maiúsculas representam categorias de tokens de `tokens.md`.
- `EOF` representa o fim do arquivo.
- `NOVA_LINHA` é o token de quebra lógica preservado pelo Lexer.

## 2. Arquivo e itens

~~~ebnf
arquivo                 = { separador }, [ item, { separador, item } ],
                          { separador }, EOF ;
item                    = declaracao | instrucao ;
separador               = NOVA_LINHA | ";" ;

declaracao              = declaracao_variavel
                        | declaracao_constante
                        | declaracao_funcao
                        | declaracao_tipo
                        | declaracao_estrutura
                        | declaracao_enumeracao
                        | declaracao_modulo ;

declaracao_variavel     = ("variavel" | "definir" | "mutavel" | "mutável"),
                          IDENTIFICADOR, [ ":", tipo ], ("=" | "<-"), expressao ;
declaracao_constante    = "constante", IDENTIFICADOR,
                          [ ":", tipo ], ("=" | "<-"), expressao ;
declaracao_funcao       = [ "publico" | "privado" ], "funcao",
                          IDENTIFICADOR, [ "<", lista_genericos, ">" ],
                          "(", [ lista_parametros ], ")",
                          [ ":", tipo ], bloco ;
lista_genericos         = IDENTIFICADOR, { ",", IDENTIFICADOR } ;
lista_parametros        = parametro, { ",", parametro } ;
parametro               = IDENTIFICADOR, ":", tipo ;
declaracao_tipo         = "tipo", IDENTIFICADOR, ("=" | "<-"), tipo ;
declaracao_estrutura    = "estrutura", IDENTIFICADOR, "{",
                          { separador }, [ campo_estrutura,
                          { separador_membro, campo_estrutura },
                          [ separador_membro ] ], { separador }, "}" ;
campo_estrutura         = IDENTIFICADOR, ":", tipo ;
declaracao_enumeracao   = "enumeracao", IDENTIFICADOR, "{",
                          { separador }, variante,
                          { separador_variante, variante },
                          { NOVA_LINHA | ";" }, { separador }, "}" ;
variante                = IDENTIFICADOR, [ "(", [ lista_tipos ], ")" ] ;
lista_tipos             = tipo, { ",", tipo } ;
separador_membro        = "," | NOVA_LINHA | ";" ;
separador_variante      = "," | NOVA_LINHA | ";" ;

declaracao_modulo       = "incluir", caminho_modulo, [ "como", IDENTIFICADOR ] ;
caminho_modulo          = IDENTIFICADOR, { "::", IDENTIFICADOR } ;

tipo                    = tipo_funcao | tipo_lista | tipo_generico ;
tipo_funcao             = "funcao", "(", [ lista_tipos ], ")",
                          [ ":", tipo ] ;
tipo_lista              = "[", tipo, "]" ;
tipo_generico           = caminho_qualificado, [ "<", lista_tipos, ">" ] ;
caminho_qualificado     = IDENTIFICADOR, { "::", IDENTIFICADOR } ;
~~~

O retorno de função omitido equivale ao tipo `vazio`. Tipos genéricos, tipos de função e listas podem ser aninhados. Os nomes e a compatibilidade dos tipos são verificados semanticamente.

## 3. Instruções e blocos

~~~ebnf
instrucao               = declaracao_variavel
                        | declaracao_constante
                        | instrucao_alterar
                        | declaracao_funcao
                        | declaracao_tipo
                        | declaracao_estrutura
                        | declaracao_enumeracao
                        | declaracao_modulo
                        | instrucao_quando
                        | instrucao_escolha
                        | instrucao_enquanto
                        | instrucao_cada
                        | instrucao_pulso
                        | instrucao_para
                        | instrucao_devolver
                        | instrucao_controle
                        | expressao ;

bloco                   = bloco_chaves | bloco_fim ;
bloco_chaves            = { NOVA_LINHA }, "{", { separador },
                          lista_itens, "}" ;
bloco_fim               = NOVA_LINHA, { separador },
                          lista_itens, "fim" ;
lista_itens             = [ item, { separador, item }, { separador } ] ;

instrucao_quando        = "quando", expressao, bloco,
                          [ { NOVA_LINHA }, "senao",
                            ( instrucao_quando | bloco ) ] ;
instrucao_escolha       = ("escolha" | "avaliar"), expressao, "{",
                          { separador }, caso_escolha,
                          { { separador }, caso_escolha },
                          { separador }, "}" ;
caso_escolha            = "caso", padrao_escolha, bloco ;
padrao_escolha          = "_" | caminho_variante, [ "(", [ lista_vinculos ], ")" ] ;
caminho_variante        = IDENTIFICADOR, "::", IDENTIFICADOR ;
lista_vinculos          = IDENTIFICADOR, { ",", IDENTIFICADOR } ;

instrucao_enquanto      = ("enquanto" | "durante" | "repetir"),
                          expressao, bloco ;
instrucao_cada          = "cada", IDENTIFICADOR, "em", expressao, bloco ;
instrucao_pulso         = "pulso", "(", expressao, ")", bloco ;
instrucao_para          = "para", "(",
                          [ "variavel", IDENTIFICADOR,
                            [ ":", tipo ], ("=" | "<-"), expressao
                          | expressao ], ";",
                          [ expressao ], ";", [ expressao ], ")", bloco ;
instrucao_alterar       = "alterar", atribuicao ;
instrucao_devolver      = "devolver", [ expressao ] ;
instrucao_controle      = ("parar" | "encerrar") | "seguir" ;
~~~

O Parser também aceita `definir` e `mutavel`/`mutável` como aliases de declaração de variável; `avaliar` como alias de `escolha`; `durante` e `repetir` como aliases de `enquanto`; e `encerrar` como alias de `parar`. `alterar` exige uma expressão de atribuição. A validação de que o destino é modificável e de que os comandos de controle estão em contexto válido é semântica.

Os blocos podem usar chaves ou terminador `fim`. Um bloco iniciado por quebra de linha exige seu próprio `fim`. Blocos aninhados são fechados do mais interno para o mais externo. O `senao` associa-se ao `quando` aberto mais próximo que ainda não tenha alternativa.

## 4. Expressões

~~~ebnf
expressao               = atribuicao ;
atribuicao              = logica_ou, [ ("=" | "<-"), atribuicao ] ;
logica_ou               = logica_e, { "ou", logica_e } ;
logica_e                = igualdade, { "e", igualdade } ;
igualdade               = comparacao, { ("==" | "!="), comparacao } ;
comparacao              = intervalo, [ ("<" | "<=" | ">" | ">="), intervalo ] ;
intervalo               = aditiva, [ "..", aditiva ] ;
aditiva                 = multiplicativa, { ("+" | "-"), multiplicativa } ;
multiplicativa          = unaria, { ("*" | "/" | "%"), unaria } ;
unaria                  = ("!" | "nao" | "-" | "+"), unaria | potencia ;
potencia                = posfixa, [ "**", unaria ] ;
posfixa                 = primaria, { chamada | indexacao | acesso_membro } ;
chamada                 = "(", [ lista_argumentos ], ")" ;
lista_argumentos        = expressao, { ",", expressao } ;
indexacao               = "[", expressao, "]" ;
acesso_membro           = ("." | "::"), IDENTIFICADOR ;
primaria                = IDENTIFICADOR | literal | tupla | lista
                        | construcao_estrutura ;
tupla                   = "(", ")" | "(", expressao, ",",
                          [ expressao, { ",", expressao }, [ "," ] ], ")"
                        | "(", expressao, ")" ;
lista                   = "[", [ lista_argumentos ], "]" ;
construcao_estrutura    = IDENTIFICADOR, "{",
                          [ campo_valor, { separador_membro, campo_valor },
                            [ separador_membro ] ], "}" ;
campo_valor             = IDENTIFICADOR, ":", expressao ;
~~~

A atribuição e a potência associam à direita. Os operadores aritméticos, lógicos e de igualdade associam à esquerda. Comparações relacionais e operadores de intervalo não podem ser encadeados sem agrupamento explícito. A análise semântica valida o destino da atribuição.

Precedência, da maior para a menor: chamada/indexação/acesso a membro; potência; unários; multiplicação/divisão/resto; adição/subtração; intervalo; comparação; igualdade; E; OU; atribuição. O Parser usa precedência Pratt equivalente a essa ordem.

## 5. Literais

~~~ebnf
literal                  = INTEIRO | DECIMAL | TEXTO | CARACTERE
                         | "verdadeiro" | "falso" | "nulo" ;
~~~

As bases numéricas, escapes, normalização de identificadores, comentários e restrições de texto/caractere estão definidos em `tokens.md`.

## 6. Quebras de linha e separadores

- Um separador é obrigatório entre itens independentes.
- Separadores repetidos e linhas em branco são permitidos.
- Um separador imediatamente antes de `}`, `fim` ou `EOF` é permitido.
- Dentro de parênteses e colchetes, quebras de linha não separam itens.
- Quebras depois de operadores que ainda precisam de operando continuam a expressão.
- Quebras antes de operadores que podem completar a expressão anterior continuam a expressão.
- O acesso a membro permite quebra antes do operador e depois dele; o nome do membro vem após o operador.
- Caminhos de tipos e módulos permitem quebra antes e depois de `::`; padrões de `escolha` exigem `Enum::Variante` sem quebra dentro do par.
- Quebras após vírgulas continuam listas de argumentos, parâmetros, tipos, tuplas e vínculos.
- Uma quebra imediatamente antes de `senao` não separa a alternativa do `quando`.
- A indentação não cria nem fecha blocos.
- A chave de abertura de blocos de funções e instruções pode ficar na linha seguinte ao cabeçalho; construções de estrutura em expressões exigem `{` imediatamente após o nome.
- Dentro de chaves, quebras de linha continuam podendo separar itens.

## 7. Restrições sintáticas e semânticas

O Parser rejeita:
- Uso de palavras reservadas para recursos futuros como construções ou identificadores.
- Comparações relacionais encadeadas.
- Operadores de intervalo encadeados.
- Parâmetros genéricos repetidos.
- Campos repetidos em uma estrutura.
- Variantes repetidas ou enumerações sem variantes.
- Campos repetidos em uma construção de estrutura.
- Mais de um caso curinga em uma instrução `escolha`.
- Repetição do mesmo caminho `Enum::Variante` na mesma instrução `escolha`.
- Instruções `alterar` cujo conteúdo não seja uma atribuição.

A existência de nomes, tipos, campos, variantes, mutabilidade do destino e validade contextual de `devolver`, `parar` e `seguir` pertence à análise semântica.

## 8. Fora do subconjunto sintático

Tratamento de exceções (`tentar`, `capturar`, `finalmente`), funções assíncronas (`assinc`), interpolação, strings multilinha, chamadas sem parênteses e declarações de função sem lista de parâmetros explícita permanecem fora desta gramática. Palavras reservadas futuras não habilitam esses recursos.

## 9. Exemplos de referência

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

estrutura Pessoa {
    nome: texto,
    idade: inteiro
}

enumeracao Resultado {
    Sucesso(texto),
    Falha(inteiro)
}

escolha resultado {
    caso Resultado::Sucesso(mensagem) { exibir(mensagem) }
    caso Resultado::Falha(codigo) { exibir(codigo) }
    caso _ { exibir("desconhecido") }
}
~~~

Forma de bloco com terminador:

~~~nebula
funcao executar()
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
