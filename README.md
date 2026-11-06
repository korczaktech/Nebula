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
| Blocos | Function Body obrigatório: `: { ... fim }` |
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

## 2. Identidade e regras da linguagem

A gramática e as regras léxicas normativas estão em [especificacao/gramatica.md](especificacao/gramatica.md) e [especificacao/tokens.md](especificacao/tokens.md). Esses documentos são a referência do Lexer, Parser e AST do crate `nebula-sintaxe`.

- Código-fonte UTF-8; identificadores Unicode comparados em NFC.
- Palavras-chave e mensagens oficiais em português brasileiro.
- Todo corpo executável ou estrutural abre com `: {`, termina com `fim` antes de `}`, e recebe outro `fim` após a chave quando a construção for completa.
- A indentação é obrigatória para legibilidade e deve ser consistente, mas não delimita blocos.
- Chaves de dados são distintas de Function Body; dicionários em declarações comuns não exigem `fim`.
- Instruções são separadas por quebra de linha ou ponto e vírgula, conforme as regras da gramática.
- Quebras dentro de parênteses e colchetes são continuações; regras específicas também permitem expressões multilinha.
- A lista de palavras reservadas implementadas e a lista de palavras reservadas futuras estão em [especificacao/tokens.md](especificacao/tokens.md).

Exemplo da sintaxe atual:

~~~nebula
funcao principal() -> vazio: {
    variavel nome = "Nebula"
    constante idade: inteiro = 18

    quando idade >= 18: {
        exibir("Olá, " + nome)
        fim
    }
    senao: {
        exibir("Olá")
        fim
    }
    fim
    fim
} fim
~~~

O exemplo demonstra a sintaxe; chamadas como `exibir` dependem do runtime/biblioteca padrão e do alvo utilizado.

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
| O0 | Preserva a IR |
| O1 | Dobramento conservador de constantes |
| O2 | Simplificação de ramos constantes e remoção de laços comprovadamente inexecutáveis |
| O3 | Remove instruções posteriores a transferências incondicionais |
| Os | Aplica O3 e remove blocos vazios |

Transformações atualmente implementadas: dobramento seguro de constantes, simplificação de fluxo constante, remoção de laços comprovadamente inexecutáveis, remoção de instruções inalcançáveis e blocos vazios. Eliminação de cópias redundantes, otimização de chamadas e especialização ainda não estão implementadas.

Nenhuma otimização poderá alterar o comportamento definido pela linguagem. Cada transformação deverá possuir testes de correção e, quando relevante, benchmarks.

## 4. Memória e execução

Nebula adotará um modelo híbrido:

- Gestão automática quando for segura e adequada.
- Controle explícito para sistemas, drivers, infraestrutura e código de baixo nível.
- Propriedade, empréstimo e verificação estática planejados para reduzir referências inválidas, cópias e acessos inseguros.
- Alocadores e estratégias configuráveis conforme o ambiente.

O crate `nebula-runtime` já fornece valores dinâmicos, escopos léxicos, constantes imutáveis, listas compartilhadas com proteção por locks, operações inteiras/decimais e matemáticas verificadas, conversões numéricas, texto Unicode (recorte, busca, prefixos, sufixos, substituição e normalização de espaços), leitura, escrita, cópia, movimentação e remoção de arquivos, consulta e listagem determinística de diretórios, relógio e suspensão, tarefas em threads, canais, inteiros atômicos e cancelamento cooperativo. O crate `nebula-biblioteca` organiza as APIs reais implementadas em módulos públicos e oferece cliente TCP síncrono com timeout e limite de leitura. O interpretador de referência da IR usa esse runtime por meio de builtins semântica e execução. Isso ainda não equivale a um borrow checker, a um modelo completo de propriedade ou à integração do runtime ao backend ELF nativo.

Não há exigência de um coletor de lixo global em todos os programas. O runtime deverá ser opcional sempre que possível, especialmente em código de baixo nível.

### Concorrência

A linguagem deverá oferecer tarefas assíncronas, paralelismo, sincronização, operações atômicas, comunicação entre tarefas e compartilhamento seguro de dados. Recursos de runtime serão incluídos apenas quando necessários ao programa.

## 5. Módulos e biblioteca padrão

A inclusão de módulos será explícita e modular:

```nebula
incluir(biblioteca: matematica)
incluir(biblioteca: arquivos)
incluir(biblioteca: rede)
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
nebula interpretar
nebula depurar --dap <arquivo.nbx>
nebula dependencias
nebula limpar
nebula versao
```

O comando `compilar` aceita `-O0`, `-O1`, `-O2`, `-O3` e `-Os`. O comando `interpretar` executa a IR pelo runtime Rust de referência; `executar` continua usando o backend nativo Linux x86-64. O comando `depurar --dap` inicia o adaptador Debug Adapter Protocol para depuração pelo VS Code, com breakpoints por linha, pilha de chamadas, escopos, variáveis, continuar, avançar, step-over, step-out e pausa. A seleção de plataformas e arquiteturas adicionais ainda não está implementada.

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
│   ├── runtime/
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

Lexer, Parser e AST estão reunidos em `nebula-sintaxe`; análise semântica, tipos e IR estão em `nebula-semantica` e `nebula-ir`. O backend nativo próprio da Fase 4 gera ELF x86-64 diretamente em Rust para o subconjunto documentado; implementação e critérios de aceite foram validados pela CI em Linux x86-64.

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

**Criadas na Fase 4 e Fase 5:**
- `nebula-backend`: geração direta de executáveis ELF64 Linux x86-64 em Rust, com instruções de máquina próprias, chamadas System V AMD64, variáveis locais e globais em segmentos ELF separados (código RX/dados RW), controle de fluxo, aritmética inteira e potência por quadratura.
- `nebula-runtime`: valores dinâmicos, memória por escopos, listas compartilhadas, operações numéricas verificadas, operações Unicode de texto, operações de arquivos e diretórios, tempo, tarefas, canais e inteiros atômicos concorrentes. Integrado ao interpretador de referência da IR.
- `nebula-ir::otimizador`: níveis O0, O1, O2, O3 e Os, com passes conservadores e métricas de transformações.

O comando `nebula compilar` publica o executável atomicamente; falhas não deixam arquivos parciais nem substituem uma saída anterior. O backend ELF x86-64 liga chamadas qualificadas a funções de módulos locais. O comando `nebula interpretar` executa a IR pelo runtime de referência e carrega os mesmos módulos recursivamente. O carregador resolve imports aninhados recursivamente, valida os identificadores dos caminhos, bloqueia travessia de diretórios, liga chamadas qualificadas entre módulos e rejeita ciclos de dependência. Módulos importados ainda não aceitam inicializadores globais. O crate `nebula-pacotes` lê manifestos `nebula.toml`, resolve dependências locais transitivas e gera `nebula.lock`; dependências de registro remoto e cache ainda não estão disponíveis. O backend ELF nativo ainda não liga o runtime nem a biblioteca padrão.

**Ainda planejadas:**
- Expansão da biblioteca padrão para os módulos ainda não implementados
- Registro remoto, cache e verificação criptográfica de pacotes
- Backends para ARM64, Windows, macOS e WebAssembly

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

## Fase 4 — Compilação nativa própria (escopo inicial concluído)

**Estado: CONCLUÍDA E VALIDADA.** CI aprovada no commit `e3a2ad338f96b832bd652876e220114f8bd53858`: `cargo fmt`, `cargo clippy`, 169 testes e benchmark inicial.

**Objetivo:** transformar a IR em executáveis nativos sem gerar outra linguagem e sem depender de compilador, assembler ou linker externo.

Entregas implementadas no alvo inicial:
- Backend Rust que gera diretamente instruções x86-64 e executáveis ELF64 para Linux.
- Segmento de código RX, segmento de dados globais RW separado e pilha NX.
- Funções, parâmetros System V AMD64 (até seis argumentos), chamadas aninhadas e recursão.
- Variáveis locais e globais, escopos, sombreamento, condicionais e laços.
- Aritmética signed de 64 bits, comparações, lógica de curto-circuito e potência por quadratura.
- Detecção de overflow em soma, subtração, multiplicação, negação, divisão, resto e potência; falhas aritméticas em tempo de execução encerram com código 2.
- Validação de alcance de saltos relativos e relocação RIP-relative para dados globais.
- CLI que grava executáveis atomicamente e não invoca compiladores, assemblers ou linkers externos.
- Testes Rust unitários e de integração que executam os binários gerados.

Limites explícitos desta etapa:
- O único alvo nativo implementado é Linux x86-64 / ELF64.
- Inteiros de 8, 16, 32 e 128 bits, byte, strings, decimais, listas, indexação, caracteres Unicode e entrada/saída aguardam suas etapas próprias.
- Módulos e chamadas qualificadas ainda não são ligados pelo backend.
- Na conclusão da Fase 4, runtime Nebula, biblioteca padrão e otimizador ainda estavam adiados; o runtime e o otimizador foram iniciados posteriormente na Fase 5.
- O benchmark é uma linha de base sem otimizações; não comprova superioridade sobre C/C++.

A conclusão foi confirmada pela aprovação de formatação, Clippy, 169 testes e benchmark no GitHub Actions. Consulte [critérios da Fase 4](especificacao/criterios-fase-4.md) e a [execução aprovada](https://github.com/korczaktech/Nebula/actions/runs/41392238233).


## Fase 5 — Consolidação e conclusão (de 75%+ até 100%)

**Critérios integrais de aceite:** [especificacao/criterios-fase-5.md](especificacao/criterios-fase-5.md). O documento define o escopo solicitado para a versão 1.0 e os portões de validação; não significa que os recursos já estejam implementados.

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

A Fase 3 e a Fase 4 estão concluídas dentro de seus escopos documentados. Na Fase 5, o runtime Rust está integrado ao interpretador de referência; o otimizador O0/O1/O2/O3/Os está implementado na IR e selecionável pela CLI; e a CLI oferece `interpretar`. O sistema de tipos já percorre estruturas, enumerações, tuplas, opcionais, resultados, funções genéricas e valores de função com chamadas indiretas. O backend ELF x86-64 também gera chamadas indiretas para assinaturas escalares, representa caracteres Unicode por pontos de código, implementa operações SSE2 de decimal32/decimal64 e conversões numéricas explícitas entre inteiros de até 64 bits e decimais. Ainda faltam, entre outros critérios, ABI nativa completa (os formatos iniciais de agregados e inteiros de 128 bits já são suportados parcialmente), borrow checker completo, expansão da biblioteca padrão, backends adicionais, integração do runtime ao backend nativo e os demais requisitos de estabilidade 1.0. A CI valida somente os recursos cobertos pelos jobs executados; isso não significa que a Fase 5 esteja concluída.
