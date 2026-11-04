# Comandos, estruturas e ferramentas da Nebula

**Escopo:** inventário da sintaxe e das ferramentas que existem no repositório. Um recurso disponível no interpretador não é automaticamente suportado pelo backend nativo.

## 1. Instruções da linguagem

| Recurso | Forma | Comportamento |
|---|---|---|
| Exibição | `exibir(valor)` | Envia valores para a saída do interpretador |
| Entrada | `receber([prompt])`, `perguntar([prompt])`, `ler_texto()` | Lê uma linha da entrada padrão; o prompt é opcional |
| Variável | `variavel nome = valor` | Declara variável mutável |
| Variável (alias) | `definir nome = valor`, `mutavel nome = valor`, `mutável nome = valor` | Mesmo comportamento da declaração de variável |
| Constante | `constante nome = valor` | Declara valor imutável |
| Atribuição | `nome = valor`, `nome <- valor` | Atualiza variável mutável |
| Alteração explícita | `alterar nome <- valor` | Executa uma atribuição; a palavra não permite contornar as regras de mutabilidade |
| Condicional | `quando condição { ... } senao { ... }` | Escolhe um bloco com base em uma condição lógica |
| Correspondência | `escolha valor { caso ... { ... } }` | Examina variantes de enumeração, Opcional ou Resultado |
| Correspondência (alias) | `avaliar valor { caso ... { ... } }` | Mesmo comportamento de `escolha` |
| Laço condicional | `enquanto condição { ... }` | Repete enquanto a condição for verdadeira |
| Laço (aliases) | `durante condição { ... }`, `repetir condição { ... }` | Mesmo comportamento de `enquanto` |
| Laço contado | `para (início; condição; atualização) { ... }` | Executa inicialização, condição e atualização |
| Iteração | `cada item em coleção { ... }` | Percorre listas, textos e intervalos numéricos |
| Ciclo periódico | `pulso(milissegundos) { ... }` | Repete o bloco com pausa entre ciclos |
| Retorno | `devolver valor` | Retorna valor de uma função |
| Interrupção | `parar` | Sai do laço mais próximo |
| Interrupção (alias) | `encerrar` | Mesmo comportamento de `parar` |
| Próxima iteração | `seguir` | Pula para a próxima iteração do laço |
| Importação | `incluir Modulo` | Inclui módulo local conforme as regras do resolvedor |
| Espera | `aguardar(ms)`, `dormir(ms)` | Suspende a execução pelo tempo informado em milissegundos |
| Geração de coleção | `gerar(valor, quantidade)` | Cria uma lista repetindo o valor informado |
| Paralelismo | `em_paralelo(funcao1, funcao2, ...)` | Executa funções sem parâmetros em threads independentes |

As declarações de funções, aliases de tipo, estruturas e enumerações também fazem parte da linguagem:

```nebula
funcao somar(a: inteiro, b: inteiro): inteiro {
    devolver a + b
}

estrutura Pessoa {
    nome: texto
    idade: inteiro
}

enumeracao Estado {
    Ativo
    Inativo
}
```

## 2. Tipos e coleções

| Tipo | Significado |
|---|---|
| `vazio` | Ausência de valor de retorno |
| `lógico` / `logico` | Valor verdadeiro ou falso |
| `inteiro` | Número inteiro |
| `inteiro8`, `inteiro16`, `inteiro32`, `inteiro64`, `inteiro128` | Inteiros com largura explícita |
| `inteiro_u8`, `inteiro_u16`, `inteiro_u32`, `inteiro_u64`, `inteiro_u128` | Inteiros sem sinal |
| `decimal`, `decimal32`, `decimal64` | Números de ponto flutuante |
| `texto` | Cadeia de caracteres Unicode |
| `caractere` | Escalar Unicode |
| `byte` | Unidade de 8 bits |
| `[T]` / `vetor<T>` | Lista homogênea |
| `matriz<T>` | Lista de listas homogêneas (`[[T]]`) |
| `Tupla<T1, T2, ...>` | Coleção heterogênea de tamanho fixo |
| `Opcional<T>` | Valor presente ou ausente |
| `Resultado<T, E>` | Resultado de sucesso ou erro |
| `funcao(T1, T2): R` | Tipo de função |

`mutável` é uma palavra usada na declaração de variável, não um tipo de dado. Os valores lógicos são `verdadeiro` e `falso`; `nulo` representa ausência de valor em contextos que o aceitam.

## 3. Operadores

| Grupo | Operadores |
|---|---|
| Atribuição | `=`, `<-` |
| Aritmética | `+`, `-`, `*`, `/`, `%`, `**` |
| Igualdade | `==`, `!=` |
| Comparação | `<`, `<=`, `>`, `>=` |
| Lógica | `e`, `ou`, `nao`, `!` |
| Intervalo | `..` |
| Acesso | `.`, `::` |

## 4. Ferramentas de terminal

Os comandos abaixo são executados no terminal; eles não são instruções que se escrevem dentro de um arquivo `.nbx`.

| Comando | Finalidade |
|---|---|
| `nebula ajuda` | Exibe a ajuda |
| `nebula versao` | Exibe a versão |
| `nebula criar <diretório>` | Cria projeto |
| `nebula verificar <arquivo>` | Valida sintaxe; aceita `--json` |
| `nebula formatar <arquivo>` | Formata o arquivo; aceita `--check` |
| `nebula analisar <arquivo>` | Faz análise semântica; aceita `--json` |
| `nebula ir <arquivo>` | Exibe a representação intermediária |
| `nebula compilar <arquivo>` | Gera executável para alvo implementado |
| `nebula interpretar <arquivo>` | Executa pelo interpretador Rust de referência |
| `nebula executar <arquivo>` | Compila e executa o executável nativo |
| `nebula testar <arquivo>` | Executa teste de fumaça por compilação e execução |
| `nebula alvos` | Lista alvos que possuem backend real |
| `nebula limpar <arquivo>` | Remove o executável padrão do arquivo |
| `nebula limpar-cache <projeto>` | Remove artefatos do cache incremental |
| `nebula dependencias resolver [diretório]` | Resolve dependências locais e grava lockfile |
| `nebula dependencias verificar [diretório]` | Verifica o lockfile |

## 5. Disponibilidade por modo de execução

- O Lexer, Parser e analisador semântico reconhecem as declarações, aliases, tipos e operadores descritos acima.
- O interpretador de referência executa os builtins de entrada, espera, geração de listas, iteração, ciclos periódicos e paralelismo que estão descritos neste inventário.
- O backend ELF Linux x86-64 não possui integração completa do runtime. Os novos nós de IR `Cada` e `Pulso` são rejeitados explicitamente pelo backend; os builtins `gerar` e `em_paralelo` ainda não são uma interface nativa disponível.
- `em_paralelo` copia os escopos da memória para cada tarefa. Células escalares ficam isoladas, mas listas e estruturas usam valores internos compartilhados com locks; alterações nesses objetos podem ser vistas por outras tarefas. As alterações locais de variáveis escalares não são mescladas no contexto chamador. Os resultados são devolvidos na ordem dos argumentos.
- O backend atualmente implementado é Linux x86-64 / ELF64. A existência de jobs de CI em Windows e macOS valida o workspace Rust, não a execução nativa de programas Nebula nessas plataformas.

## 6. Nomes que ainda não são comandos nem tipos

Os termos `órbita`, `fluxo`, `nexo`, `elo`, `prisma`, `núcleo` e `sinal` apareceram como ideias de organização de dados ou componentes, mas ainda não possuem definição formal de sintaxe, tipo, regras semânticas ou comportamento observável. Por isso, não são palavras reservadas nem recursos implementados. Não devem ser confundidos com `vetor` e `matriz`, que possuem aliases de tipo definidos.

## 7. Exemplos executáveis

Entrada e saída:

```nebula
variavel nome = receber("Seu nome: ")
exibir("Olá, " + nome)
```

Iteração e geração:

```nebula
variavel valores = gerar(5, 3)
cada valor em valores {
    exibir(valor)
}
```

Escolha com tipo opcional:

```nebula
variavel resposta: Opcional<inteiro> = Opcional::Algum(42)
avaliar resposta {
    caso Opcional::Algum(valor) { exibir(valor) }
    caso Opcional::Nenhum { exibir(0) }
}
```

Ciclo periódico:

```nebula
variavel ciclos = 0
pulso(100) {
    alterar ciclos <- ciclos + 1
    quando ciclos >= 3 { encerrar }
}
exibir(ciclos)
```
