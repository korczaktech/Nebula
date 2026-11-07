# Nebula

> Linguagem de programação própria da Korczak Technologies.

Nebula é uma linguagem de propósito geral, com sintaxe original em português brasileiro, tipagem estática, inferência controlada e modelo híbrido de memória. Seu projeto busca combinar desempenho, legibilidade, segurança, portabilidade e controle de baixo nível.

| Propriedade | Definição |
|---|---|
| Nome | Nebula |
| Extensão | `.nbx` |
| Idioma da linguagem | Português brasileiro, conforme as regras léxicas oficiais |
| Implementação do compilador | Rust |
| Modelo | Compilada, multiparadigma e multiplataforma |
| Tipagem | Estática, forte e com inferência controlada |
| Memória | Modelo híbrido |
| Alvos | Nativo e WebAssembly, em evolução |
| Repositório | [korczaktech/Nebula](https://github.com/korczaktech/Nebula) |
| Extensão VS Code | [Nebula-Extension](https://github.com/korczaktech/Nebula-Extension) |

A meta histórica de investigar desempenho de até 3× C++ é experimental e restrita a cargas específicas. Não representa uma garantia. Qualquer comparação deverá utilizar benchmarks públicos, reproduzíveis e equivalentes.

---

## 1. Objetivos

Nebula foi concebida para permitir a criação de:

- Sistemas operacionais, kernels, drivers e sistemas embarcados.
- Aplicativos desktop, móveis e web.
- APIs, serviços, backends e sistemas distribuídos.
- Jogos, motores gráficos e ferramentas de desenvolvimento.
- Bibliotecas, infraestrutura, automação e aplicações científicas.
- Componentes de baixo nível com controle explícito de memória e hardware.

### Princípios de projeto

1. Sintaxe própria: Nebula não é uma tradução de Python, C, C++ ou JavaScript.
2. Desempenho mensurável e previsível.
3. Regras léxicas, sintáticas e semânticas consistentes.
4. Diagnósticos claros, com códigos estáveis e localização precisa.
5. Tipos explícitos quando necessários e inferência quando segura.
6. Controle de memória adequado tanto a aplicações quanto a sistemas.
7. Biblioteca padrão modular.
8. Portabilidade sem esconder diferenças entre plataformas.
9. Capacidade de compilar código freestanding e bare-metal conforme os backends evoluírem.
10. Ferramentas oficiais integradas ao fluxo de desenvolvimento.

## 2. Sintaxe e identidade

A especificação normativa está organizada em `especificacao/`. Ela define a linguagem; o código do compilador e os testes determinam quais partes já estão implementadas.

Características centrais:

- Arquivos de código com extensão `.nbx`.
- Fonte em UTF-8.
- Palavras-chave e APIs com identidade em português.
- Identificadores Unicode tratados conforme as regras de normalização da especificação.
- Indentação consistente para legibilidade.
- Delimitadores e encerramentos definidos pela gramática oficial.
- Comentários, operadores, declarações, funções, tipos e estruturas de controle com regras próprias.

Exemplo ilustrativo da sintaxe documentada:

```nebula
funcao principal() -> vazio: {
    variavel nome = "Nebula"
    constante idade: inteiro = 18

    quando idade >= 18: {
        exibir("Ola, " + nome)

    }
    senao: {
        exibir("Bem-vindo")

    }


}
```

O exemplo demonstra a organização sintática. A disponibilidade de cada função depende da biblioteca, do runtime e do alvo utilizado.

**Importante:** existem documentos históricos e complementares de sintaxe no projeto. Em caso de divergência, deve prevalecer o contrato normativo mais recente em `especificacao/`, acompanhado pelos testes de conformidade do compilador. Documentos antigos não tornam automaticamente uma construção válida.

## 3. Arquitetura do compilador

O compilador é desenvolvido em Rust e dividido em componentes independentes.

```text
Fonte .nbx
   |
   v
Lexer e tokens
   |
   v
Parser
   |
   v
AST
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
Backend e alvo
   |
   +----> Executável nativo
   |
   +----> WebAssembly
```

### Componentes

| Componente | Responsabilidade |
|---|---|
| Fonte | Leitura, validação e localização do código-fonte |
| Diagnósticos | Erros, avisos, códigos e sugestões |
| Sintaxe | Lexer, Parser, tokens e AST |
| Semântica | Escopos, resolução de nomes e validações |
| IR | Representação intermediária independente de alvo |
| Backend | Geração de código para arquiteturas suportadas |
| CLI | Interface de compilação e execução |
| Runtime | Serviços necessários durante a execução |
| Pacotes | Organização e resolução de dependências |
| Segurança | Verificações e mecanismos de proteção |
| Cache | Reutilização de resultados de compilação |
| LSP | Integração com editores e ferramentas de linguagem |
| Biblioteca | APIs públicas e módulos reutilizáveis |

O workspace Rust e seus membros estão definidos no `Cargo.toml` da raiz. A arquitetura não exige LLVM como dependência do compilador.

### Lexer, Parser e AST

O Lexer transforma caracteres em tokens e identifica palavras reservadas, identificadores, números, textos, operadores, comentários e separadores. O Parser valida a gramática e constrói a AST. A AST representa a estrutura do programa sem depender diretamente da plataforma de destino.

A suíte de testes de conformidade deve cobrir tanto entradas válidas quanto inválidas, incluindo diagnósticos obrigatórios, recuperação de erros e localização no código-fonte.

### Análise semântica e tipos

A análise semântica deverá verificar:

- Declarações e resolução de identificadores.
- Escopos e visibilidade.
- Compatibilidade e inferência de tipos.
- Fluxo de controle e retornos.
- Uso de módulos e funções.
- Regras de propriedade, referências e vida útil.
- Operações permitidas em contextos de baixo nível.

O sistema de tipos deverá contemplar tipos primitivos, compostos, definidos pelo usuário, genéricos, opcionais, funções e tipos específicos de sistemas.

### Representação intermediária e otimização

A IR é a fronteira entre a semântica da linguagem e a geração de código. Os backends devem receber operações tipadas e explícitas, sem redefinir o comportamento da linguagem.

Níveis de otimização previstos:

| Nível | Objetivo |
|---|---|
| O0 | Compilação com otimização mínima |
| O1 | Simplificações conservadoras |
| O2 | Otimizações intermediárias |
| O3 | Otimizações avançadas |
| Os | Redução do tamanho do código |

Toda transformação deve preservar a semântica, respeitar efeitos colaterais e possuir testes de correção. Recursos descritos como planejados não devem ser confundidos com otimizações já implementadas.

## 4. Memória e runtime

Nebula adota um modelo híbrido de memória:

- Gerenciamento automático quando adequado.
- Controle explícito para aplicações de sistemas.
- Possibilidade de execução sem runtime obrigatório.
- Alocadores configuráveis conforme o ambiente.
- Regras de propriedade e vida útil destinadas a evitar referências inválidas.
- Operações de baixo nível com contratos explícitos de segurança.

O runtime Rust existente reúne recursos como valores dinâmicos, escopos léxicos, constantes imutáveis, coleções compartilhadas protegidas, operações matemáticas verificadas, conversões numéricas, manipulação Unicode, arquivos, diretórios, relógio, tarefas, canais, atomics e cancelamento cooperativo.

A biblioteca pública organiza funcionalidades em módulos. A presença de uma API no runtime não significa que ela esteja disponível em todos os backends ou perfis de compilação.

### Concorrência

A arquitetura prevê:

- Tarefas e execução assíncrona.
- Threads e paralelismo.
- Canais e comunicação entre tarefas.
- Operações atômicas.
- Sincronização e cancelamento cooperativo.
- Regras de compartilhamento seguro de dados.

## 5. Biblioteca padrão

A biblioteca deverá ser modular, com dependências explícitas e sem carregar componentes desnecessários.

| Módulo | Responsabilidade |
|---|---|
| Núcleo | Tipos e operações essenciais |
| Matemática | Funções matemáticas |
| Texto | Strings, Unicode e manipulação textual |
| Coleções | Listas, vetores, mapas e estruturas |
| Arquivos | Arquivos e diretórios |
| Rede | Comunicação e protocolos |
| Tempo | Datas, horários e temporizadores |
| Sistema | Informações e serviços do ambiente |
| Memória | Alocação e controle de memória |
| Paralelo | Concorrência e sincronização |
| Segurança | Operações criptográficas e proteção |
| Dados | Serialização e formatos |
| Interface | Recursos de interface de usuário |
| Gráficos | Computação e renderização gráfica |
| Banco | Integração com bancos de dados |
| Hardware | Recursos específicos de dispositivos e sistemas |

A lista é o catálogo arquitetural. Cada módulo deve indicar sua situação real: implementado, experimental ou planejado.

## 6. Backends e portabilidade

### Código nativo

O objetivo é gerar executáveis nativos para arquiteturas como x86-64 e ARM64, com suporte progressivo a Linux, Windows, macOS e sistemas embarcados.

### WebAssembly

O alvo WebAssembly deverá permitir execução em navegadores e ambientes compatíveis, com interfaces controladas para comunicação com o ambiente hospedeiro.

### Freestanding e bare-metal

A arquitetura deverá permitir programas sem runtime de sistema hospedeiro, com ponto de entrada, memória, ABI, interrupções, ligação e operações de hardware definidas pelo alvo.

O suporte a uma plataforma só deve ser anunciado como funcional após backend, runtime quando aplicável, testes e validação específicos.

## 7. Diagnósticos

Os diagnósticos oficiais devem ser escritos em português e possuir:

- Código estável.
- Gravidade.
- Descrição da causa.
- Arquivo, linha e coluna.
- Trecho relacionado.
- Sugestão de correção quando possível.

Exemplo de formato:

```text
Erro N001 — Identificador desconhecido
Arquivo: exemplo.nbx
Localização: 12:8
O identificador não foi declarado neste escopo.
Sugestão: declare-o antes de utilizá-lo.
```

## 8. CLI oficial

A interface de linha de comando oferece operações de desenvolvimento. Os comandos disponíveis devem ser consultados com `nebula --help`.

Formas de uso previstas e documentadas:

```sh
nebula criar
nebula compilar arquivo.nbx
nebula executar arquivo.nbx
nebula interpretar arquivo.nbx
nebula verificar arquivo.nbx
nebula formatar arquivo.nbx
nebula testar
nebula analisar arquivo.nbx
nebula depurar --dap arquivo.nbx
nebula dependencias
nebula limpar
nebula versao
```

Nem todos os comandos ou alvos possuem necessariamente o mesmo nível de implementação. A execução interpretada pela IR é diferente da geração e execução de um binário nativo.

## 9. Integração com ferramentas

O projeto mantém uma extensão oficial para Visual Studio Code no repositório [Nebula-Extension](https://github.com/korczaktech/Nebula-Extension).

A integração pode incluir:

- Reconhecimento de arquivos `.nbx`.
- Realce de sintaxe.
- Snippets e configurações de edição.
- Autocomplete e informações contextuais.
- Diagnósticos e navegação.
- Language Server Protocol.
- Compilação, interpretação e execução.
- Depuração por DAP, conforme os recursos disponíveis.

## 10. Organização do repositório

A organização é dividida por responsabilidade:

- `compilador/`: crates e componentes do compilador.
- `biblioteca/`: APIs e módulos públicos.
- `especificacao/`: contratos normativos e decisões da linguagem.
- `testes/`: testes unitários, de integração e conformidade.
- `exemplos/`: programas de demonstração.
- `benchmarks/`: medições reproduzíveis.
- `.github/workflows/`: automação de validação.

A estrutura real da branch principal é a referência para os caminhos existentes.

## 11. Qualidade e critérios de evolução

Uma funcionalidade só pode ser considerada concluída quando:

1. Possuir implementação integrada ao fluxo correto.
2. Tiver testes positivos e negativos.
3. Produzir diagnósticos adequados quando aplicável.
4. Respeitar a especificação.
5. Não quebrar funcionalidades existentes.
6. Passar pelas verificações de formatação, lint e testes do workspace.
7. Tiver documentação correspondente.
8. Possuir benchmark quando houver alegação de desempenho.

A CI deve validar os componentes relevantes e impedir que falhas conhecidas sejam tratadas como sucesso.

## 12. Licença e autoria

Nebula é um projeto da Korczak Technologies. Consulte `LICENSE` e `LICENSE-STATUS.md` para conhecer os termos aplicáveis, inclusive as condições de uso e redistribuição.

---

**Nebula — linguagem, compilador e ecossistema de desenvolvimento da Korczak Technologies.**
