# Critérios de aceite — Fase 5 e Nebula 1.0

**Estado: EM DESENVOLVIMENTO.** Este documento define o alvo integral solicitado para a Fase 5. Os itens abaixo distinguem implementações parciais de requisitos completos. Nenhum item amplo é considerado concluído apenas porque uma parte foi implementada. A fase só poderá ser encerrada após implementação, testes automatizados, execução nos alvos declarados, documentação e distribuição verificáveis.

## Progresso de implementação verificado

| Item | Estado atual |
|---|---|
| 1. Linguagem e tipos | Parcial: especificação normativa em `especificacao/linguagem-e-tipos.md`; primitivas, tipos nominais, genéricos e tuplas são validados e representados na IR. Estruturas e enumerações têm lexer/parser/AST, validação semântica, construtores, payloads, padrões exaustivos e layouts físicos calculados em `ProgramaIr.layouts_tipos`; o runtime calcula tamanho, alinhamento, padding e tags. Tipos de função têm sintaxe `funcao(T1, T2): Retorno`, resolução semântica recursiva, layout de ponteiro de código e valores de função com chamadas indiretas no interpretador; o backend ELF também gera referências e chamadas indiretas para assinaturas escalares, incluindo funções retornadas por outras funções. `Opcional` e `Resultado` executam construtores e correspondência de padrões. A IR normaliza operações tipadas: overflow é verificado para inteiros assinados/sem sinal de 8 a 128 bits; `decimal32` arredonda em precisão IEEE-754 binary32 após cada operação. O backend ELF x86-64 representa caracteres Unicode por pontos de código escalares e valida o intervalo, além de implementar operações SSE2 de decimal32/decimal64 (aritmética básica e comparações). Igualdade estrutural é validada e executada para listas, tuplas, estruturas, enumerações e tipos algébricos; movimentação de payloads é rastreada. Campos não copiáveis de estruturas e posições de tuplas com índice literal podem ser movidos individualmente, com acesso a irmãos preservado, rejeição do uso integral do agregado, reatribuição do caminho e junção conservadora entre ramos; índices literais de listas também movem apenas o elemento selecionado, preservando irmãos e permitindo reatribuição; índices dinâmicos ainda movem conservadoramente a lista inteira. A inferência genérica rejeita lacunas de tipo aninhadas e unificações inconsistentes. A ABI nativa agora passa e retorna estruturas/tuplas de até 16 bytes nos formatos de campos suportados e enumerações/Opcionais/Resultados com no máximo um payload escalar, além de gerar construção e pattern matching nesses casos; layouts maiores e payloads múltiplos ainda pendem. O ABI próprio também transporta `decimal32`/`decimal64` em registradores inteiros, incluindo parâmetros, retornos, campos compostos e payloads escalares; testes ELF de integração foram adicionados para esses caminhos e aguardam validação do CI. Inteiros de 128 bits usam dois registradores para parâmetros e retornos e implementam soma, subtração, comparação, negação assinada e multiplicação nativa por algoritmo binário com verificação de overflow e divisão/resto nativos por divisão restauradora de 128 iterações, incluindo divisão por zero, sinais e overflow do quociente; conversões entre inteiros de 128 bits e larguras de até 64 bits já verificam sinal e faixa, conversões entre inteiros de 128 bits e decimal32/decimal64 agora têm geração nativa baseada em SSE2 e divisão por 2^64, com verificação de faixa na conversão decimal→inteiro; a cobertura de execução e conformidade desses limites ainda precisa ser ampliada; conversões decimais/inteiras no backend ELF agora incluem inteiro128↔decimal32/decimal64, mas ainda exigem cobertura de conformidade e execução de limites em todos os cenários, cópia explícita/derivada, movimentação parcial de payloads de enumerações e inicialização parcial geral, inferência completa de tipos e fixtures normativas integrais. Testes Linux adicionais executam os ELF gerados para multiplicação, divisão e resto assinados/sem sinal de 128 bits, incluindo operandos acima de 64 bits, divisão por zero e overflow; também executam conversões decimal↔inteiro128, cobrindo sinal negativo e valores cuja parte alta excede 64 bits. CI em Windows, Debian 13 Trixie e Ubuntu aprovou esses testes no commit `f841333`. A IR também valida a compatibilidade dos valores normalizados com tipos primitivos, estruturas, enumerações, opcionais, resultados, funções, listas e tuplas, rejeitando valores incompatíveis em vez de aceitá-los silenciosamente; testes de regressão cobrem incompatibilidades e payloads algébricos. Intervalos tipados agora validam a família do tipo interno e normalizam cada limite conforme sua largura ou precisão, rejeitando intervalos texto/integer/decimal incompatíveis e valores fora da faixa; testes cobrem inteiros assinados/sem sinal, limites e arredondamento decimal32. A normalização da IR também formaliza a promoção numérica de inteiros para decimal32/decimal64, com arredondamento binary32 e testes para valores com e sem sinal; a validação do CI identificou e motivou essa correção após remover a aceitação permissiva genérica. Foram acrescentados testes semânticos para a matriz de tipos primitivos, limites superiores de inteiros de 128 bits, valores fora da faixa e ausência de coerções implícitas. A análise semântica também possui regressões para intervalos numéricos válidos e rejeição de limites textuais ou com tipos mistos. O tipo genérico `Intervalo<T>` agora também é resolvido em anotações explícitas e assinaturas, não apenas inferido em expressões. Um teste de integração compila, executa e percorre `Intervalo<inteiro>` anotado, verificando os limites inclusivos. O backend rejeita recursos sem implementação. A validação semântica agora percorre recursivamente árvores de tipos compostos e rejeita referências desconhecidas em aliases, parâmetros/retornos, variáveis, campos de estruturas e payloads de enumerações; testes de regressão cobrem cada posição. A política de cópia também foi corrigida para incluir `inteiro8`, com teste que garante cópia em vez de movimentação.
| 2. Memória e segurança | Parcial: verificação estática de move, uso após move, cópia de primitivos, reinicialização, junção conservadora de estados em ramos/laços e movimentação parcial de campos/tuplas/índices literais; alocador Rust com handles geracionais/quota; runtime oferece guards RAII de empréstimos compartilhados/exclusivos por chave, com exclusão dinâmica entre threads. Borrow checker estático, referências/vidas úteis na linguagem, associação dos guards a variáveis/IR, destruição definida pelo usuário e inicialização parcial geral continuam pendentes |
| 3. Runtime | Parcial: contexto Rust integrado ao interpretador, finalização LIFO/idempotente, limpeza no descarte, isolamento de panics e alocador seguro de blocos com quota e handles geracionais. Foi adicionada uma camada de descrição/validação de valores tipados compostos e cálculo de layout; ela ainda não está ligada aos valores da IR nem ao ABI dos executáveis. ABI/runtime nativo, ligação com executáveis, política de panic/abort e integração completa da biblioteca padrão pendentes |
| 4. Concorrência | Parcial: threads, tarefas, canais, inteiros atômicos e token de cancelamento cooperativo no runtime; async/await e integração com propriedade pendentes |
| 5. Biblioteca padrão | Parcial: `nebula-biblioteca` expõe APIs Rust para Núcleo, Matemática, Texto, Coleções, Arquivos, Rede TCP, Tempo, Sistema, Memória, Paralelo, Segurança (SHA-256/HMAC/comparação), Dados (JSON), Interface textual, Gráficos RGBA e Banco JSON local. Todas têm limites documentados; a integração para chamadas a partir de `.nbx`, testes de integração e estabilidade 1.0 ainda pendem |
| 6. Módulos e pacotes | Parcial: resolução de módulos locais recursiva; `nebula-pacotes` valida SemVer do manifesto e restrições de dependência `*`, `^`, `~`, comparadores e conjunções, resolve dependências locais transitivas, detecta ciclos/conflitos, grava lockfile atômico com SHA-256 e confere lockfile sem alterá-lo. Cache incremental do compilador considera fonte principal, fontes de módulos recursivos, versão, fingerprint do binário compilador, alvo e otimização, e verifica checksum do ELF. Registro remoto, ranges completos/pré-lançamentos SemVer, cache de pacotes, integridade de origem e ligação de bibliotecas ainda pendentes |
| 7. Otimizador | Parcial: O0/O1/O2/O3/Os; dobramento, fluxo constante, remoção de código inalcançável/blocos vazios e propagação conservadora de constantes imutáveis; eliminação de redundâncias, análise de fluxo avançada e otimizações seguras de chamadas/laços pendentes |
| 8. Backends | Parcial: geração ELF64 Linux x86-64 permanece o único backend real; API de alvos lista o suporte efetivo e diagnostica ARM64, Windows, macOS e WebAssembly como não implementados. Backends adicionais ainda pendentes |
| 9. Ferramentas | Parcial: CLI oferece criar, compilar, executar, testar, verificar, analisar, ir, interpretar, formatar, dependencias resolver/verificar, limpar, limpar-cache e versao; cache incremental de artefatos nativos já é usado na compilação. `formatar` continua conservador e não faz indentação canônica; compilação incremental por unidades/IR, diagnósticos estruturados e opções multiplataforma ainda pendem |
| 10. Qualidade e segurança | Parcial: CI de fmt/Clippy/testes/docs em runners Ubuntu, Windows e macOS, benchmark Linux; `cargo audit` agendado; fuzz smoke determinístico de Lexer/Parser/JSON/manifestos; SHA-256/HMAC com vetores conhecidos e testes de travessia/symlink. A matriz é de execução do workspace Rust, não de backends Nebula adicionais; fuzzing contínuo, auditoria independente, stress e baseline ampla ainda pendem |
| 11. Documentação e lançamento | Parcial: README e critérios atualizados; estabilidade 1.0, instaladores, assinaturas e distribuição pendentes |
| 12. Autocompilação | Não implementada; permanece dependente de maturidade da linguagem |

A validação automatizada de runtime, builtins, interpretador, CLI, otimizador, concorrência, imports recursivos e chamadas de módulos no backend ELF passou em [CI no commit 6a7bc86](https://github.com/korczaktech/Nebula/actions/runs/41475056421). Essa aprovação comprova somente os testes executados nesse workflow, não a conclusão da Fase 5.

## Objetivo

Consolidar a Nebula como linguagem de propósito geral e publicar a primeira versão estável 1.0, contemplando o conjunto completo de capacidades planejadas no README: linguagem e tipos, memória segura, concorrência, runtime, biblioteca padrão, otimização, módulos e pacotes, backends multiplataforma, ferramentas, documentação, testes, segurança, benchmarks e distribuição.

A Fase 5 não pode apagar nem enfraquecer os contratos já aprovados nas Fases 1–4. Mudanças incompatíveis devem ser deliberadas, documentadas e cobertas por testes de migração.

## 1. Linguagem e sistema de tipos

- [ ] Fechar uma especificação normativa única para sintaxe, semântica, avaliação, precedência, conversões, overflow, erros e comportamento entre plataformas.
- [ ] Implementar e testar todos os tipos primitivos planejados: lógicos, inteiros com e sem sinal nas larguras suportadas, ponto flutuante, byte, caractere Unicode, texto e vazio.
- [ ] Implementar tipos compostos e algébricos: listas/vetores, tuplas, estruturas, enumerações, opcionais, resultados e tipos definidos pelo usuário.
- [ ] Implementar funções, tipos de função, genéricos e inferência com limites e regras determinísticas.
- [ ] Definir layout, alinhamento, representação, igualdade, cópia/movimentação, conversões e ABI de cada tipo.
- [ ] Garantir diagnósticos precisos para erros de tipo, inferência, conversão, padrão incompleto e uso inválido.
- [ ] Manter gramática, tokens, AST, análise semântica, IR, exemplos e fixtures de conformidade sincronizados.

## 2. Memória, propriedade e segurança

- [ ] Especificar e implementar propriedade, movimentação, empréstimos compartilhados e exclusivos, mutabilidade e vidas úteis.
- [ ] Rejeitar estaticamente referências pendentes, aliasing mutável inválido, uso após movimentação, empréstimos conflitantes e acessos fora dos limites quando aplicável.
- [ ] Definir destruição determinística, escopos de vida, inicialização parcial, tratamento de recursos e garantias durante retorno/erro.
- [ ] Definir operações inseguras explícitas e suas obrigações; código seguro não pode depender de comportamento indefinido.
- [ ] Implementar alocadores e interfaces de memória documentados, incluindo alocação dinâmica e liberação segura.
- [ ] Não exigir coletor de lixo global; qualquer gerenciamento automático deve ter semântica e custo documentados.
- [ ] Testar casos positivos e negativos, stress de memória, sanitização quando disponível e regressões de segurança.

## 3. Runtime

- [ ] Implementar runtime próprio em Rust, modular e opcional quando o programa não precisar dele.
- [ ] Definir inicialização, encerramento, falhas, panic/abort, pilhas, alocação, destruição e integração com o sistema operacional.
- [ ] Implementar os serviços necessários à linguagem e à biblioteca padrão sem depender de runtime C++.
- [ ] Definir interfaces estáveis entre código gerado, runtime e módulos nativos.
- [ ] Testar reentrância, falhas, limites, vazamentos e execução sem runtime quando permitido.

## 4. Concorrência e paralelismo

- [ ] Especificar modelo de tarefas, async/await ou sintaxe equivalente, escalonamento e cancelamento.
- [ ] Implementar threads/tarefas, sincronização, canais/comunicação, atomics e primitivas necessárias.
- [ ] Integrar as garantias de concorrência ao sistema de propriedade e tipos.
- [ ] Definir memória compartilhada, ordenação de memória, deadlocks, panics e encerramento.
- [ ] Testar determinismo quando exigido, condições de corrida, stress, cancelamento e falhas concorrentes.

## 5. Biblioteca padrão

- [ ] Entregar e testar os módulos planejados: Núcleo, Matemática, Texto, Coleções, Arquivos, Rede, Tempo, Sistema, Memória, Paralelo, Segurança, Dados, Interface, Gráficos e Banco.
- [ ] Documentar API pública, erros, garantias, dependências, portabilidade e exemplos de cada módulo.
- [ ] Implementar apenas APIs reais; nenhuma função pode ser placeholder ou simulação apresentada como pronta.
- [ ] Definir política de compatibilidade e versionamento da biblioteca padrão.
- [ ] Cobrir APIs públicas com testes unitários, integração, fixtures e testes específicos por plataforma.

## 6. Módulos, pacotes e ligação

- [ ] Implementar resolução real de módulos e símbolos, visibilidade, reexportação, dependências e detecção de ciclos.
- [ ] Criar formato de pacote, manifesto, versionamento semântico, lockfile e resolução reproduzível.
- [ ] Implementar comandos de dependências e operações de pacote na CLI.
- [ ] Definir cache, integridade, origem e verificação de conteúdo dos pacotes.
- [ ] Implementar ligação de unidades e bibliotecas estáticas/dinâmicas conforme cada alvo.
- [ ] Testar dependências transitivas, conflitos de versão, ausência de rede, cache corrompido e builds reproduzíveis.

## 7. Otimizador

- [ ] Implementar otimizador próprio sobre IR, sem delegar otimização a compiladores de outra linguagem.
- [ ] Entregar níveis O0, O1, O2, O3 e Os com semântica documentada.
- [ ] Implementar passes aprovados por testes de equivalência: propagação de constantes, simplificação, eliminação de código morto, análise de fluxo, eliminação de redundâncias e otimizações de chamadas/loops quando seguras.
- [ ] Validar cada transformação isoladamente e em composição; otimização não pode alterar resultados, efeitos observáveis ou diagnósticos definidos.
- [ ] Comparar builds otimizados e não otimizados em benchmarks reproduzíveis, publicando cargas, ambiente, metodologia e limitações.
- [ ] Não declarar metas de desempenho como garantias sem evidência estatística reproduzível.

## 8. Backends e plataformas

- [ ] Preservar o alvo Linux x86-64 ELF64 já validado.
- [ ] Implementar e validar os alvos nativos planejados: ARM64, Windows x86-64 e macOS nas arquiteturas suportadas.
- [ ] Implementar backend WebAssembly e definir a interface de integração com o ambiente hospedeiro.
- [ ] Documentar formato de objeto/executável, ABI, convenção de chamada, layout, relocação, linking, stack, TLS e tratamento de erros por alvo.
- [ ] Não depender obrigatoriamente de compilador C/C++, LLVM, assembler ou linker externo para gerar o código Nebula.
- [ ] Executar testes em runners/ambientes reais de cada alvo; compilação cruzada isolada não basta para declarar execução validada.
- [ ] Rejeitar recursos não suportados com diagnósticos claros, sem produzir artefatos inválidos.

## 9. Ferramentas e experiência de desenvolvimento

- [ ] Completar e testar os comandos oficiais: criar, compilar, executar, verificar, formatar, testar, analisar, dependências, limpar e versão.
- [ ] Implementar formatador determinístico, idempotente e não destrutivo, com testes de round-trip e fixtures.
- [ ] Oferecer diagnósticos humanos e estruturados, posições corretas, trechos, códigos estáveis e sugestões seguras.
- [ ] Implementar compilação incremental/cacheada quando aplicável, com invalidação correta.
- [ ] Definir opções de alvo, otimização, memória, saída e diagnóstico sem flags que finjam suporte.
- [ ] Testar CLI em caminhos com espaços, permissões negadas, arquivos grandes, entradas inválidas e falhas de I/O.

## 10. Qualidade, segurança e desempenho

- [ ] Manter testes unitários, integração, conformidade da linguagem, regressão e testes negativos.
- [ ] Adicionar fuzzing para Lexer, Parser, serialização, pacotes e interfaces críticas.
- [ ] Executar auditorias de dependências, licenças, supply chain e vulnerabilidades.
- [ ] Executar testes de stress e limites para memória, concorrência, compilação e execução.
- [ ] Ter benchmarks representativos de compilação e execução, com baseline versionada e resultados reproduzíveis.
- [ ] CI deve executar fmt, Clippy sem warnings, testes completos, conformidade, fuzz smoke, auditorias aplicáveis e matriz de alvos.
- [ ] Nenhum teste pode ser ignorado, mascarado ou marcado como sucesso sem executar a validação correspondente.

## 11. Documentação, compatibilidade e lançamento

- [ ] Publicar especificação completa da linguagem, guia de migração, referência da CLI, runtime, biblioteca padrão, pacotes e backends.
- [ ] Garantir que exemplos públicos compilem e executem em CI nos alvos declarados.
- [ ] Definir política de estabilidade da linguagem, ABI, runtime, biblioteca padrão e formato de pacotes.
- [ ] Definir versionamento semântico, changelog, suporte, correções e processo de depreciação.
- [ ] Definir licença do projeto e política de distribuição das ferramentas e bibliotecas.
- [ ] Gerar artefatos com atestações assinadas de proveniência, checksums, manifests e instruções de instalação para cada plataforma oficialmente suportada; assinatura destacada dos próprios executáveis ainda exige definição do processo de chaves.
- [ ] Publicar versão 1.0 somente depois da aprovação de todos os critérios obrigatórios e da auditoria final.

## 12. Autocompilação

- [x] Avaliar a autocompilação com critérios objetivos de viabilidade e segurança; a decisão para 1.0 é mantê-la fora do escopo estável.
- [ ] Se entrar no escopo do lançamento, compilar o compilador com a própria Nebula, validar bootstrap reproduzível e comparar artefatos.
- [x] Como não atingiu maturidade para 1.0, documentar a decisão e manter a autocompilação explicitamente experimental/futura, sem alegar que está concluída (`docs/LANCAMENTO.md`).

## Portões de conclusão

A Fase 5 só pode ser marcada como concluída quando:

1. Todos os itens obrigatórios acima estiverem implementados ou tiverem uma decisão explícita e justificada de não integrar o escopo estável 1.0.
2. Não houver API, comando, tipo, backend ou módulo anunciado como pronto que seja placeholder, parcial ou simulado.
3. A suíte completa passar em CI para todos os alvos oficialmente suportados.
4. Os testes de conformidade, segurança, regressão, concorrência e integração forem aprovados.
5. Benchmarks e limitações forem publicados sem alegações não comprovadas.
6. A documentação, os exemplos, os pacotes e os artefatos de distribuição corresponderem ao comportamento real.
7. A auditoria final não encontrar defeitos conhecidos bloqueadores nem divergências entre especificação e implementação.

**Regra de honestidade:** uma CI verde comprova somente os jobs e testes que ela executou. Não substitui revisão de arquitetura, testes nos alvos reais, auditoria de segurança ou validação de distribuição. A expressão “100% concluída” só poderá ser usada com evidências para cada portão acima.
