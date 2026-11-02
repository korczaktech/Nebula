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
