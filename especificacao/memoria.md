# Memória e segurança — arquitetura inicial

## Objetivo

Permitir aplicações comuns com gerenciamento seguro e, simultaneamente, sistemas operacionais, drivers, runtimes e código embarcado com controle explícito.

## Garantias implementadas no subconjunto da Fase 3

- constante é imutável após a inicialização.
- variavel pode receber novas atribuições compatíveis com seu tipo.
- Atribuições só podem ter como destino uma variável nomeada modificável.
- Listas e valores da IR de referência seguem semântica por valor; não há referências compartilhadas expostas pela linguagem nesta fase.
- Declarações exigem inicializador, portanto não há variável local declarada sem valor inicial.

## Implementações parciais da Fase 5

- A análise semântica acompanha movimentações de valores não copiáveis e rejeita uso após move.
- A movimentação é propagada de forma conservadora na junção de ramos condicionais e em laços que podem executar zero vezes.
- A IR representa movimentos nomeados por uma operação explícita `Mover`; a geração automática já cobre inicializações, retornos, atribuições, argumentos de chamadas com assinatura conhecida, agregados e valores consumidos por `escolha`.
- O backend nativo reconhece a operação de movimento e carrega a representação do valor; a validação estática continua responsável por impedir reutilização indevida no caminho compilado.
- O runtime rejeita movimento enquanto há empréstimo ativo, rejeita leituras após movimento e permite reinicialização por atribuição compatível.
- A propagação de constantes da IR invalida a constante de uma variável movida, evitando substituir leituras posteriores pelo valor antigo.
- Atribuição compatível reinicializa uma variável anteriormente movida.
- Junções de fluxo preservam todos os empréstimos ativos de ramos alternativos, inclusive quando identificadores provisórios colidem em ramos clonados; identificadores são remapeados sem descartar empréstimos distintos. A análise de proveniência de referências também une os caminhos de `quando`, expressões condicionais, `escolha` e laços; origens divergentes são invalidadas conservadoramente.
- O módulo de semântica contém um analisador de CFG por fila de trabalho, com convergência em ciclos, operações explícitas de escopo, validação de contexto inseguro e emissão de planos de destruição LIFO. Declarações e empréstimos usam identidades estáveis durante reprocessamentos do CFG.
- O plano semântico de destruição omite valores integralmente movidos e registra projeções parcialmente movidas para impedir destruição dupla. A IR/runtime ainda não consome esse plano para destruição parcial; o runtime fecha escopos e finalizadores em LIFO, mas destrutores definidos pelo usuário e exclusão física de campos parcialmente movidos não estão concluídos.
- O runtime Rust possui um alocador de blocos com limite de bytes, identificadores geracionais, validação de intervalos, redimensionamento e invalidação de handles liberados.
- O contexto do runtime é dono do alocador e invalida todas as alocações durante a finalização.

## Proteção dinâmica de empréstimos

O runtime oferece `TabelaEmprestimos` como mecanismo dinâmico de exclusão por recurso: empréstimos compartilhados coexistem; um empréstimo exclusivo exige ausência de qualquer outro empréstimo; guards liberam os empréstimos automaticamente no encerramento do escopo que criou a referência. A identidade do recurso combina o identificador estável do escopo de origem com o nome da variável, evitando colisões por sombreamento.

A IR agora adquire guards ao avaliar `&valor` e `&mutavel valor`. A validação dinâmica também bloqueia leituras diretas do proprietário durante empréstimo exclusivo e escritas diretas durante qualquer empréstimo ativo. A desreferência usa o caminho de acesso referenciado, preservando a permissão concedida pelo guard. Os guards são liberados em ordem LIFO antes do fechamento da memória do escopo, inclusive no caminho de erro/retorno que fecha o bloco.

A duração dinâmica atual é lexical: o guard permanece até o fim do escopo que criou a referência. Isso é conservador e pode manter empréstimos ativos depois do último uso; análise NLL e encerramento antecipado por último uso ainda não estão implementados. O mecanismo dinâmico complementa, mas não substitui, a análise estática.

## Limites explícitos

A análise estática de referências e empréstimos está em integração e ainda não constitui um borrow checker completo com vidas úteis inferidas até o último uso. O plano semântico de destruição de valores movidos parcialmente ainda não é consumido pela IR/runtime, e destrutores definidos pelo usuário e inicialização parcial geral não estão concluídos. A validação dinâmica é aplicada aos acessos diretos de variável e campo representados pela IR; outras interfaces de memória e interoperabilidade devem ser auditadas antes de anunciar uma garantia geral de segurança de memória.

## Diretrizes arquiteturais

1. Modelo híbrido: propriedade/vida útil verificadas estaticamente no caminho seguro e recursos de baixo nível explicitamente delimitados.
2. Sem coletor de lixo global obrigatório.
3. Runtime opcional quando o alvo e o programa permitirem.
4. Alocadores configuráveis e interfaces explícitas para alocação.
5. Operações inseguras devem ser visíveis, localizadas e auditáveis.
6. O compilador deve prevenir referências inválidas no subconjunto seguro quando referências forem introduzidas.
7. Concorrência segura deve considerar propriedade, sincronização e compartilhamento.

## Não decidido

Sintaxe final de empréstimo, mutabilidade de referências, destrutores, alocadores regionais, ponteiros crus, atomics e modelo de memória concorrente. Nenhuma garantia específica deve ser anunciada antes de testes e especificação formal.
