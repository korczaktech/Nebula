# Estado dos itens 9 a 11 da Fase 5

Este registro apresenta entregas verificáveis. Não substitui os critérios integrais de aceite da Fase 5.

## Item 9 — Ferramentas

**Estado: parcial.**

Implementado:
- CLI principal: `ajuda`, `versao`, `alvos`, `criar`, `compilar`, `executar`, `testar`, `verificar`, `analisar`, `ir`, `interpretar`, `formatar`, `dependencias resolver`, `dependencias verificar`, `limpar` e `limpar-cache`.
- `criar <diretorio> [--nome <nome>]` gera `nebula.toml` e `principal.nbx` sem substituir diretórios existentes.
- `limpar <arquivo.nbx>` remove somente o `.out` padrão quando o cabeçalho corresponde a ELF64 little-endian x86-64; rejeita links simbólicos e preserva arquivos que não sejam executáveis ELF desse formato.
- Formatador integrado na CLI principal e disponível como binário separado. Valida a sintaxe, aplica indentação de quatro espaços por bloco, normaliza espaços finais, remove linhas vazias finais, oferece `--check` e grava por arquivo temporário exclusivo, preservando permissões do arquivo.
- Testes de integração para criação, não sobrescrita, compilação/limpeza de artefato e preservação de `.out` que não seja ELF.

Pendente:
- Regras canônicas completas para todos os construtos, round-trip integral e fixtures de formatação ainda pendem; o formatador atual cobre indentação de blocos delimitados por chaves e preservação de strings/comentários.
- Cache incremental de artefatos ELF com SHA-256, invalidado por fonte principal, módulos locais recursivos, versão/fingerprint do compilador, alvo e nível de otimização. Cache incremental por unidade de módulo/IR e invalidação fina ainda pendem.
- Diagnósticos estruturados JSON foram adicionados a `verificar` e `analisar`; `alvos` lista somente backends reais e `compilar --alvo` rejeita destinos não implementados. Ainda faltam trechos/sugestões avançados e opções completas de build.
- Caminhos de I/O, permissões, arquivos grandes e falhas multiplataforma para todos os comandos.

## Item 10 — Qualidade e segurança

**Estado: parcial.**

Implementado:
- CI para `cargo fmt`, Clippy com `-D warnings`, testes do workspace, `cargo doc` e benchmark.
- Workflow de `cargo audit` por push/PR e agenda.
- Dependabot semanal para Cargo e GitHub Actions.
- Fuzz smoke determinístico de Lexer/Parser, parser JSON e manifestos de pacotes com entradas malformadas.
- Testes de travessia de diretórios e symlinks na resolução de módulos, cache e lockfile.
- Guards de empréstimo RAII no runtime: múltiplos leitores ou um escritor exclusivo por identificador, com liberação automática no descarte.
- Restrições SemVer locais com versões numéricas sem limite de `u64`, pré-lançamentos, metadados, comparadores, caret, tilde, intervalos com hífen, conjunções e alternativas `||`, associadas às dependências do manifesto.
- Crate `nebula-seguranca` com SHA-256, HMAC-SHA-256 e comparação de bytes sem encerramento antecipado pelo conteúdo; vetores de teste conhecidos.
- Parser e serializer JSON com testes de entradas inválidas, chaves duplicadas, Unicode e limites.

Pendente:
- Campanhas contínuas de fuzzing com corpus/cobertura para pacotes, JSON, lockfiles e interfaces críticas. O workflow atual executa apenas fuzz smoke determinístico com entradas limitadas.
- Auditoria independente de código, criptografia, supply chain e segurança.
- Matriz de execução do backend para alvos adicionais e stress de memória/concorrência. A matriz atual valida o workspace Rust em Linux/Windows/macOS, não executáveis Nebula nesses três sistemas. Há baseline inicial de loop e Fibonacci em `benchmarks/README.md`, coletada em runner hospedado variável; ainda falta baseline reproduzível em hardware fixo.

## Item 11 — Documentação e lançamento

**Estado: parcial.**

Implementado:
- Especificação, critérios da Fase 5, documentos de estabilidade, ferramentas, segurança, lançamento e biblioteca padrão.
- Política de relato responsável e índice da documentação.
- Workflow de release restrito ao Linux x86-64 com validação, checksums, atestação de proveniência Sigstore/GitHub e bloqueio explícito enquanto `LICENSE-STATUS.md` não registrar aprovação para distribuição.
- CI ampliada para executar fmt, Clippy, testes e documentação em runners Ubuntu, Windows e macOS; o backend nativo declarado continua sendo apenas Linux x86-64.
- Arquivo `LICENSE` e documento de situação da licença.

Pendente:
- Revisão jurídica e confirmação formal da licença e da titularidade.
- Assinatura destacada/verificável dos executáveis e processo de gestão de chaves organizacionais; a atestação de proveniência já é assinada via Sigstore.
- Instaladores e pacotes testados para cada plataforma suportada.
- Guia de migração completo e referências finais da linguagem, runtime, biblioteca, pacotes e backends.
- Congelamento e auditoria dos contratos 1.0.

## Validação

A CI deve ser consultada no HEAD da branch `main` antes de qualquer marco de conclusão. As execuções aprovadas comprovam somente os jobs e testes executados; não concluem os itens completos nem a Fase 5.
