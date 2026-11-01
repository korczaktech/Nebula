# Memória e segurança — arquitetura inicial

## Objetivo

Permitir aplicações comuns com gerenciamento seguro e, simultaneamente, sistemas operacionais, drivers, runtimes e código embarcado com controle explícito.

## Diretrizes

1. Modelo híbrido: propriedade/vida útil verificadas estaticamente no caminho seguro e recursos de baixo nível explicitamente delimitados.
2. Sem coletor de lixo global obrigatório.
3. Runtime opcional quando o alvo e o programa permitirem.
4. Alocadores configuráveis e interfaces explícitas para alocação.
5. Operações inseguras devem ser visíveis, localizadas e auditáveis.
6. O compilador deve prevenir referências inválidas no subconjunto seguro.
7. Concorrência segura deve considerar propriedade, sincronização e compartilhamento.

## Não decidido na Fase 1

Sintaxe final de empréstimo, mutabilidade de referências, destrutores, alocadores regionais, ponteiros crus, atomics e modelo de memória concorrente. Nenhuma garantia específica deve ser anunciada antes de testes e especificação formal.
