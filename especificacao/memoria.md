# Memória e segurança — arquitetura inicial

## Objetivo

Permitir aplicações comuns com gerenciamento seguro e, simultaneamente, sistemas operacionais, drivers, runtimes e código embarcado com controle explícito.

## Garantias implementadas no subconjunto da Fase 3

- constante é imutável após a inicialização.
- variavel pode receber novas atribuições compatíveis com seu tipo.
- Atribuições só podem ter como destino uma variável nomeada modificável.
- Listas e valores da IR de referência seguem semântica por valor; não há referências compartilhadas expostas pela linguagem nesta fase.
- Declarações exigem inicializador, portanto não há variável local declarada sem valor inicial.

## Limites explícitos

A Fase 3 não implementa verificador de empréstimo, referências, ponteiros, destrutores, alocadores, regiões, atomics ou concorrência segura. A AST e a gramática iniciais ainda não oferecem sintaxe para esses recursos; portanto, nenhuma garantia de segurança de memória baseada em ownership/borrow checker deve ser anunciada.

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
