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
| Alvos iniciais | Código nativo e WebAssembly |

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

As decisões abaixo formam a base inicial da especificação. A gramática será formalizada e validada antes de ser considerada estável.

- Código-fonte em UTF-8.
- Identificadores Unicode normalizados em NFC.
- Identificadores sensíveis a maiúsculas e minúsculas.
- Organização visual flexível: espaços, quebras de linha, indentação e delimitadores podem ser combinados.
- Chaves `{}` podem delimitar blocos; parênteses `()` podem agrupar condições, parâmetros e chamadas; colchetes `[]` podem organizar coleções.
- Delimitadores não devem ser exigidos quando a estrutura já estiver clara e puder ser interpretada sem ambiguidade.
- Ponto e vírgula `;` é opcional quando a separação das instruções já estiver clara.
- Aspas delimitam literais de texto; dentro de uma string, regras próprias de escape serão definidas.
- Arquivos `.nbx` como unidades de código independentes.
- Não é obrigatório declarar um projeto ou programa no início de cada arquivo.
- Palavras-chave e mensagens oficiais em português, combinadas com termos próprios do Nebulês.
- Operadores familiares combinados com operadores característicos da linguagem.

### Sintaxe visual e delimitadores flexíveis

A Nebula prioriza a leitura natural e a organização visual. Símbolos como parênteses, chaves, colchetes e ponto e vírgula servem para separar e agrupar partes do código, mas não devem ser obrigatórios quando a intenção já estiver clara.

Princípios propostos:

- **Parênteses `()`:** agrupam expressões, condições, parâmetros e argumentos quando isso melhora a leitura ou evita ambiguidades.
- **Chaves `{}`:** delimitam blocos de funções, condicionais e outras estruturas quando forem úteis para deixar os limites explícitos.
- **Colchetes `[]`:** delimitam listas, vetores e acessos por índice.
- **Aspas `""`:** delimitam literais de texto; não são removidas quando necessárias para distinguir texto de identificadores ou expressões.
- **Ponto e vírgula `;`:** pode separar instruções, mas é dispensável quando quebras de linha ou a própria gramática já fazem essa separação.
- **Espaços, linhas em branco e indentação:** podem ser usados livremente para melhorar a organização visual, sem criar regras artificiais de formatação.

A gramática deverá aceitar formas compactas e formas mais explícitas sempre que ambas forem inequívocas. O compilador não deverá confundir preferência estética com exigência semântica: delimitadores são ferramentas de clareza, não decoração obrigatória.

Exemplo conceitual — a sintaxe ainda será consolidada na especificação formal:

```nebula
funcao principal(): vazio {
    variavel nome = "Raphael"
    variavel idade = 18

    quando (idade >= 18) {
        exibir("Olá, " + nome)
    }
}
```

A mesma estrutura poderá admitir uma apresentação mais compacta, caso a gramática consiga identificar os limites sem ambiguidade. O exemplo não representa ainda uma gramática final.

### Vocabulário e operadores em estudo

Palavras candidatas: `variavel`, `constante`, `funcao`, `incluir`, `quando`, `senao`, `avaliar`, `caso`, `para`, `enquanto`, `cada`, `receber`, `devolver`, `exibir`, `encerrar`, `aguardar`, `parar` e `seguir`.

Operadores candidatos: `=`, `==`, `!=`, `>=`, `<=`, `??`, `..`, `**`, `::` e `~>`.

Essas listas são propostas de trabalho, não uma promessa de que todos os itens já estejam implementados ou definitivamente aprovados.

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

Responsável por ler o código-fonte e produzir tokens.

Deverá reconhecer palavras reservadas, identificadores, números, textos, caracteres, operadores, separadores opcionais, comentários e quebras de linha. O reconhecimento de blocos não poderá depender exclusivamente de indentação obrigatória.

Requisitos:
- Reconhecimento de operadores pela maior correspondência.
- Preservação de arquivo, linha, coluna e intervalo de origem.
- Suporte a UTF-8 e identificadores Unicode.
- Preservação de quebras de linha e espaços relevantes para a separação de instruções, sem tornar a indentação obrigatória.
- Diagnósticos para caracteres e sequências inválidas.

### 3.2 Parser

Transforma tokens em estruturas sintáticas válidas.

Responsabilidades:
- Validar a gramática.
- Construir a AST.
- Detectar estruturas incompletas e inválidas.
- Recuperar-se de erros quando possível, permitindo mais de um diagnóstico por execução.

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

O projeto não será definido pelo LLVM. LLVM poderá ser adotado como infraestrutura opcional para determinados backends, sem tornar-se a especificação da linguagem nem uma dependência obrigatória de todos os alvos.

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

A estrutura inicial já criada na Fase 1 é:

```text
nebula/
├── README.md
├── CONTRIBUTING.md
├── CHANGELOG.md
├── Cargo.toml
├── rust-toolchain.toml
├── .gitignore
├── .github/
│   └── workflows/
│       └── ci.yml
├── especificacao/
│   ├── linguagem.md
│   ├── decisoes.md
│   ├── gramatica.md
│   ├── tokens.md
│   ├── tipos.md
│   ├── memoria.md
│   ├── operadores.md
│   ├── modulos.md
│   ├── diagnosticos.md
│   └── alvos.md
├── compilador/
│   ├── fonte/
│   ├── diagnosticos/
│   └── cli/
├── exemplos/
│   └── ola_mundo.nbx
├── testes/
│   └── conformidade/
│       ├── README.md
│       └── sintaxe_visual.nbx
└── CHANGELOG.md
```

Os diretórios de Lexer, Parser, AST, semântica, IR e backends serão adicionados nas fases correspondentes. Não existem pastas vazias declaradas como componentes implementados.

### Crates Rust

**Criadas na Fase 1:**
- `nebula-fonte`: arquivo-fonte, intervalos e posições.
- `nebula-diagnosticos`: estrutura de diagnóstico independente da interface.
- `nebula-cli`: comandos informativos iniciais (`ajuda` e `versao`).

**Planejadas para fases posteriores:**
- `nebula-lexer`
- `nebula-parser`
- `nebula-ast`
- `nebula-semantica`
- `nebula-tipos`
- `nebula-ir`
- `nebula-otimizador`
- `nebula-backend`
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

**Estado: fundação implementada; validação automatizada em andamento.**

**Objetivo:** estabelecer a identidade técnica e preparar a base reproduzível do projeto.

Entregas realizadas:
- Especificação inicial da linguagem, tokens, gramática, tipos, memória, operadores, módulos, diagnósticos, alvos e registro de decisões.
- Regras de separação visual flexível, com delimitadores opcionais e indentação sem significado semântico.
- Workspace Cargo com Rust estável e edição 2024.
- Crates iniciais `nebula-fonte`, `nebula-diagnosticos` e `nebula-cli`.
- Tipos básicos de fonte, intervalos, linha/coluna e diagnósticos.
- CLI informativa com ajuda e versão, sem alegar compilação de arquivos.
- Exemplo .nbx e fixtures de conformidade.
- CI para formatação, Clippy e testes.
- Política de contribuição, progresso e changelog.

Critérios de conclusão:
- A estrutura e os contratos iniciais estão registrados.
- O código-base está preparado para validação automatizada.
- A gramática documenta explicitamente as ambiguidades ainda pendentes.
- A confirmação final de execução do CI será registrada separadamente; não declarar validação verde antes do resultado do GitHub Actions.

## Fase 2 — Frontend da linguagem (meta acumulada: 30%)

**Objetivo:** transformar código Nebula em uma representação sintática confiável.

Entregas:
- Leitor de arquivos e posições de origem.
- Lexer completo para a gramática aprovada.
- Parser.
- AST.
- Tratamento e recuperação de erros sintáticos.
- Diagnósticos em português.
- Testes positivos, negativos e de regressão.
- CLI inicial para verificar arquivos.

Critérios de conclusão:
- Arquivos de exemplo são tokenizados e analisados corretamente.
- Erros indicam localização precisa.
- Testes cobrem tokens, precedência, estruturas gramaticais e variações válidas de espaçamento e delimitadores.
- Nenhuma etapa depende de análise textual improvisada em substituição à AST.

## Fase 3 — Semântica, tipos e IR (meta acumulada: 50%)

**Objetivo:** dar significado e consistência aos programas.

Entregas:
- Tabelas de símbolos e escopos.
- Resolução de nomes e módulos.
- Sistema de tipos estáticos.
- Inferência controlada.
- Verificação semântica.
- Regras iniciais de propriedade e memória.
- Construção da IR.
- Interpretador de IR para testes, quando adequado.
- Testes semânticos e de IR.

Critérios de conclusão:
- Programas corretos passam pela análise.
- Programas com tipos ou nomes inválidos são rejeitados.
- IR representa os recursos já aprovados da linguagem.
- Testes validam o comportamento da análise e da representação intermediária.

## Fase 4 — Compilação executável e desempenho inicial (meta acumulada: mínimo de 75%)

**Objetivo:** transformar o frontend em uma ferramenta capaz de gerar e executar programas reais.

Entregas:
- Pipeline completo de compilação.
- Backend nativo inicial para pelo menos uma plataforma definida.
- Backend WebAssembly inicial, se viável dentro do escopo validado.
- Linkedição e execução de programas simples.
- Otimizador inicial com níveis documentados.
- CLI funcional: compilar, executar, verificar e testar.
- Biblioteca padrão essencial inicial.
- Tratamento de erros de compilação e ligação.
- Testes ponta a ponta.
- Primeiros benchmarks reproduzíveis e relatório de limitações.

Critérios de conclusão:
- Um programa Nebula de ponta a ponta compila e executa.
- Variáveis, expressões, funções e controle de fluxo definidos no MVP funcionam.
- Artefatos gerados são executáveis no alvo declarado.
- Testes de regressão e integração passam.
- O projeto atinge pelo menos 75% do escopo definido para a primeira versão utilizável.

**Marco:** ao final da Fase 4, Nebula deverá ser um compilador utilizável em seu escopo inicial, não apenas um protótipo de Lexer e Parser.

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
