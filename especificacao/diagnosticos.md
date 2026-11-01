# Diagnósticos da Nebula

## Formato

Todo diagnóstico deve conter:

- Código estável.
- Severidade: erro, aviso ou informação.
- Mensagem em português.
- Arquivo e intervalo de origem.
- Linha e coluna.
- Trecho destacado quando disponível.
- Sugestão de correção quando confiável.

## Identificadores

Códigos usam prefixos por subsistema: `N0xxx` para fonte/lexer/parser, `N1xxx` para semântica/tipos, `N2xxx` para memória/concorrência e `N3xxx` para backend/ligação.

## Exemplo

```text
Erro N0001: caractere inesperado.
Arquivo: exemplo.nbx
Linha: 4, coluna: 9
```

Mensagens não devem depender da localização do sistema operacional. A CLI poderá oferecer formato humano e formato estruturado para ferramentas.
