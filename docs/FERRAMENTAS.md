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

## Cache incremental

A compilação nativa mantém entradas em `.nebula/cache`. A chave SHA-256 inclui os bytes da fonte principal, fontes de módulos carregados recursivamente, versão do compilador, alvo e nível de otimização. Cada entrada armazena checksum do artefato; conteúdo corrompido é descartado e recompilado. `nebula limpar-cache <projeto>` remove os arquivos armazenados sem apagar as fontes. Falha ou indisponibilidade do cache não bloqueia uma compilação normal.
