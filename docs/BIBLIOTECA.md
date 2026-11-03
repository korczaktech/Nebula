# Biblioteca padrão — estado real das APIs

Este documento descreve somente APIs Rust que existem no repositório. A existência de uma API neste crate não significa que ela já possa ser chamada diretamente a partir de código `.nbx`: a integração da biblioteca com o frontend, a IR, o interpretador e os backends ainda é parcial.

## Módulos disponíveis

| Módulo | APIs presentes | Limites relevantes |
|---|---|---|
| Núcleo | Tipos de erro e valores do runtime | Ainda não constitui interface estável 1.0 |
| Matemática | Operações numéricas verificadas e funções matemáticas | Não cobre todas as funções especiais nem todos os tipos em todos os backends |
| Texto | Operações Unicode e conversões | Não promete normalização/segmentação linguística completa |
| Coleções | Memória lexical e valores dinâmicos | Não é a API definitiva de coleções genéricas da linguagem |
| Arquivos | Leitura, escrita, cópia, movimento, diretórios e remoção | Permissões e diferenças do sistema operacional permanecem explícitas |
| Rede | Cliente TCP síncrono com timeout e limite de leitura | Não oferece TLS, HTTP, DNS assíncrono ou protocolos de alto nível |
| Tempo | Relógio Unix em milissegundos e suspensão | Não oferece calendários, fusos horários ou relógio monotônico público completo |
| Sistema | Argumentos, plataforma e variáveis de ambiente | Não oferece todos os serviços de processos e sistema operacional |
| Memória | Alocador com quota e handles geracionais; tabela de empréstimos compartilhados/exclusivos com guards RAII | Controle dinâmico por identificador; não é o borrow checker estático e não é associado automaticamente às variáveis do programa |
| Paralelo | Tarefas, canais, atomics e cancelamento cooperativo | Sem async/await na gramática e sem integração completa ao ownership |
| Segurança | SHA-256, HMAC-SHA-256 e comparação de bytes sem encerramento antecipado pelo conteúdo | Comprimento é observável; sem RNG criptográfico, KDF, gestão de chaves, TLS ou protocolos completos |
| Dados | JSON estrito, serialização determinística e validação de entrada | Números são representados por ponto flutuante de 64 bits; sem formatos binários, esquema ou migrações |
| Interface | Buffer textual, texto, caixas e saída ANSI | Sem janela nativa, eventos de teclado/mouse, acessibilidade ou layout gráfico |
| Gráficos | Imagem RGBA em memória, linhas, retângulos e exportação de bytes | Sem codecs PNG/JPEG, GPU, janela, renderização 3D ou integração com drivers |
| Banco | Banco local de documentos JSON, snapshots e commit otimista dentro do mesmo processo | Sem SQL, rede, índices, bloqueio interprocessos, WAL, replicação ou durabilidade transacional 1.0 |

## Segurança de dados

O parser JSON rejeita chaves duplicadas, números não finitos, escapes Unicode inválidos, conteúdo extra e profundidade acima do limite configurado. O serializador ordena chaves de objetos por ordem lexicográfica para produzir saídas determinísticas.

O banco local grava um snapshot JSON por substituição de arquivo e detecta conflitos entre transações que compartilham a mesma instância `BancoJson`. Ele não coordena escritores de outros processos; múltiplas instâncias ou processos apontando ao mesmo arquivo não possuem isolamento garantido. Não use esse banco como substituto de um SGBD em aplicações concorrentes multi-processo.

## Status de integração

As APIs estão organizadas no crate `nebula-biblioteca` e possuem testes Rust. A integração delas como módulos importáveis de Nebula (`incluir Segurança`, `incluir Dados` etc.), as assinaturas da linguagem, o encaminhamento de erros e o suporte no backend ELF ainda precisam ser implementados e testados. Por isso, os módulos da tabela não são declarados como biblioteca padrão completa ou estável.
