# Gramática normativa da Nebula

**Estado:** normativa e alinhada ao Lexer, Parser e AST atualmente implementados em `compilador/sintaxe`.
**Regra central:** blocos de instrução usam indentação explícita e terminam com `fim`. Chaves NÃO substituem blocos indentados. Chaves continuam válidas para construções de dados, como valores de estruturas, e para delimitação de contextos que não sejam blocos de instrução.

Esta gramática descreve a sintaxe reconhecida. Resolução de nomes, compatibilidade de tipos, mutabilidade, ABI, validade de operações e regras dependentes do contexto pertencem às fases semânticas.

## 1. Notação e tokens estruturais

- `=` define uma produção; `|` representa alternativas.
- `[ ... ]` indica elemento opcional; `{ ... }` indica zero ou mais repetições.
- Terminais aparecem entre aspas.
- Nomes em maiúsculas representam categorias de tokens produzidas pelo Lexer.
- `EOF` é o fim do arquivo.
- `NOVA_LINHA` é uma quebra lógica de linha preservada.
- `INDENT` inicia um nível de indentação; `DEDENT` encerra um ou mais níveis.
- `FIM_BLOCO` é a palavra-chave `fim`.
- `separador` é `NOVA_LINHA` ou `;`.

O Lexer emite `INDENT` e `DEDENT` a partir das colunas de indentação. Eles são tokens estruturais, não caracteres que precisem ser escritos no arquivo.

## 2. Regra normativa de indentação

### 2.1 Princípios obrigatórios

1. Todo bloco de instruções DEVE começar em uma nova linha e em um nível de indentação maior que o nível do cabeçalho.
2. Todo bloco de instruções DEVE terminar com `fim`, depois que seu nível de indentação for encerrado.
3. O corpo de um bloco DEVE manter um nível de indentação consistente. Blocos aninhados DEVEM aumentar a indentação e encerrar esse nível antes do `fim` do bloco externo.
4. O nível-base do arquivo é a coluna zero. Um item no escopo do arquivo NÃO DEVE começar indentado.
5. A indentação usa espaços. Tabulações são diagnosticadas como ambíguas; código normativo DEVE usar espaços.
6. Linhas vazias e linhas contendo apenas comentários NÃO alteram a pilha de indentação.
7. A indentação não pode ser usada para fechar implicitamente um bloco: o `DEDENT` fecha o nível visual, mas o `fim` explícito fecha a construção sintática.
8. Chaves NÃO substituem a indentação de funções, condicionais, laços, classes, contextos ou outros blocos de instrução.
9. Estruturas e enumerações também usam cabeçalho terminado em dois-pontos, corpo indentado, `DEDENT` e `fim`.
10. O Parser rejeita indentação inesperada, ausência de indentação, ausência de `fim` e delimitadores de bloco incompatíveis.

### 2.2 Formação dos níveis

- O Lexer inicia com uma pilha de colunas contendo apenas a coluna zero.
- Ao encontrar uma linha com código fora de delimitadores, compara a coluna inicial com o topo da pilha.
- Coluna maior: empilha a coluna e emite um `INDENT`.
- Coluna igual: não emite token de indentação.
- Coluna menor: desempilha e emite um `DEDENT` para cada nível encerrado.
- A coluna resultante DEVE corresponder a um nível anterior. Se não corresponder, o Lexer emite diagnóstico de indentação inconsistente.
- Ao chegar ao fim do arquivo, o Lexer emite os `DEDENT` restantes antes de `EOF`.
- O primeiro nível de código não pode começar em coluna maior que zero.

A indentação é medida em colunas de espaços. Tabulação é diagnosticada; sua largura interna de recuperação não constitui estilo permitido pela linguagem.

### 2.3 Delimitadores e continuação de linha

- Dentro de parênteses `()` e colchetes `[]`, quebras físicas são continuação de linha e não geram separadores de instrução nem níveis de indentação.
- Enquanto qualquer delimitador estiver aberto, o Lexer suprime a geração de tokens de indentação. Isso inclui chaves usadas em construções de dados.
- Dentro de chaves, quebras de linha permanecem disponíveis como separadores, mas a indentação não cria blocos.
- Uma expressão pode continuar após operador binário, atribuição, vírgula, ponto, `::`, chamada ou indexação conforme as regras da seção 8.
- Uma quebra usada para continuar expressão não abre bloco por si só. A indentação extra de continuação é consumida como continuação e não pode ser confundida com um bloco de instrução.

### 2.4 Forma canônica de um bloco

~~~ebnf
bloco                   = [ ":" ], NOVA_LINHA, { separador },
                          INDENT, lista_itens, DEDENT, "fim" ;
bloco_sem_fim           = [ ":" ], NOVA_LINHA, { separador },
                          INDENT, lista_itens, DEDENT ;
lista_itens             = [ item, { separador, item }, { separador } ] ;
separador               = NOVA_LINHA | ";" ;
~~~

`bloco_sem_fim` é uma regra interna para construções com cláusulas encadeadas, como `quando/sendo/senao`, `escolha/caso` e `tente/erro/sempre`. A construção externa consome um único `fim` depois de todas as cláusulas. Não significa que o programa possa omitir o terminador.

Exemplo normativo:

~~~nebula
funcao principal() -> vazio
    variavel pronto = verdadeiro

    quando pronto
        exibir("Iniciado")
    senao
        exibir("Aguardando")
    fim
fim
~~~

O `fim` mais interno encerra o condicional; o último encerra a função.

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

declaracao_funcao       = [ "assinc" ], { modificador_funcao },
                          "funcao", nome_funcao,
                          [ "<", lista_genericos, ">" ],
                          "(", [ lista_parametros ], ")",
                          [ retorno_funcao ], bloco ;
nome_funcao             = IDENTIFICADOR | "construtor" ;
modificador_funcao      = "publico" | "privado" | "protegido" | "estatico" ;
retorno_funcao          = "->", tipo
                        | ":", tipo ;
lista_genericos         = IDENTIFICADOR, { ",", IDENTIFICADOR } ;
lista_parametros        = parametro, { ",", parametro }, [ "," ] ;
parametro               = IDENTIFICADOR, ":", tipo, [ "=", expressao ] ;

declaracao_classe       = "classe", IDENTIFICADOR,
                          [ "herda", IDENTIFICADOR ], bloco ;
declaracao_tipo         = "tipo", IDENTIFICADOR, "=", tipo ;

declaracao_estrutura    = "estrutura", IDENTIFICADOR, ":",
                          NOVA_LINHA, { separador }, INDENT,
                          lista_campos_estrutura, DEDENT, "fim" ;
lista_campos_estrutura  = [ campo_estrutura,
                          { separador_membro, campo_estrutura },
                          [ separador_membro ] ] ;
campo_estrutura         = IDENTIFICADOR, ":", tipo ;

declaracao_enumeracao   = "enumeracao", IDENTIFICADOR, ":",
                          NOVA_LINHA, { separador }, INDENT,
                          variante, { separador_variante, variante },
                          [ separador_variante ], DEDENT, "fim" ;
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
- A forma `: tipo` após os parâmetros declara retorno quando existe um tipo na mesma linha; `: NOVA_LINHA` introduz o corpo indentado sem declarar retorno.
- `externa funcao ...` declara função externa sem corpo. `global nome` e `externa nome` declaram referências de escopo.
- `exportar funcao ...` equivale a uma declaração de função pública.
- Modificadores de função são aceitos antes de `funcao`; `assinc` marca a função como assíncrona.
- `estrutura` exige dois-pontos, quebra de linha, indentação e `fim`. Campos exigem nome, dois-pontos e tipo. Campos repetidos geram diagnóstico.
- `enumeracao` exige ao menos uma variante. Uma variante pode não ter payload, usar payload vazio `()` ou conter uma lista de tipos. Variantes repetidas são erro.
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
                          bloco_sem_fim,
                          { "sendo", expressao_antes_bloco, bloco_sem_fim },
                          [ "senao", (instrucao_quando | bloco_sem_fim) ],
                          "fim" ;

instrucao_escolha       = ("escolha" | "avaliar"), expressao_antes_bloco,
                          ":", NOVA_LINHA, { separador }, INDENT,
                          lista_casos_escolha, DEDENT, "fim" ;
lista_casos_escolha     = caso_escolha, { separador, caso_escolha },
                          { separador } ;
caso_escolha            = ("caso", padrao_escolha | "padrao"),
                          bloco_sem_fim ;
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

instrucao_tente         = "tente", bloco_sem_fim,
                          { "erro", [ IDENTIFICADOR ], bloco_sem_fim },
                          [ "sempre", bloco_sem_fim ], "fim" ;
instrucao_com           = "com", expressao_antes_bloco,
                          [ "como", IDENTIFICADOR ], bloco ;

expressao_antes_bloco   = expressao ;
~~~

Regras específicas:

- `quando` aceita `se` como alias. As cláusulas `sendo` repetem condições alternativas. `senao` é opcional e pode conter outro condicional. Um único `fim` encerra a construção completa.
- `escolha` exige dois-pontos, nova linha e indentação. Cada caso ocupa seu próprio bloco indentado; `caso` recebe padrão e `padrao` representa o caso curinga. Um único `fim` encerra a escolha.
- Padrões aceitos: `Enum::Variante`, `Enum::Variante(vinculo, ...)`, `_`, `padrao` e literais. Padrões de enumeração e curinga duplicados são rejeitados; o curinga pode aparecer antes ou depois de variantes.
- `tente` possui um corpo, zero ou mais cláusulas `erro` e uma cláusula `sempre` opcional. Deve existir pelo menos uma cláusula `erro` ou `sempre`. Um único `fim` encerra a construção.
- `para nome dentro iteravel` é a forma iterativa. A forma tradicional usa três partes separadas por ponto e vírgula dentro de parênteses.
- `alterar` deve conter uma atribuição simples ou composta.
- A validade contextual de `devolver`, `parar`, `seguir` e `ceder` é semântica.

## 5. Tipos

~~~ebnf
tipo                    = tipo_funcao | tipo_lista | tipo_generico ;
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

A AST de expressões inclui atribuições simples e compostas, operações binárias e unárias, chamadas posicionais e nomeadas, chamadas genéricas, acesso a membros, acesso opcional, indexação, listas, tuplas, construções de estruturas, lambdas, lambdas com bloco, expressões condicionais e propagação opcional.

O operador `->` também introduz lambda: `x -> x * 2`, `(a, b) -> a + b` ou uma lambda de bloco com corpo indentado e `fim`. A palavra `lambda` introduz a forma `lambda x: expressão`.

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
3. Separador final é permitido antes de `DEDENT`, `fim` e `EOF`.
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
15. A indentação só abre níveis quando o Lexer está fora de delimitadores e não está processando continuação de expressão.
16. O `DEDENT` deve encerrar o corpo antes do `fim` correspondente.

## 9. Delimitadores

- Parênteses, colchetes e chaves devem ser pareados e respeitar a ordem de aninhamento.
- `fim` fecha somente uma construção de bloco indentado; não fecha parênteses, colchetes ou chaves.
- Chaves de construção de estrutura fecham com `}`; não recebem `fim`.
- Fechamento cruzado, extra ou fora do contexto é erro sintático.
- O Parser diagnostica fechamento incompatível e tenta preservar os itens posteriores.
- Abertura sem fechamento no fim do arquivo é diagnosticada pelo Lexer/Parser.

## 10. Restrições sintáticas e diagnósticos

O Parser rejeita, entre outros casos:

- Bloco de instrução delimitado por chaves em vez de indentação e `fim`.
- Bloco sem nova linha, sem `INDENT`, sem `DEDENT` ou sem `fim`.
- Indentação inicial no escopo do arquivo ou nível de indentação sem bloco correspondente.
- Campo de estrutura sem dois-pontos ou tipo.
- Estrutura ou enumeração usando chaves como corpo.
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
funcao principal() -> vazio
    variavel ativo = verdadeiro
    variavel contador: inteiro = 0

    enquanto ativo
        quando contador < 3
            exibir(contador)
            contador += 1
        sendo contador == 3
            exibir("Último ciclo")
            ativo = falso
        senao
            passar
        fim
    fim
fim
~~~

### Estrutura e enumeração

~~~nebula
estrutura Cabecalho:
    codigo: u32
    tamanho: u16
    flags: u16
fim

enumeracao Estado:
    Inicializando
    Pronto
    Falha(inteiro)
fim
~~~

### Escolha com padrões

~~~nebula
funcao tratar(estado: Estado) -> vazio
    escolha estado:
        caso Estado::Inicializando
            exibir("Iniciando")
        caso Estado::Pronto
            exibir("Pronto")
        caso Estado::Falha(codigo)
            exibir(codigo)
        padrao
            passar
    fim
fim
~~~

### Tratamento de erro e contexto de recurso

~~~nebula
funcao carregar() -> vazio
    tente
        variavel dados = lerArquivo("dados.bin")
    erro ArquivoInvalido
        exibir("Arquivo inválido")
    sempre
        exibir("Operação encerrada")
    fim

    com abrir("dados.bin", "leitura") como arquivo
        exibir(arquivo)
    fim
fim
~~~

### Lambda e tipos compostos

~~~nebula
variavel dobrar = lambda valor: valor * 2
variavel somar = (a, b) -> a + b
variavel aplicar = valor -> 
    quando valor > 0
        devolver valor
    senao
        devolver 0
    fim
fim

funcao inverter(par: Tupla<inteiro64, inteiro64>) -> Tupla<inteiro64, inteiro64>
    devolver (par[1], par[0])
fim
~~~

### Construção de estrutura (chaves são dados, não bloco)

~~~nebula
variavel cabecalho = Cabecalho{
    codigo: 10,
    tamanho: 20,
    flags: 0
}
~~~

### Código de baixo nível

~~~nebula
@representacao(C)
@alinhamento(8)
estrutura Registro:
    codigo: u32
    tamanho: u16
fim

@convencao(C)
externa funcao escreverByte(valor: u8) -> vazio

funcao lerTeclado() -> vazio
    variavel tecla = porta.ler8(0x60)
    critico:
        passar
    fim
fim
~~~

### Expressão multilinha com continuação

~~~nebula
variavel resultado = valorInicial
    + primeiraEtapa
    + segundaEtapa
    + terceiraEtapa
~~~

A indentação adicional nesse exemplo é continuação da expressão, não um bloco de instrução. A expressão continua até ser concluída; não recebe `fim`.

## 13. Conformidade

Uma implementação conforme deve:

1. Gerar tokens `INDENT` e `DEDENT` segundo a seção 2.
2. Exigir indentação para todo bloco de instrução.
3. Exigir `fim` para toda construção de bloco que o Parser modela como bloco completo.
4. Rejeitar chaves como substitutas de blocos de instrução.
5. Aceitar chaves em construções de dados e agrupamentos previstos.
6. Preservar as regras de separação e continuação da seção 8.
7. Produzir AST e diagnósticos sem depender da indentação como informação semântica depois que os limites do bloco foram reconhecidos.
8. Manter esta gramática, `tokens.md`, exemplos e testes de conformidade sincronizados.
