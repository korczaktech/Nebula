# Estado dos itens 9 a 11 da Fase 5

Este registro apresenta entregas verificáveis. Não substitui os critérios integrais de aceite da Fase 5.

## Item 9 — Ferramentas

**Estado: parcial.**

Implementado:
- CLI principal: `ajuda`, `versao`, `criar`, `compilar`, `executar`, `testar`, `verificar`, `analisar`, `ir`, `interpretar`, `formatar`, `dependencias resolver`, `dependencias verificar`, `limpar` e `limpar-cache`.
- `criar <diretorio> [--nome <nome>]` gera `nebula.toml` e `principal.nbx` sem substituir diretórios existentes.
- `limpar <arquivo.nbx>` remove somente o `.out` padrão quando o cabeçalho corresponde a ELF64 little-endian x86-64; rejeita links simbólicos e preserva arquivos que não sejam executáveis ELF desse formato.
- Formatador integrado na CLI principal e disponível como binário separado. Valida a sintaxe, remove espaços finais, padroniza a quebra de linha final, oferece `--check` e grava por arquivo temporário.
- Testes de integração para criação, não sobrescrita, compilação/limpeza de artefato e preservação de `.out` que não seja ELF.

Pendente:
- Formatador canônico completo (indentação e regras de estilo), round-trip integral e fixtures de todos os construtos.
- Cache incremental de artefatos ELF com SHA-256, invalidado por fonte principal, módulos locais recursivos, versão/fingerprint do compilador, alvo e nível de otimização. Ainda falta cache incremental por unidade de módulo/IR e invalidação fina.
- Alvos, diagnósticos estruturados e opções completas de build.
- Caminhos de I/O, permissões, arquivos grandes e falhas multiplataforma para todos os comandos.

## Item 10 — Qualidade e segurança

**Estado: parcial.**

Implementado:
- CI para `cargo fmt`, Clippy com `-D warnings`, testes do workspace, `cargo doc` e benchmark.
- Workflow de `cargo audit` por push/PR e agenda.
- Dependabot semanal para Cargo e GitHub Actions.
- Fuzz smoke determinístico do Lexer/Parser para entradas malformadas.
- Testes de travessia de diretórios e symlinks na resolução de módulos, cache e lockfile.
- Guards de empréstimo RAII no runtime: múltiplos leitores ou um escritor exclusivo por identificador, com liberação automática no descarte.
- Restrições SemVer locais (`*`, `^`, `~`, comparadores e conjunções) associadas às dependências do manifesto.
- Crate `nebula-seguranca` com SHA-256, HMAC-SHA-256 e comparação de bytes sem encerramento antecipado pelo conteúdo; vetores de teste conhecidos.
- Parser e serializer JSON com testes de entradas inválidas, chaves duplicadas, Unicode e limites.

Pendente:
- Campanhas contínuas de fuzzing com corpus/cobertura para pacotes, JSON, lockfiles e interfaces críticas. O workflow atual executa apenas fuzz smoke determinístico com entradas limitadas.
- Auditoria independente de código, criptografia, supply chain e segurança.
- Matriz de execução do backend para alvos adicionais, stress de memória e concorrência e baseline de benchmarks representativos. A matriz atual valida o workspace Rust em Linux/Windows/macOS, não executáveis Nebula nesses três sistemas.

## Item 11 — Documentação e lançamento

**Estado: parcial.**

Implementado:
- Especificação, critérios da Fase 5, documentos de estabilidade, ferramentas, segurança, lançamento e biblioteca padrão.
- Política de relato responsável e índice da documentação.
- Workflow de release restrito ao Linux x86-64 com validação, checksums, atestação de proveniência Sigstore/GitHub e bloqueio de publicação se faltar licença.
- CI ampliada para executar fmt, Clippy, testes e documentação em runners Ubuntu, Windows e macOS; o backend nativo declarado continua sendo apenas Linux x86-64.
- Arquivo `LICENSE` e documento de situação da licença.

Pendente:
- Revisão jurídica e confirmação formal da licença e da titularidade.
- Assinatura destacada/verificável dos executáveis e processo de gestão de chaves organizacionais; a atestação de proveniência já é assinada via Sigstore.
- Instaladores e pacotes testados para cada plataforma suportada.
- Guia de migração completo e referências finais da linguagem, runtime, biblioteca, pacotes e backends.
- Congelamento e auditoria dos contratos 1.0.

## Validação

A CI verde mais recente que cobre o código da biblioteca e os módulos de segurança foi executada no commit `5267dfaa093cb4c12f9e9b54d45f37e1bdc63119`. Os comandos e recursos de cache, restrições SemVer e guards de empréstimo foram adicionados depois das execuções anteriores e precisam de aprovação na execução de CI do HEAD atual. Uma CI aprovada valida somente os jobs e testes executados; não conclui os itens completos nem a Fase 5.
