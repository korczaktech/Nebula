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

Os diagnósticos semânticos são globais nesta fase e podem não conter intervalo de origem, pois os nós da AST inicial ainda não carregam spans próprios. A localização exata por nó é uma melhoria planejada, não uma garantia atual.
