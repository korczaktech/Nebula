# Ferramentas da Nebula

## CLI

O executável principal é `nebula`. Os comandos implementados são documentados pela própria ajuda (`nebula ajuda`). Um comando não deve ser anunciado como funcional se ainda não estiver ligado a uma implementação real.

## Formatador conservador

O binário `nebula-formatar` valida a sintaxe antes de alterar o arquivo. Nesta etapa ele normaliza espaços em branco no fim das linhas, remove linhas vazias finais e garante uma única quebra de linha final. Não reindenta blocos nem reescreve tokens; essa escolha evita alterar código enquanto a gramática e o estilo canônico ainda evoluem.

Uso:

```sh
nebula-formatar programa.nbx
nebula-formatar programa.nbx --check
```

O modo `--check` retorna código 1 se houver alterações pendentes e não grava o arquivo. A gravação normal usa arquivo temporário no mesmo diretório e substituição atômica. O formatador não substitui a revisão da gramática nem é apresentado como formatador estilístico completo.

## Cache

O cache incremental do compilador ainda não está disponível. Até sua implementação, os builds são completos; não há cache oculto nem promessa de invalidação incremental.
