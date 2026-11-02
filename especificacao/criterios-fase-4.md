# Critérios de aceite — Fase 4

**Estado: EM VALIDAÇÃO.** A fase só será marcada como concluída após a CI validar o commit integrado, incluindo os testes ponta a ponta e a execução do benchmark.

## Objetivo

Entregar compilação executável e desempenho inicial: sair da IR para um binário nativo real, executar programas do MVP e disponibilizar uma primeira forma reproduzível de comparar níveis de otimização.

## Entregas e critérios

| Critério | Implementação | Evidência |
|---|---|---|
| Pipeline completo | Lexer/Parser → semântica → IR → backend → compilador C++17 | CLI compila fontes `.nbx` até executáveis |
| Backend nativo | `nebula-backend`, geração C++17 | Binários executados no Linux x86-64 da CI |
| Linkedição | Invocação do compilador C++ do sistema | Erros do compilador/linker retornados à CLI; binário removido em caso de falha |
| Variáveis e tipos MVP | Inteiros até 64 bits, byte, decimais, lógico, texto, caractere ASCII e listas homogêneas | Testes de execução de valores e listas |
| Expressões | Aritmética, potência, comparações, lógica, concatenação de texto e atribuição | Testes ponta a ponta |
| Listas | Construção, passagem/retorno por função e indexação com verificação de limites | Testes de execução e geração de `std::vector` |
| Funções | Protótipos, parâmetros, retorno e recursão | Testes de chamadas e Fibonacci recursivo |
| Controle de fluxo | `quando/senao`, `enquanto`, `para`, `parar` e `seguir` | Testes de execução |
| Biblioteca padrão essencial | `exibir` e `ler_texto` | Testes de stdout e stdin |
| CLI | `compilar`, `executar`, `testar`, `verificar`, `analisar` e `ir` | Testes de integração |
| Otimização inicial | `-O0`, `-O1`, `-O2`, `-O3` e `-Os`, encaminhados ao compilador C++ | Testes de todos os níveis |
| Benchmark reproduzível | Laço de 50 milhões de iterações e Fibonacci recursivo; cinco medições por nível | `benchmarks/executar.sh`, executado na CI |
| Diagnósticos de backend | Falhas de geração, compilação e linkedição propagadas sem deixar executável inválido | Teste de fonte inválida e verificação do artefato |
| CI | Formatação, Clippy, testes e benchmark | Workflow verde com `--locked` |

## Alvo e requisitos

- Alvo validado nesta fase: Linux x86-64.
- Requer Rust 1.85.0 para construir a CLI e um compilador C++17 acessível como `c++` para gerar executáveis.
- O backend gera C++17 como representação de baixo nível e delega a geração de código de máquina e a linkedição ao compilador C++ instalado.
- Os níveis de otimização são encaminhados ao compilador C++; não são passes próprios de otimização sobre a IR.
- O benchmark não comprova a meta experimental de desempenho de até 3× C++.

## WebAssembly

O backend WebAssembly não foi incluído nesta fase. A validação exigiria um toolchain e sysroot WASI disponíveis e testados no ambiente de CI e nos alvos declarados. Como esse ambiente não está estabelecido, a implementação seria uma promessa não comprovada; o alvo fica para uma fase posterior.

## Limites conhecidos

- `inteiro128` ainda não possui ABI nativa estável.
- `caractere` aceita caracteres ASCII no backend atual; texto suporta UTF-8.
- Resolução de módulos fornecidos em memória existe na Fase 3, mas carregamento de módulos do disco e resolvedor de pacotes permanecem fora desta fase.
- A biblioteca padrão inicial cobre entrada de uma linha e saída; não é a biblioteca padrão completa.
- O backend depende do compilador C++ externo e não promete portabilidade binária entre plataformas ainda não testadas.
- Não há alegação de desempenho superior a C/C++; os benchmarks são uma linha de base inicial.

## Comandos

```text
nebula verificar programa.nbx
nebula analisar programa.nbx
nebula ir programa.nbx
nebula compilar programa.nbx [-o executavel] [-O0|-O1|-O2|-O3|-Os]
nebula executar programa.nbx
nebula testar programa.nbx
```

A Fase 4 será concluída somente quando os critérios acima aplicáveis estiverem implementados, testados e documentados, e a CI integrada estiver verde.