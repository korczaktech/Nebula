# Nebula

> **Uma linguagem própria, construída para fazer tudo.**

Nebula é uma linguagem de programação de propósito geral, com identidade brasileira, sintaxe própria e foco em desempenho, simplicidade, organização e portabilidade.

| Propriedade | Definição |
|---|---|
| Nome | Nebula |
| Extensão | `.nbx` |
| Idioma principal | Português brasileiro |
| Implementação inicial do compilador | Rust |
| Modelo | Compilada, multiparadigma e multiplataforma |
| Tipagem | Estática, com inferência controlada |
| Organização visual | Delimitadores flexíveis; uso conforme necessidade de clareza |
| Memória | Modelo híbrido |
| Alvos planejados | Código nativo e WebAssembly |

A meta de desempenho é investigar e buscar resultados de até 3× C++ em cargas específicas. Isso é uma meta experimental, não uma garantia universal: qualquer comparação deverá ser sustentada por benchmarks reproduzíveis.

---

## 1. Visão e objetivos

Nebula deverá permitir o desenvolvimento de:

- Sistemas operacionais, kernels e drivers.
- Aplicativos desktop e móveis.
- Aplicações web e backends.
- APIs, serviços e sistemas distribuídos.
- Jogos e motores gráficos.
- Sistemas embarcados.
- Bibliotecas, ferramentas e infraestrutura.

### Princípios

1. Desempenho como prioridade arquitetural.
2. Sintaxe original, não uma tradução de Python, C++ ou outra linguagem.
3. Facilidade de leitura e aprendizagem.
4. Organização rigorosa e regras consistentes.
5. Controle previsível de memória.
6. Segurança sem sacrificar desempenho desnecessariamente.
7. Compilação para múltiplas plataformas.
8. Biblioteca padrão modular.
9. Diagnósticos claros em português.
10. Possibilidade futura de autocompilação.

## 2. Identidade e regras iniciais da linguagem

A gramática e as regras léxicas do subconjunto inicial estão formalizadas em [especificacao/gramatica.md](especificacao/gramatica.md) e [especificacao/tokens.md](especificacao/tokens.md). Esses documentos são a referência normativa do Lexer, Parser e AST já implementados no crate `nebula-sintaxe`.

- Código-fonte UTF-8; identificadores Unicode comparados em NFC.
- Palavras-chave da linguagem e mensagens oficiais em português brasileiro.
- Indentação não define blocos nem altera o significado.
- Instruções podem ser separadas por quebra de linha ou ponto e vírgula, conforme as regras da gramática.
- Blocos aceitam chaves ou o terminador fim.
- Parênteses em condições podem ser omitidos quando o início do bloco delimitar a expressão sem ambiguidade.
- Chamadas, parâmetros de funções e cabeçalhos de para mantêm os delimitadores definidos na gramática.
- Literais de texto usam aspas duplas; caracteres usam aspas simples.

Exemplo do subconjunto inicial:

~~~nebula
funcao principal(): vazio {
    variavel nome = "Nebula"
    constante idade: inteiro = 18

    quando idade >= 18 {
        exibir("Olá, " + nome)
    } senao {
        exibir("Olá")
    }
}
~~~

A lista de palavras reservadas implementadas na gramática e a lista separada de palavras reservadas futuras estão em especificacao/tokens.md. Uma palavra reservada futura não significa que o recurso correspondente já esteja disponível.

## 3. Arquitetura geral do compilador

O compilador será desenvolvido inicialmente em Rust e organizado em componentes independentes.

```text
Código-fonte (.nbx)
        |
        v
Analisador léxico (Lexer)
        |
        v
Analisador sintático (Parser)
        |
        v
Árvore sintática abstrata (AST)
        |
        v
Análise semântica
        |
        v
Sistema de tipos
        |
        v
Representação intermediária (IR)
        |
        v
Otimizador
        |
        v
Gerenciador de alvos
      /       \
     v         v
  Nativo     WebAssembly
     \         /
      v       v
   Executável / módulo
```

### 3.1 Fonte e Lexer

O crate `nebula-sintaxe` implementa o Lexer do subconjunto inicial e produz tokens com lexemas originais e intervalos em bytes.

O Lexer reconhece palavras reservadas, identificadores Unicode, números, textos, caracteres, operadores, comentários e quebras de linha. Aplica maior correspondência aos operadores, normaliza identificadores em NFC e diagnostica entradas léxicas inválidas. A leitura da CLI rejeita arquivos que não sejam UTF-8 válido.

### 3.2 Parser

O Parser do crate `nebula-sintaxe` transforma os tokens em uma AST independente da CLI, valida o subconjunto gramatical inicial e emite diagnósticos para construções inválidas. Implementa recuperação para continuar a análise quando possível e preservar uma AST parcial.

### 3.3 AST

Representa a estrutura lógica do programa sem depender de plataforma específica.

Deverá contemplar declarações, variáveis, constantes, funções, tipos, condicionais, repetições, operações, chamadas, módulos, tratamento de erros, concorrência e estruturas de dados.

### 3.4 Análise semântica

Verifica o significado do código:
- Escopos e resolução de nomes.
- Compatibilidade de tipos.
- Visibilidade e acesso.
- Inicialização e fluxo de controle.
- Uso de módulos.
- Regras de propriedade, empréstimo e vida útil da memória.

### 3.5 Sistema de tipos

A tipagem será estática por padrão, com inferência controlada.

Deverá suportar tipos primitivos, compostos, definidos pelo usuário, genéricos, opcionais, funções e tipos de baixo nível.

Tipos iniciais em estudo: `inteiro`, `decimal`, `lógico`, `texto`, `caractere`, `byte` e `vazio`. Tamanhos numéricos explícitos deverão estar disponíveis, incluindo variantes de 8, 16, 32 e 64 bits e, quando suportado, 128 bits.

### 3.6 Representação intermediária (IR)

A IR será o núcleo independente de plataforma do compilador.

Deverá representar operações, fluxo de controle, chamadas, memória e estruturas de dados de forma tipada e explícita. Todos os backends receberão IR, não deverão implementar diretamente a semântica da linguagem.

O projeto não será definido nem dependerá de LLVM. Os backends próprios serão implementados em Rust, com geração direta para cada alvo suportado.

### 3.7 Otimizador

Níveis planejados:

| Nível | Objetivo |
|---|---|
| O0 | Compilação rápida, otimização mínima |
| O1 | Otimizações básicas |
| O2 | Otimizações avançadas |
| O3 | Otimização agressiva |
| Os | Redução do tamanho do artefato |

Transformações previstas: eliminação de código morto, propagação de constantes, simplificação de expressões, eliminação de cópias redundantes, otimização de chamadas e loops, análise de fluxo e especialização quando vantajosa.

Nenhuma otimização poderá alterar o comportamento definido pela linguagem. Cada transformação deverá possuir testes de correção e, quando relevante, benchmarks.

## 4. Memória e execução

Nebula adotará um modelo híbrido:

- Gestão automática quando for segura e adequada.
- Controle explícito para sistemas, drivers, infraestrutura e código de baixo nível.
- Propriedade, empréstimo e verificação estática para reduzir referências inválidas, cópias e acessos inseguros.
- Alocadores e estratégias configuráveis conforme o ambiente.

Não haverá exigência de um coletor de lixo global em todos os programas. O runtime deverá ser opcional sempre que possível, especialmente em código de baixo nível.

### Concorrência

A linguagem deverá oferecer tarefas assíncronas, paralelismo, sincronização, operações atômicas, comunicação entre tarefas e compartilhamento seguro de dados. Recursos de runtime serão incluídos apenas quando necessários ao programa.

## 5. Módulos e biblioteca padrão

A inclusão de módulos será explícita e modular:

```nebula
incluir Matemática
incluir Arquivos
incluir Rede
```

Cada módulo deverá ter identidade, interface, dependências e regras de visibilidade definidas. O compilador deverá processar somente os módulos necessários.

Módulos planejados para a biblioteca padrão:

| Módulo | Responsabilidade |
|---|---|
| Núcleo | Recursos essenciais |
| Matemática | Operações matemáticas |
| Texto | Manipulação de textos |
| Coleções | Vetores, listas e mapas |
| Arquivos | Sistema de arquivos |
| Rede | Comunicação de rede |
| Tempo | Datas, horários e intervalos |
| Sistema | Recursos do sistema operacional |
| Memória | Alocação e controle |
| Paralelo | Concorrência e paralelismo |
| Segurança | Operações criptográficas e seguras |
| Dados | Serialização e formatos |
| Interface | Interfaces de usuário |
| Gráficos | Computação gráfica |
| Banco | Integração com bancos de dados |

## 6. Backends e plataformas

### Backend nativo

Geração de executáveis para arquiteturas como x86-64 e ARM64, com suporte progressivo a Linux, Windows, macOS e sistemas embarcados.

### Backend WebAssembly

Geração de módulos WebAssembly para navegadores, aplicações web, serviços isolados e ambientes portáveis. Integrações com JavaScript serão oferecidas quando necessárias.

### Baixo nível

A arquitetura deverá permitir programas sem runtime obrigatório, com controle de ponto de entrada, memória, interrupções, hardware, ABI e ligação de símbolos. Isso será essencial para kernels, drivers e sistemas embarcados.

## 7. Diagnósticos

Mensagens oficiais em português, com dois formatos:

**Compacto**
```text
Erro N001: identificador não declarado.
Arquivo: Agendamento.nbx
Linha: 12
```

**Detalhado**
```text
Erro N001 — Identificador desconhecido

O identificador "cliente" não foi declarado neste escopo.

Localização: Agendamento.nbx:12:8

Sugestão: declare a variável antes de utilizá-la.
```

Cada diagnóstico deverá possuir código estável, gravidade, mensagem, arquivo, linha, coluna, trecho do código e sugestão de correção quando possível.

## 8. Ferramenta de linha de comando

A CLI oficial deverá oferecer comandos equivalentes a:

```text
nebula criar
nebula compilar
nebula executar
nebula verificar
nebula formatar
nebula testar
nebula analisar
nebula dependencias
nebula limpar
nebula versao
```

Deverá permitir selecionar arquivo, plataforma, arquitetura, nível de otimização, opções de memória, formato de saída e formato dos diagnósticos.

## 9. Estrutura planejada do repositório

A estrutura atual do repositório é:

```text
nebula/
├── README.md
├── CONTRIBUTING.md
├── CHANGELOG.md
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── .gitignore
├── .github/
│   └── workflows/
│       └── ci.yml
├── especificacao/
│   ├── linguagem.md
│   ├── decisoes.md
│   ├── criterios-fase-3.md
│   ├── criterios-fase-4.md
│   ├── gramatica.md
│   ├── tokens.md
│   ├── tipos.md
│   ├── memoria.md
│   ├── operadores.md
│   ├── modulos.md
│   ├── diagnosticos.md
│   ├── alvos.md
│   └── criterios-fase-1.md
├── compilador/
│   ├── sintaxe/
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   ├── fonte/
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   ├── semantica/
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   ├── ir/
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   ├── backend/
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   ├── diagnosticos/
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   └── cli/
│       ├── Cargo.toml
│       ├── src/main.rs
│       └── tests/
│           ├── cli.rs
│           └── native.rs
├── benchmarks/
│   ├── README.md
│   ├── executar.sh
│   ├── loop.nbx
│   └── fibonacci.nbx
├── exemplos/
│   └── ola_mundo.nbx
├── testes/
│   └── conformidade/
│       ├── README.md
│       └── sintaxe_visual.nbx
```

Lexer, Parser e AST estão reunidos em `nebula-sintaxe`; análise semântica, tipos e IR estão em `nebula-semantica` e `nebula-ir`. O backend nativo inicial foi entregue na Fase 4; otimizador próprio sobre a IR e backends adicionais permanecem planejados.

### Crates Rust

**Criadas na Fase 1:**
- `nebula-fonte`: arquivo-fonte, intervalos e posições.
- `nebula-diagnosticos`: estrutura de diagnóstico independente da interface.
- `nebula-cli`: interface de linha de comando.

**Criada na Fase 2:**
- `nebula-sintaxe`: Lexer, AST e Parser do subconjunto gramatical inicial.

**Criadas na Fase 3:**
- `nebula-semantica`: símbolos, escopos, resolução de módulos, inferência e verificação estática.
- `nebula-ir`: representação intermediária e interpretador de referência.

**Criada na Fase 4:**
- `nebula-backend`: geração direta de executáveis ELF x86-64 em Rust (em desenvolvimento).

**Planejadas para fases posteriores:**
- `nebula-otimizador`
- `nebula-biblioteca`

As dependências deverão ser unidirecionais sempre que possível. AST não dependerá da CLI; Lexer não dependerá do Backend.

## 10. Testes, segurança e qualidade

Cada componente deverá possuir testes unitários, integração e regressão. O compilador deverá testar programas válidos e inválidos, diferentes estilos de espaçamento e delimitadores, tipos, nomes, memória, concorrência, geração e execução de artefatos.

O CI deverá executar formatação, análise estática, compilação e testes automaticamente.

Regras de qualidade:
1. Não implementar funcionalidades fictícias.
2. Não declarar componentes concluídos sem testes.
3. Não misturar responsabilidades entre módulos.
4. Não acoplar a linguagem a uma única plataforma.
5. Não adicionar dependências sem justificativa.
6. Não sacrificar segurança sem necessidade técnica.
7. Não imitar sintaxe de outras linguagens sem uma decisão de projeto.
8. Documentar decisões arquiteturais.
9. Medir desempenho antes de declarar otimizações eficazes.
10. Manter a especificação sincronizada com a implementação.

## 11. Autocompilação

A meta futura é que o compilador Nebula possa ser compilado pela própria linguagem.

Isso exigirá que Nebula tenha tipos, módulos, estruturas de dados, funções, arquivos, recursos de baixo nível e ferramentas estáveis. A implementação inicial permanecerá em Rust até que a autocompilação seja confiável.

---

# 12. Plano de desenvolvimento — 5 fases

O projeto será desenvolvido em cinco fases. As quatro primeiras deverão entregar, em conjunto, pelo menos **75% de um compilador utilizável e de seu ecossistema essencial**. A quinta fase será reservada para evoluir esse resultado de 75% ou mais até 100%.

Os percentuais são metas de conclusão do escopo total planejado, não medidas automáticas de linhas de código. Uma fase só será considerada concluída quando seus critérios de aceite forem atendidos.

## Fase 1 — Fundação e especificação (meta acumulada: 10%)

**Estado: Fase 1 concluída; especificação consolidada e CI aprovado.**

**Objetivo:** estabelecer a identidade técnica e preparar a base reproduzível do projeto.

Entregas realizadas:
- Especificação inicial consolidada da linguagem, tokens, gramática, tipos, memória, operadores, módulos, diagnósticos, alvos e registro de decisões.
- Separação explícita entre palavras reservadas da gramática inicial e palavras reservadas futuras.
- Regras léxicas e gramaticais alinhadas para quebras de linha, delimitadores, precedência, literais e construções suportadas.
- Regras de separação visual flexível, com delimitadores opcionais e indentação sem significado semântico.
- Workspace Cargo com Rust 1.85.0 fixado, edição 2024 e Cargo.lock versionado.
- Crates iniciais `nebula-fonte`, `nebula-diagnosticos` e `nebula-cli`.
- Tipos básicos de fonte, intervalos, linha/coluna e diagnósticos.
- CLI informativa com ajuda e versão, sem alegar compilação de arquivos.
- Exemplo .nbx e fixtures de conformidade.
- CI para formatação, Clippy e testes com versão Rust e dependências travadas pelo lockfile.
- Política de contribuição, progresso e changelog.

Critérios de conclusão:
- A estrutura e os contratos iniciais estão registrados.
- O código-base está preparado para validação automatizada.
- Gramática e tokens descrevem o mesmo subconjunto inicial, documentam regras de separação e identificam explicitamente os recursos futuros e as decisões ainda abertas.
- Toolchain e resolução de dependências são reproduzíveis por versão fixada e Cargo.lock.
- Workspace compila, passa pela formatação, análise Clippy e testes automatizados no GitHub Actions.

## Fase 2 — Frontend da linguagem (meta acumulada: 30%)

**Estado: Fase 2 concluída; CI verde no commit `d50b121317e19d4eb73453407b095a4c6ad44283`.**

**Objetivo:** transformar código Nebula em uma representação sintática confiável.

Entregas:
- Leitor de arquivos e posições de origem.
- Lexer para o subconjunto léxico normativo, com tokens e intervalos de origem.
- Parser Pratt e AST pública para o subconjunto gramatical aprovado.
- Recuperação de erros e diagnósticos em português.
- Testes positivos, negativos, de regressão e fixtures de conformidade.
- CLI `nebula verificar <arquivo.nbx>` para validação léxica e sintática.

Critérios de conclusão:
- Arquivos de exemplo são tokenizados e analisados corretamente.
- Erros indicam localização precisa.
- Testes cobrem tokens, precedência, estruturas gramaticais e variações válidas de espaçamento e delimitadores.
- Nenhuma etapa depende de análise textual improvisada em substituição à AST.

## Fase 3 — Semântica, tipos e IR (meta acumulada: 50%)

**Estado: Fase 3 concluída — 108 testes aprovados, 0 falhas; CI verde.**

[Ver validação completa no GitHub Actions](https://github.com/korczaktech/Nebula/actions/runs/41373497754).

**Objetivo:** dar significado e consistência aos programas.

Entregas implementadas:
- Tabelas de símbolos e escopos léxicos, com pré-declaração de funções e aliases.
- Resolução de nomes e de funções exportadas por módulos fornecidos explicitamente.
- Sistema de tipos estáticos, incluindo larguras inteiras, decimais e listas homogêneas.
- Inferência local controlada.
- Verificação de chamadas, operadores, condições, retornos, mutabilidade e controle de laços.
- Regras iniciais de memória: constantes imutáveis e valores por valor; sem borrow checker nesta fase.
- Construção da IR com operadores normalizados.
- Interpretador de referência com limite de passos e profundidade de chamadas.
- Testes semânticos, de IR e integração da CLI.

Critérios de conclusão:
- Programas corretos passam pela análise.
- Programas com tipos ou nomes inválidos são rejeitados.
- IR representa os recursos já aprovados da linguagem.
- Testes validam o comportamento da análise e da representação intermediária.

## Fase 4 — Compilação nativa própria (meta acumulada: mínimo de 75%)

**Estado: em reestruturação arquitetural; ainda não concluída.**

**Objetivo:** transformar a IR em executáveis nativos sem gerar outra linguagem e sem depender de compilador, assembler ou linker externo.

Direção arquitetural aprovada:
- Compilador e backend implementados em Rust.
- Geração direta de instruções de máquina e estrutura de executável pelo backend.
- Primeiro alvo: Linux x86-64, formato ELF64.
- CLI grava o executável produzido pelo backend e pode executá-lo.
- Testes do backend escritos em Rust e testes de integração executam os binários gerados.
- Nenhuma dependência obrigatória de C, C++, LLVM, Cranelift, assembler ou linker externo.
- O otimizador próprio será implementado somente depois que os recursos-base do compilador estiverem prontos.
- O runtime e a biblioteca padrão não serão antecipados; serão implementados apenas na etapa definida pelo plano.

Implementação inicial em andamento:
- Backend Rust que escreve ELF64 e código x86-64 diretamente.
- Suporte inicial a valores inteiros e lógicos, variáveis locais, funções, parâmetros, chamadas, recursão, expressões aritméticas, comparações, condicionais e laços.
- Entrada de processo e encerramento pelo syscall Linux, sem runtime Nebula externo.
- CLI sem chamada a `c++` e sem opções de otimização delegadas.

Limites atuais e explícitos:
- O alvo inicial é exclusivamente Linux x86-64.
- Strings, decimais, listas, indexação, entrada/saída e biblioteca padrão aguardam suas etapas próprias; o backend rejeita esses recursos em vez de gerar código fictício.
- Ainda é necessário ampliar e validar o conjunto de instruções, ABI, escopos, chamadas, tratamento de erros e limites de código/stack.
- Não há otimizador nem níveis `-O0` a `-Os`.
- O runtime Nebula não foi implementado nesta alteração.
- O benchmark é apenas uma linha de base sem otimizações; não comprova superioridade sobre C/C++.

Critérios detalhados e estado verificável: [critérios da Fase 4](especificacao/criterios-fase-4.md).

## Fase 5 — Consolidação e conclusão (de 75%+ até 100%)

**Objetivo:** completar, endurecer e preparar a primeira versão estável do ecossistema.

Entregas previstas:
- Completar recursos da linguagem ainda pendentes.
- Expandir e estabilizar o sistema de tipos.
- Completar regras de memória, propriedade e concorrência.
- Ampliar backends e arquiteturas suportadas.
- Completar biblioteca padrão planejada para a versão.
- Melhorar otimizações com evidências de benchmark.
- Melhorar compilação incremental e experiência de desenvolvimento.
- Completar formatador, testes e ferramentas oficiais.
- Documentação de linguagem, CLI e biblioteca.
- Testes de compatibilidade, segurança, regressão e desempenho.
- Empacotamento e distribuição.
- Auditoria da especificação e remoção de inconsistências.
- Definição de critérios para estabilidade e compatibilidade.
- Avaliação da autocompilação como objetivo posterior ou entrega experimental, conforme maturidade.

Critérios de conclusão:
- Todos os recursos declarados como estáveis estão implementados e testados.
- Nenhuma funcionalidade anunciada como pronta permanece apenas simulada.
- Documentação corresponde ao comportamento real.
- Testes obrigatórios passam nos alvos oficialmente suportados.
- Benchmarks e limitações estão publicados.
- A versão 1.0 possui escopo, compatibilidade e processo de distribuição definidos.

**Marco final:** 100% do escopo aprovado para a versão 1.0. Recursos futuros fora desse escopo não serão usados para impedir artificialmente a conclusão.

---

## 13. Política de progresso

O progresso será acompanhado por entregas verificáveis, não por estimativas subjetivas.

Para cada item:
- **Não iniciado:** ainda não existe implementação.
- **Em desenvolvimento:** implementação parcial.
- **Implementado:** código presente, ainda aguardando validação.
- **Testado:** testes relevantes aprovados.
- **Concluído:** implementação, testes e documentação aprovados.

Uma fase não será concluída apenas porque os arquivos foram criados ou porque o código compila. O critério é o comportamento funcional comprovado.

## 14. Objetivo final

Nebula deverá se tornar uma linguagem brasileira, independente, eficiente e de propósito geral, capaz de atender desde pequenos programas até aplicações empresariais, servidores, sistemas operacionais e softwares de alto desempenho.

**Nebula — uma linguagem própria, construída para fazer tudo.**


## Estado atual da Fase 3

A Fase 3 está concluída. A Fase 4 está em reestruturação para gerar ELF x86-64 diretamente em Rust, sem compilador externo; runtime e otimizador permanecem adiados. A CI mais recente validou 121 testes acumulados e executou a linha de base de benchmark sem otimizações. Consulte [critérios da Fase 3](especificacao/criterios-fase-3.md) e [critérios da Fase 4](especificacao/criterios-fase-4.md) para limites e evidências.
