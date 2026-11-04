# Diagnósticos da Nebula

## Modelo de dados

O diagnóstico-base, independente da interface, contém:
- Código estável.
- Severidade: erro, aviso ou informação.
- Mensagem em português.
- Intervalo de origem opcional, com offsets em bytes.

A estrutura inicial implementada em nebula-diagnosticos contém exatamente esses campos. O caminho do arquivo e a conversão do intervalo para linha/coluna pertencem ao contexto do arquivo-fonte e à camada de apresentação; não são campos duplicados obrigatoriamente em cada valor de diagnóstico.

## Apresentação

Quando houver origem associada, a apresentação humana deve incluir:
- Arquivo ou unidade de origem.
- Linha e coluna, base 1.
- Trecho destacado quando disponível.
- Sugestão de correção somente quando confiável.

Diagnósticos sintéticos ou globais podem não possuir intervalo. Nesses casos, a apresentação deve indicar que a localização não está disponível, sem inventar uma posição.

## Identificadores

Códigos usam faixas por subsistema: N0001 para falhas de leitura; N1000–N1099 para entrada UTF-8 e Lexer; N2000–N2099 para Parser; N3000–N3099 para semântica e tipos; N4000–N4099 para memória e concorrência; N5000–N5099 para backend e ligação.

## Diagnósticos sintáticos

Os códigos `N2000`–`N2099` pertencem ao Parser. O código é estável e a mensagem deve identificar a construção esperada ou a regra sintática violada.

| Código | Significado |
|---|---|
| N2001 | Separador ausente entre instruções ou itens de bloco |
| N2002 | Modificador de visibilidade sem função |
| N2003 | Operador de atribuição ausente em declaração |
| N2004 | Parêntese de abertura ausente na função |
| N2005 | Parâmetro sem anotação de tipo ou separador inválido |
| N2006 | Atribuição ausente em alias de tipo |
| N2007 | Bloco ausente ou malformado |
| N2008 | Expressão ausente ou inválida |
| N2009 | Token delimitador ou estrutural esperado não encontrado |
| N2010 | Nome/identificador esperado |
| N2011 | Palavra reservada futura usada em posição inválida |
| N2012 | Comparação relacional encadeada |
| N2013 | Operador de intervalo encadeado |
| N2014 | Parâmetro genérico duplicado |
| N2015 | Lista genérica vazia ou parâmetro genérico ausente |
| N2016 | Construção reservada para versão futura |
| N2030 | Chave de abertura ausente na estrutura |
| N2031 | Campo duplicado na estrutura |
| N2032 | Separador ausente entre campos da estrutura |
| N2033 | Chave de abertura ausente na enumeração |
| N2034 | Variante duplicada na enumeração |
| N2035 | Separador ou tipo de payload ausente na enumeração |
| N2036 | Enumeração sem variantes |
| N2037 | Campo duplicado na construção de estrutura |
| N2038 | Separador ausente entre campos da construção |
| N2040 | Chave de abertura ausente em escolha |
| N2041 | Caso ausente em escolha |
| N2042 | Padrão de escolha inválido |
| N2043 | Caso curinga duplicado |
| N2044 | Escolha sem casos |
| N2045 | Vínculo de payload ausente ou inválido |
| N2046 | Vínculo duplicado no payload do padrão |
| N2047 | Alteração sem expressão de atribuição |
| N2048 | Padrão de enumeração duplicado em escolha |
| N2049 | Limite de aninhamento sintático excedido |
| N2050 | Parâmetro de função duplicado |
| N2051 | Delimitador de fechamento incompatível, sem abertura ou fora do bloco esperado |

## Garantias dos diagnósticos sintáticos

- Os diagnósticos léxicos e sintáticos são preservados em uma única lista.
- A lista final é ordenada pela posição inicial no arquivo-fonte, independentemente da etapa que produziu o diagnóstico; diagnósticos sem intervalo ficam ao final.
- Todo erro sintático associado a um token aponta para o intervalo desse token, com offsets em bytes, início inclusivo e fim exclusivo.
- Quando um token esperado está ausente no fim do arquivo, o diagnóstico usa um intervalo vazio na posição final da fonte. Isso permite que editores apresentem o cursor exatamente onde a construção deveria continuar.
- Intervalos devem respeitar limites UTF-8 e permanecer dentro do conteúdo da fonte.
- Fechamentos `)`, `]`, `}` e `fim` devem corresponder ao agrupamento ou bloco aberto; um fechamento incompatível ou sem abertura gera `N2051`, enquanto uma abertura sem fechamento gera `N2009`.
- A recuperação após um fechamento cruzado descarta apenas o agrupamento interno inválido e restabelece a emissão de quebras de linha quando o delimitador externo correspondente é fechado.
- Códigos são estáveis e mensagens são em português, específicas à regra violada ou à construção esperada.
- O Parser não descarta diagnósticos léxicos ao recuperar erros sintáticos; ambos permanecem disponíveis para a ferramenta chamadora.

Essas garantias são verificadas por testes de regressão que cobrem códigos documentados, severidade, mensagem, limites de intervalos, UTF-8, posição de EOF, ordenação combinada entre Lexer e Parser, pares de delimitadores, blocos aninhados e recuperação após fechamentos incompatíveis.

## Exemplo de apresentação futura

~~~text
Erro N1003: caractere inesperado.
Arquivo: exemplo.nbx
Linha: 4, coluna: 9
~~~

Mensagens não devem depender da localização do sistema operacional. A CLI poderá oferecer formato humano e formato estruturado para ferramentas. A recuperação sintática inicial é implementada na Fase 2. Formatação detalhada, sugestões e saída estruturada poderão evoluir nas fases seguintes.


## Diagnósticos semânticos implementados na Fase 3

| Código | Significado |
|---|---|
| N3001 | Função duplicada |
| N3002 | Alias de tipo duplicado |
| N3003 | Alias de tipo inválido ou cíclico |
| N3004 | Módulo não disponibilizado ao analisador |
| N3005 | Retorno fora de função |
| N3006 | Controle de laço fora de laço |
| N3007 | Nome não declarado |
| N3008 | Operador unário incompatível |
| N3009 | Tentativa de alterar constante |
| N3010 | Destino de atribuição inválido |
| N3011 | Função qualificada não resolvida |
| N3012 | Alvo de chamada inválido |
| N3013 | Indexação aplicada a valor que não é lista |
| N3014 | Acesso a membro não definido no subconjunto |
| N3015 | Quantidade incorreta de argumentos |
| N3016–N3018 | Operandos incompatíveis |
| N3019 | Nome duplicado ou colisão entre namespaces de valor, função e módulo |
| N3020 | Tipos incompatíveis |
| N3021 | Tipo não declarado |
| N3022 | Alias de módulo duplicado |
| N3023 | Função aninhada não permitida |
| N3024 | Função pode terminar sem devolver valor |
| N3025 | Alias de tipo fora do escopo de arquivo |
| N3026 | Inclusão de módulo fora do escopo de arquivo |
| N3027 | Lista vazia sem contexto para inferência de tipo |
| N3028 | Literal numérico fora do intervalo representável |

Os diagnósticos semânticos são globais nesta fase e podem não conter intervalo de origem, pois os nós da AST inicial ainda não carregam spans próprios. A localização exata por nó é uma melhoria planejada, não uma garantia atual.
