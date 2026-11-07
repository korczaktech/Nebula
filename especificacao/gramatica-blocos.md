# Gramática de blocos Nebula

Esta gramática define a delimitação dos corpos executáveis e estruturais. Ela substitui integralmente qualquer regra anterior que exigia a palavra reservada `fim`.

## Terminais

```ebnf
ABRE_CORPO  = ":" , "{" ;
FECHA_CORPO = "}" ;
NOVA_LINHA  = ? quebra de linha reconhecida pelo Lexer ? ;
```

## Produções

```ebnf
corpo              = ABRE_CORPO , separadores , { item , separadores } , FECHA_CORPO ;
corpo_funcao       = corpo ;
corpo_condicional  = corpo ;
corpo_laco         = corpo ;
corpo_escolha      = corpo ;
corpo_classe       = corpo ;
corpo_estrutura    = corpo ;
corpo_tratamento   = corpo ;
corpo_exportacao   = corpo ;
separadores        = { NOVA_LINHA | ";" } ;
```

A produção `item` é definida pelas regras de declarações e instruções da linguagem. Corpos aninhados usam a mesma produção `corpo`, portanto cada abertura `{` exige exatamente uma chave `}` correspondente.

## Regras obrigatórias

- `fim` não é palavra reservada nem token de fechamento.
- O Lexer classifica a sequência de caracteres `fim` como identificador comum.
- O Parser encerra um corpo exclusivamente ao consumir sua chave `}` correspondente.
- A indentação deve refletir o aninhamento das chaves e é validada separadamente.
- Chaves de expressões de objetos e agregados continuam sendo delimitadores de expressão; não recebem terminadores textuais.
- Uma chave ausente, extra ou incompatível gera diagnóstico sintático.
- Não há fechamento implícito de corpo no fim do arquivo.

## Exemplo normativo

```nbx
funcao principal(): {
    se verdadeiro: {
        exibir("Nebula")
    }
    senao: {
        exibir("Indisponivel")
    }
}
```

## Rejeição da forma anterior

O antigo marcador `fim` não fecha corpos. Uma ocorrência isolada em posição de instrução é analisada como identificador e, se não formar uma expressão ou declaração válida, recebe o diagnóstico sintático correspondente.
