## Anexo de gramática formal da Nebula

**Estado:** anexo técnico subordinado à fonte normativa canônica `linguagem-e-tipos.md`. Este arquivo formaliza as produções sintáticas e deve permanecer sincronizado com o contrato principal; não constitui uma segunda fonte normativa independente.


**Estado:** normativa e alinhada ao Lexer, Parser e AST atualmente implementados em `compilador/sintaxe`.
**Regra central:** todo bloco executável ou estrutural usa Function Body: `: { ... }`. Blocos aninhados fecham suas próprias chaves. A palavra `fim` não é reservada nem participa da gramática de blocos. A sintaxe antiga baseada somente em indentação é rejeitada. Chaves de valores de dados, como dicionários em declarações comuns, continuam sendo expressões de dados.

Esta gramática descreve a sintaxe reconhecida. Resolução de nomes, compatibilidade de tipos, mutabilidade, ABI, validade de operações e regras dependentes do contexto pertencem às fases semânticas.

## 1. Notação e tokens estruturais

- `=` define uma produção; `|` representa alternativas.
- `[ ... ]` indica elemento opcional; `{ ... }` indica zero ou mais repetições.
- Terminais aparecem entre aspas.
- Nomes em maiúsculas representam categorias de tokens produzidas pelo Lexer.
- `EOF` é o fim do arquivo.
- `NOVA_LINHA` é uma quebra lógica de linha preservada.
- `ABRE_CHAVE` e `FECHA_CHAVE` delimitam o Function Body.
- `separador` é `NOVA_LINHA` ou `;`.

A indentação organiza visualmente o código e é validada como estilo. Os limites sintáticos dos blocos são definidos por `: {` e `}`; `INDENT` e `DEDENT` não substituem esses delimitadores.

## 2. Regra normativa do Function Body

### 2.1 Princípios obrigatórios

1. Todo corpo executável ou estrutural DEVE começar com dois-pontos seguido de abertura de chave: `: {`.
2. O corpo DEVE terminar na chave `}` correspondente.
3. Blocos aninhados DEVEM fechar suas próprias chaves, respeitando a ordem de abertura.
4. Cláusulas encadeadas, como `sendo`, `senao`, `caso`, `erro` e `sempre`, usam um Function Body cada.
5. A indentação DEVE ser consistente e refletir o aninhamento das chaves.
6. Chaves usadas em valores de dados, como objetos e dicionários em declarações comuns, não se tornam Function Body automaticamente.
7. O Parser DEVE rejeitar abertura ausente, chave de fechamento ausente, fechamento fora de ordem e delimitadores incompatíveis.
8. A palavra `fim` é um identificador comum e nunca fecha um bloco.

### 2.2 Produções fundamentais

```ebnf
corpo       = ":", "{", { separador }, { instrucao | declaracao }, { separador }, "}" ;
bloco       = corpo ;
bloco_funcao = corpo ;
bloco_clausula = corpo ;
separador   = NOVA_LINHA | ";" ;
```


## 3. Arquivo e itens

~~~ebnf
arquivo                 = { separador }, [ item, { separador, item } ],
                          { separador }, EOF ;
item                    = declaracao | instrucao ;

declaracao              = declaracao_variavel
                        | declaracao_constante
                        | declaracao_funcao
                        | declaracao_classe
                        | declaracao_tipo
                        | declaracao_estrutura
                        | declaracao_enumeracao
                        | declaracao_modulo
                        | declaracao_atributo
                        | declaracao_contextual ;

declaracao_variavel     = ("variavel" | "definir" | "mutavel" | "mutável"),
                          IDENTIFICADOR, [ ":", tipo ], "=", expressao ;
declaracao_constante    = "constante", IDENTIFICADOR,
                          [ ":", tipo ], "=", expressao ;

declaracao_funcao       = [ "async" ], { modificador_funcao },
                          "funcao", nome_funcao,
                          [ "<", lista_genericos, ">" ],
                          "(", [ lista_parametros ], ")",
                          [ retorno_funcao ], bloco ;
nome_funcao             = IDENTIFICADOR | "construtor" ;
modificador_funcao      = "publico" | "privado" | "protegido" | "estatico" ;
retorno_funcao          = "->", tipo ;
lista_genericos         = IDENTIFICADOR, { ",", IDENTIFICADOR } ;
lista_parametros        = parametro, { ",", parametro }, [ "," ] ;
parametro               = IDENTIFICADOR, ":", tipo, [ "=", expressao ] ;

declaracao_classe       = "classe", IDENTIFICADOR,
                          [ "herda", IDENTIFICADOR ], bloco ;
declaracao_tipo         = "tipo", IDENTIFICADOR, "=", tipo ;

declaracao_estrutura    = "estrutura", IDENTIFICADOR, ":",
                          "{", { separador }, lista_campos_estrutura,
                          { separador }, "}" ;
lista_campos_estrutura  = [ campo_estrutura,
                          { separador_membro, campo_estrutura },
                          [ separador_membro ] ] ;
campo_estrutura         = IDENTIFICADOR, ":", tipo ;

declaracao_enumeracao   = "enumeracao", IDENTIFICADOR, ":",
                          "{", { separador }, variante,
                          { separador_variante, variante },
                          [ separador_variante ], "}" ;
variante                = IDENTIFICADOR, [ "(", [ lista_tipos ], ")" ] ;
lista_tipos             = tipo, { ",", tipo } ;

declaracao_modulo       = "incluir", [ "biblioteca" | "modulo" ],
                          caminho_modulo,
                          [ "de", caminho_modulo ],
                          [ "como", IDENTIFICADOR ] ;
caminho_modulo          = IDENTIFICADOR, { "::", IDENTIFICADOR } ;

declaracao_atributo     = { atributo }, item ;
atributo                = "@", IDENTIFICADOR, [ "(", [ lista_argumentos ], ")" ] ;

declaracao_contextual   = ("critico" | "seguro" | "privilegio" | "montagem"),
                          [ IDENTIFICADOR ],
                          [ "<", lista_tipos, ">" ],
                          [ "(", [ lista_argumentos ], ")" ], bloco ;
separador_membro        = "," | NOVA_LINHA | ";" ;
separador_variante      = "," | NOVA_LINHA | ";" ;
~~~

Observações normativas:

- O retorno de função omitido equivale a `vazio`.
- A forma `-> tipo` após os parâmetros declara o retorno; `: {` inicia o Function Body. O retorno omitido equivale a `vazio`.
- `externa funcao ...` declara função externa sem corpo. `global nome` e `externa nome` declaram referências de escopo.
- `exportar funcao ...` equivale a uma declaração de função pública.
- Modificadores de função são aceitos antes de `funcao`; `async` marca a função como assíncrona.
- `estrutura` exige `: {` e fecha com `}`. Campos exigem nome, dois-pontos e tipo. Campos repetidos geram diagnóstico.
- `enumeracao` exige `: {`, ao menos uma variante e fecha com `}`. Uma variante pode não ter payload, usar payload vazio `()` ou conter uma lista de tipos. Variantes repetidas são erro.
- Atributos podem ser repetidos antes de uma declaração e recebem argumentos posicionais.
- `montagem` também pode aparecer como expressão de chamada nativa; a forma contextual acima representa o bloco contextual.
- A forma de inclusão aceita caminhos qualificados e origem explícita, por exemplo `incluir raiz de matematica como mat`.

## 4. Instruções

~~~ebnf
instrucao               = declaracao_variavel
                        | declaracao_constante
                        | instrucao_alterar
                        | declaracao_funcao
                        | declaracao_classe
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
                        | instrucao_tente
                        | instrucao_com
                        | instrucao_devolver
                        | instrucao_controle
                        | instrucao_lancar
                        | instrucao_verifique
                        | instrucao_escopo
                        | expressao ;

instrucao_alterar       = "alterar", atribuicao ;
instrucao_devolver      = ("devolver" | "retorne"), [ expressao ] ;
instrucao_controle      = ("parar" | "encerrar" | "pare")
                        | ("seguir" | "prossiga")
                        | "passar"
                        | "ceder", [ expressao ] ;
instrucao_lancar        = "lancar", [ "erro" ], expressao ;
instrucao_verifique     = "verifique", expressao, [ ",", expressao ] ;
instrucao_escopo        = ("global" | "externa"), IDENTIFICADOR ;

instrucao_quando        = ("quando" | "se"), expressao_antes_bloco,
                          bloco,
                          { "sendo", expressao_antes_bloco, bloco },
                          [ "senao", (instrucao_quando | bloco) ] ;

instrucao_escolha       = ("escolha" | "avaliar"), expressao_antes_bloco,
                          ":", "{", { separador }, lista_casos_escolha,
                          { separador }, "}" ;
lista_casos_escolha     = caso_escolha, { separador, caso_escolha },
                          { separador } ;
caso_escolha            = ("caso", padrao_escolha | "padrao"),
                          bloco ;
padrao_escolha          = "_"
                        | caminho_variante, [ "(", lista_vinculos, ")" ]
                        | literal ;
caminho_variante        = IDENTIFICADOR, "::", IDENTIFICADOR ;
lista_vinculos          = IDENTIFICADOR, { ",", IDENTIFICADOR } ;

instrucao_enquanto      = ("enquanto" | "durante" | "repetir"),
                          expressao_antes_bloco, bloco ;
instrucao_cada          = "cada", IDENTIFICADOR, "em",
                          expressao_antes_bloco, bloco ;
instrucao_para          = "para", IDENTIFICADOR, "dentro",
                          expressao_antes_bloco, bloco
                        | "para", "(", [ inicializador ], ";",
                          [ expressao ], ";", [ expressao ], ")", bloco ;
inicializador           = ("variavel", IDENTIFICADOR, [ ":", tipo ], "=",
                          expressao) | expressao ;
instrucao_pulso         = "pulso", "(", expressao, ")", bloco ;

instrucao_tente         = "tente", bloco,
                          { "erro", [ IDENTIFICADOR ], bloco },
                          [ "sempre", bloco ] ;
instrucao_com           = "com", expressao_antes_bloco,
                          [ "como", IDENTIFICADOR ], bloco ;

expressao_antes_bloco   = expressao ;
~~~

Regras específicas:

- `quando` aceita `se` como alias. As cláusulas `sendo` repetem condições alternativas. `senao` é opcional e pode conter outro condicional. A chave `}` encerra a construção completa.
- `escolha` exige `: {`. Cada caso ocupa seu próprio Function Body, fechado por `}`; a escolha também fecha por `}`.
- Padrões aceitos: `Enum::Variante`, `Enum::Variante(vinculo, ...)`, `_`, `padrao` e literais. Padrões de enumeração e curinga duplicados são rejeitados. O curinga deve ser o último caso; casos posteriores são inalcançáveis e geram diagnóstico semântico.
- `tente` possui um corpo, zero ou mais cláusulas `erro` e uma cláusula `sempre` opcional. Deve existir pelo menos uma cláusula `erro` ou `sempre`. A chave `}` encerra a construção.
- `para nome dentro iteravel` é a forma iterativa. A forma tradicional usa três partes separadas por ponto e vírgula dentro de parênteses.
- `alterar` deve conter uma atribuição simples ou composta.
- A validade contextual de `devolver`, `parar`, `seguir` e `ceder` é semântica.

## 5. Tipos

~~~ebnf
tipo                    = tipo_funcao | tipo_lista | tipo_tupla | tipo_agrupado | tipo_generico ;
tipo_tupla              = "(", ")" | "(", tipo, ",", [ lista_tipos ], ")" ;
tipo_agrupado           = "(", tipo, ")" ;
tipo_funcao             = "funcao", "(", [ lista_tipos ], ")",
                          [ ":", tipo ] ;
tipo_lista              = "[", tipo, "]" ;
tipo_generico           = caminho_qualificado,
                          [ "<", lista_argumentos_tipo, ">" ] ;
lista_argumentos_tipo   = argumento_tipo, { ",", argumento_tipo } ;
argumento_tipo          = tipo | INTEIRO ;
caminho_qualificado     = IDENTIFICADOR, { "::", IDENTIFICADOR } ;
~~~

Tipos de função sem retorno explícito retornam `vazio`. Tipos compostos podem ser aninhados. Argumentos genéricos podem ser tipos ou inteiros constantes. A existência e compatibilidade dos tipos são verificadas semanticamente.

## 6. Expressões

O Parser usa precedência Pratt. A gramática abaixo representa a estrutura geral; a tabela da seção 7 define a precedência exata.

~~~ebnf
expressao               = expressao_prefixa, { operador_posfixo | operador_binario } ;
expressao_prefixa       = literal
                        | IDENTIFICADOR
                        | "isto" | "obter" | "biblioteca" | "modulo"
                        | "lambda", [ lista_identificadores ], ":", expressao
                        | "montagem", IDENTIFICADOR, [ "<", lista_tipos, ">" ],
                          "(", [ lista_argumentos ], ")"
                        | operador_unario, expressao
                        | "++", expressao | "--", expressao
                        | "(", [ expressao, { ",", expressao } ], ")"
                        | "[", [ lista_argumentos ], "]"
                        | construcao_estrutura ;

construcao_estrutura    = IDENTIFICADOR, "{", { separador },
                          [ campo_valor, { separador_membro, campo_valor },
                            [ separador_membro ] ], { separador }, "}" ;
campo_valor             = IDENTIFICADOR, ":", expressao ;

operador_posfixo        = chamada | chamada_generica | indexacao
                        | acesso_membro | incremento_posfixo
                        | operador_opcional ;
chamada                 = "(", [ lista_argumentos ], ")" ;
chamada_generica        = "<", lista_tipos, ">", "(", [ lista_argumentos ], ")" ;
lista_argumentos        = argumento, { ",", argumento }, [ "," ] ;
argumento               = [ IDENTIFICADOR, "=" ], expressao ;
indexacao               = "[", expressao, "]" ;
acesso_membro           = ("." | "::" | "?."),
                          IDENTIFICADOR ;
incremento_posfixo      = "++" | "--" ;
operador_opcional       = "?" ;
lista_identificadores   = IDENTIFICADOR, { ",", IDENTIFICADOR } ;
operador_unario         = "!" | "nao" | "~" | "&" | "..." | "-"
                        | "+" | "aguarda" | "novo" | "apagar" ;

literal                 = INTEIRO | DECIMAL | TEXTO | TEXTO_INTERPOLADO
                        | CARACTERE | "sim" | "verdadeiro" | "falso" | "nulo" ;
~~~

A AST de expressões inclui atribuições simples e compostas, operações binárias e unárias, chamadas posicionais e nomeadas, chamadas genéricas, acesso a membros, acesso opcional, indexação, listas, tuplas, construções de estruturas, lambdas, lambdas com Function Body, expressões condicionais e propagação opcional.

O operador `->` também introduz lambda: `x -> x * 2`, `(a, b) -> a + b` ou uma lambda de bloco com Function Body `: { ... }`. A palavra `lambda` introduz a forma `lambda x: expressão`.

Construções de estrutura usam chaves e campos `nome: expressão`; não são blocos de instrução. Campos podem conter expressões aninhadas. Campos duplicados são erro. Em cabeçalhos que aguardam bloco, uma construção vazia com chaves deve ser agrupada para não ser confundida com o início de bloco.

## 7. Operadores e precedência

Da maior para a menor:

| Nível | Operadores | Associatividade |
|---:|---|---|
| 16 | chamada, chamada genérica, indexação, acesso `.`, `::`, `?.`, pós-incremento/decremento | esquerda |
| 15 | `**` | direita |
| 14 | `!`, `nao`, `~`, `&`, `...`, unários `+` e `-`, `aguarda`, `novo`, `apagar`, pré-incremento/decremento | prefixa |
| 13 | `*`, `/`, `//`, `%` | esquerda |
| 12 | `+`, `-` | esquerda |
| 11 | `<<`, `>>`, `>>>` | esquerda |
| 10 | `..` | não encadeável |
| 9 | `<`, `<=`, `>`, `>=`, `dentro`, `fora`, `tipoDe`, `instanciaDe` | não encadeável para comparações relacionais |
| 8 | `==`, `!=`, `!==`, `iguais`, `identico` | esquerda |
| 7 | `&` binário | esquerda |
| 6 | `^` | esquerda |
| 5 | `|` | esquerda |
| 4 | `e`, `&&` | esquerda |
| 3 | `??` | direita |
| 2 | `ou`, `||` | esquerda |
| 1 | `=`, `+=`, `-=`, `*=`, `/=`, `//=`, `%=`, `**=`, `??=`, `->`, ternário `? :` | atribuições à direita |

Comparações relacionais não podem ser encadeadas sem agrupamento explícito. Operadores de intervalo também não podem ser encadeados. A atribuição e a potência associam à direita. Operadores compostos são representados na AST como atribuição cujo valor contém a operação correspondente.

## 8. Quebras de linha e separadores

1. Entre itens independentes no arquivo e dentro de blocos, DEVE existir ao menos um separador: quebra lógica de linha ou `;`.
2. Quebras repetidas, linhas vazias e pontos e vírgulas repetidos são aceitos entre itens.
3. Separador final é permitido antes de `}` e `EOF`.
4. Dentro de `()` e `[]`, quebras físicas são ignoradas como separadores e funcionam como espaço.
5. Dentro de `()` e `[]`, `;` continua token explícito e NÃO substitui vírgula de parâmetros, argumentos, listas, tuplas ou tipos.
6. Dentro de chaves usadas em construções de estrutura, quebras e `;` separam campos.
7. Depois de operador binário ou de atribuição que ainda exige operando, uma ou mais quebras continuam a expressão.
8. Antes de operador binário ou de atribuição que completa a expressão anterior, a quebra continua a expressão.
9. Uma quebra antes de identificador ou literal sem operador de continuação encerra o item anterior.
10. Chamadas, indexações e acessos a membro/escopo podem continuar na linha seguinte ao alvo.
11. Em acesso a membro, o identificador vem depois do operador; quebras também podem ocorrer entre operador e nome.
12. Caminhos de tipos e módulos aceitam quebra antes/depois de `::`; o padrão de escolha exige `Enum::Variante` sem quebra dentro do par.
13. Quebras após vírgulas continuam listas quando a construção permite a continuação.
14. Uma quebra imediatamente antes de `senao`, `sendo`, `erro` ou `sempre` mantém a cláusula associada à construção aberta. Ponto e vírgula não substitui essa regra.
15. A indentação organiza visualmente o conteúdo de Function Body e não cria limites sintáticos.
16. Cada corpo é encerrado exclusivamente pela chave `}` correspondente.

## 9. Delimitadores

- Parênteses, colchetes e chaves devem ser pareados e respeitar a ordem de aninhamento.
- `}` encerra o conteúdo de um Function Body.
- Chaves de dados delimitam expressões de agregados e não criam um Function Body.
- Fechamento cruzado, extra ou fora do contexto é erro sintático.
- O Parser diagnostica fechamento incompatível e tenta preservar os itens posteriores.
- Abertura sem fechamento no fim do arquivo é diagnosticada pelo Lexer/Parser.

## 10. Restrições sintáticas e diagnósticos

O Parser rejeita, entre outros casos:

- Bloco de instrução sem abertura `: {`.
- Bloco sem a chave `}` correspondente.
- Indentação inicial no escopo do arquivo ou nível de indentação sem bloco correspondente.
- Campo de estrutura sem dois-pontos ou tipo.
- Estrutura ou enumeração sem Function Body ou sem a chave de fechamento correspondente.
- Enumeração sem variantes.
- Campo, variante, parâmetro, parâmetro genérico ou vínculo duplicado.
- Padrão de escolha inválido, variante repetida ou curinga duplicado.
- Comparações relacionais e intervalos encadeados.
- `alterar` sem atribuição.
- Tokens reservados para recursos futuros quando usados como construção.
- Delimitadores incompatíveis, não fechados ou sem abertura.

O limite de aninhamento sintático é de **64 níveis**. Entradas que excedem o limite recebem `N2049`; o limite protege a implementação e não é regra de tipagem.

Os códigos e mensagens detalhados dos diagnósticos permanecem definidos no código do Parser e nos testes de conformidade. A análise semântica valida nomes, tipos, campos, mutabilidade, compatibilidade e contexto de controle.

## 11. Recursos reservados e limites

A presença de uma palavra reservada no Lexer não implica que o recurso esteja implementado. Palavras reservadas futuras são rejeitadas pelo Parser quando usadas como construção.

O Lexer reconhece tokens de texto interpolado, mas o tratamento semântico completo da interpolação depende da implementação correspondente. A gramática só aceita o token como literal indivisível; não define uma gramática interna de expressões interpoladas.

## 12. Exemplos normativos

### Função, condicional e laço

~~~nebula
funcao principal() -> vazio: {
    variavel ativo = verdadeiro
    variavel contador: inteiro = 0
    enquanto ativo: {
        quando contador < 3: {
            exibir(contador)
            contador += 1

        }
        sendo contador == 3: {
            exibir("Último ciclo")
            ativo = falso

        }
        senao: {
            passar

        }


    }

}

~~~

### Estrutura e enumeração

~~~nebula
estrutura Cabecalho: {
    codigo: u32
    tamanho: u16
    flags: u16

}

enumeracao Estado: {
    Inicializando
    Pronto
    Falha(inteiro)

}
~~~

### Escolha com padrões

~~~nebula
funcao tratar(estado: Estado) -> vazio: {
    escolha estado: {
        caso Estado::Inicializando: {
            exibir("Iniciando")

        }
        caso Estado::Pronto: {
            exibir("Pronto")

        }
        caso Estado::Falha(codigo): {
            exibir(codigo)

        }
        padrao: {
            passar

        }

    }

}

~~~

### Tratamento de erro e contexto de recurso

~~~nebula
funcao carregar() -> vazio: {
    tente: {
        variavel dados = lerArquivo("dados.bin")

    }
    erro ArquivoInvalido: {
        exibir("Arquivo inválido")

    }
    sempre: {
        exibir("Operação encerrada")

    }


    com abrir("dados.bin", "leitura") como arquivo: {
        exibir(arquivo)

    }

}
~~~

### Lambda e tipos compostos

~~~nebula
variavel dobrar = lambda valor: valor * 2
variavel somar = (a, b) -> a + b
variavel aplicar = valor ->: {
    quando valor > 0: {
        devolver valor

    }
    senao: {
        devolver 0

    }


}

funcao inverter(par: Tupla<inteiro64, inteiro64>) -> Tupla<inteiro64, inteiro64>: {
    devolver (par[1], par[0])

}
~~~

### Construção de estrutura (chaves de dados)

~~~nebula
variavel cabecalho = Cabecalho{
    codigo: 10,
    tamanho: 20,
    flags: 0,

}
~~~

### Código de baixo nível

~~~nebula
@representacao(C)
@alinhamento(8)
estrutura Registro: {
    codigo: u32
    tamanho: u16

}

@convencao(C)
externa funcao escreverByte(valor: u8) -> vazio

funcao lerTeclado() -> vazio: {
    variavel tecla = porta.ler8(0x60)
    critico: {
        passar

    }

}
~~~

### Expressão multilinha com continuação

~~~nebula
variavel resultado = valorInicial
    + primeiraEtapa
    + segundaEtapa
    + terceiraEtapa
~~~

A indentação adicional nesse exemplo é continuação da expressão, não um bloco de instrução. A expressão continua até ser concluída; não recebe terminador textual.

## 13. Conformidade

Uma implementação conforme deve:

1. Preservar os tokens `:`, `{` e `}` com suas localizações.
2. Exigir `: {` para todo Function Body.
3. Exigir uma chave `}` correspondente para cada abertura `{`.
4. Rejeitar a sintaxe antiga baseada somente em indentação.
5. Distinguir Function Body de chaves usadas em valores de dados.
6. Preservar as regras de separação e continuação da seção 8.
7. Produzir AST e diagnósticos com limites de bloco determinados pelos delimitadores, não apenas pela indentação.
8. Manter esta gramática, `tokens.md`, exemplos e testes de conformidade sincronizados.
