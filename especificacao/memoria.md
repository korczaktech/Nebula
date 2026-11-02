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
- Atribuição compatível reinicializa uma variável anteriormente movida.
- O runtime Rust possui um alocador de blocos com limite de bytes, identificadores geracionais, validação de intervalos, redimensionamento e invalidação de handles liberados.
- O contexto do runtime é dono do alocador e invalida todas as alocações durante a finalização.

## Limites explícitos

Ainda não há verificador de empréstimos compartilhados/exclusivos, vidas úteis, referências ou ponteiros na AST/gramática. Também faltam destruição por valor, inicialização parcial, integração do alocador com valores da linguagem e ABI nativa. O acompanhamento de moves é uma análise parcial e não equivale a um borrow checker completo; portanto, nenhuma garantia geral de segurança de memória baseada em ownership/borrow checker deve ser anunciada.

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
