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

Códigos usam prefixos por subsistema: N0xxx para fonte/lexer/parser, N1xxx para semântica/tipos, N2xxx para memória/concorrência e N3xxx para backend/ligação.

## Exemplo de apresentação futura

~~~text
Erro N0001: caractere inesperado.
Arquivo: exemplo.nbx
Linha: 4, coluna: 9
~~~

Mensagens não devem depender da localização do sistema operacional. A CLI poderá oferecer formato humano e formato estruturado para ferramentas. A formatação completa e a recuperação de erros serão desenvolvidas nas fases de frontend.
