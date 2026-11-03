# Ferramentas da Nebula

## CLI

O executável principal é `nebula`. Os comandos implementados são documentados pela própria ajuda (`nebula ajuda`). Um comando não deve ser anunciado como funcional se ainda não estiver ligado a uma implementação real.

## Formatador conservador

O binário `nebula-formatar` valida a sintaxe antes de alterar o arquivo. Ele aplica indentação de quatro espaços por nível de bloco, normaliza espaços finais, remove linhas vazias finais e garante uma única quebra de linha final. O scanner ignora chaves dentro de strings e comentários e preserva o texto dos tokens; a formatação é determinística e idempotente para o subconjunto léxico validado. Não reordena declarações, não reescreve expressões e não tenta corrigir sintaxe inválida.

Uso:

```sh
nebula-formatar programa.nbx
nebula-formatar programa.nbx --check
```

O modo `--check` retorna código 1 se houver alterações pendentes e não grava o arquivo. A gravação normal usa arquivo temporário no mesmo diretório e substituição atômica. O formatador não substitui a revisão da gramática nem é apresentado como formatador estilístico completo.

## Cache incremental

A compilação nativa mantém entradas em `.nebula/cache`. A chave SHA-256 inclui os bytes da fonte principal, fontes de módulos carregados recursivamente, versão do compilador, alvo e nível de otimização. Cada entrada armazena checksum do artefato; conteúdo corrompido é descartado e recompilado. `nebula limpar-cache <projeto>` remove os arquivos armazenados sem apagar as fontes. `nebula dependencias resolver <projeto>` atualiza o lockfile por substituição atômica; `nebula dependencias verificar <projeto>` recalcula o grafo e falha quando o lockfile não corresponde ao estado dos manifestos. Falha ou indisponibilidade do cache não bloqueia uma compilação normal.

## Diagnósticos estruturados e alvos

- `nebula verificar arquivo.nbx --json` retorna o resultado sintático em JSON.
- `nebula analisar arquivo.nbx --json` retorna diagnósticos sintáticos e semânticos em JSON.
- Cada diagnóstico contém código estável, severidade, mensagem, intervalo em bytes e linha/coluna quando a fonte está disponível.
- `nebula alvos` lista somente os alvos com backend implementado.
- `nebula compilar arquivo.nbx --alvo linux-x86_64` seleciona explicitamente o backend ELF64 Linux. Alvos ainda não implementados são rejeitados antes da geração do artefato.
