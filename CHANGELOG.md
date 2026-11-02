# Histórico de alterações

## Não lançado — Fase 3: semântica, tipos e IR

- Adiciona o crate nebula-semantica com tabelas de símbolos, escopos léxicos e resolução de nomes.
- Implementa pré-declaração de funções e aliases, detecção de aliases cíclicos e validação de tipos.
- Implementa tipos inteiros com larguras explícitas, decimais, listas homogêneas e inferência local.
- Valida assinaturas e aridade de funções, operadores, condições, atribuições, retornos e controle de laços.
- Resolve módulos e funções exportadas por meio do ambiente explícito ModulosDisponiveis.
- Define o subconjunto inicial de memória: constantes imutáveis e valores por valor; sem referências ou borrow checker.
- Adiciona o crate nebula-ir com instruções, expressões, funções e operadores normalizados.
- Adiciona interpretador de referência com proteção contra divisão por zero, limite de passos e limite de recursão.
- Adiciona comandos nebula analisar e nebula ir à CLI.
- Adiciona testes unitários e de integração para análise semântica e IR.
- Documenta os critérios e limites de aceite da Fase 3.

## Não lançado — Fase 2: frontend sintático

- Adiciona o crate nebula-sintaxe com Lexer, AST e Parser do subconjunto gramatical inicial.
- Implementa identificadores Unicode XID e normalização NFC.
- Reconhece palavras reservadas, literais, comentários, pontuação, operadores e separadores.
- Implementa análise de declarações, blocos, condicionais, laços e expressões com precedência.
- Adiciona diagnósticos léxicos e sintáticos com intervalos de origem e recuperação de erros.
- Adiciona nebula verificar <arquivo.nbx> à CLI e testes de frontend.
- Adiciona critérios de aceite específicos para a Fase 2.
- Corrige tratamento de CR isolado em comentários e a recuperação após literais inválidos.
- Rejeita comparações relacionais e intervalos encadeados, conforme a gramática.
- Alinha continuação de linhas, inicialização de laços e argumentos da CLI aos contratos documentados.
- Atualiza a ação de checkout da CI para a versão compatível com Node.js 24.
- A análise semântica e a geração de executáveis continuam fora deste escopo.

## Não lançado — Auditoria final da Fase 1

- Corrige a árvore de arquivos documentada e registra critérios formais de aceite da Fase 1.
- Remove produções de tipos sem tokens correspondentes e elimina ambiguidades na notação da gramática.

## Não lançado — Consolidação da fundação

- Sincroniza a gramática, as palavras reservadas e as regras léxicas do subconjunto inicial.
- Define explicitamente recursos reservados para versões futuras e evita apresentá-los como sintaxe disponível.
- Esclarece separação de linhas, precedência, comparação, blocos e limites de literais.
- Fixa a versão mínima/reprodutível do toolchain Rust e adiciona Cargo.lock.
- Fortalece a CI para validar o workspace usando o lockfile.

## 0.1.0 — Fundação inicial

- Registro da identidade e dos princípios da linguagem.
- Especificação inicial, gramática de trabalho e decisões arquiteturais.
- Workspace Rust e crates-base de fonte, diagnósticos e CLI informativa.
- Integração contínua para formatação, análise estática e testes.
- Nenhum Lexer, Parser, análise semântica ou backend implementado nesta versão.
