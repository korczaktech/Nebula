# Nebula — Especificação da Sintaxe Abrasileirada

**Extensão:** `.nbx`  
**Compilador:** Nebula Compiler (Rust)  
**Idioma:** Português brasileiro, sem acentos na sintaxe  
**Paradigma:** Multiparadigma  
**Tipagem:** Estática, forte e com inferência  
**Memória:** Gerenciamento híbrido  
**Blocos:** Indentação obrigatória e encerramento explícito  
**Codificação:** UTF-8

> Documento temporário de referência para a definição da sintaxe Nebula. A presença de uma construção neste documento não significa que ela já esteja implementada no compilador.

---

# 1. Identidade e regras de nomenclatura

Nebula é uma linguagem de programação independente. Sua sintaxe utiliza português brasileiro abrasileirado, com nomes intuitivos, curtos e consistentes. A linguagem aproveita conceitos conhecidos de Python e JavaScript, mas não é uma tradução literal dessas linguagens.

Regras:

1. Priorizar palavras em português brasileiro.
2. Não utilizar acentos em palavras reservadas, identificadores e nomes da biblioteca padrão.
3. Utilizar uma palavra sempre que possível.
4. Quando forem necessárias duas palavras, utilizar camelCase: primeira palavra minúscula e inicial da segunda palavra maiúscula.
5. Evitar nomes excessivamente longos.
6. Evitar termos em inglês quando houver alternativa portuguesa clara.
7. Preservar termos técnicos universais quando sua tradução prejudicar a clareza, como `lambda`.
8. Não criar sinônimos redundantes para a mesma operação.
9. Manter a mesma nomenclatura no Lexer, Parser, biblioteca padrão, documentação e ferramentas.

Exemplos: `lerTexto()`, `criarArquivo()`, `removerItem()`, `verificarTipo()`, `aguarda`, `retorne`, `assinc`.

# 2. Regras obrigatórias da sintaxe

A sintaxe é obrigatória e não pode ser ignorada pelo compilador.

## 2.1. Delimitadores

| Símbolo | Finalidade |
|---|---|
| `()` | Chamadas, parâmetros e agrupamentos |
| `[]` | Listas, índices e vetores |
| `{}` | Objetos e estruturas delimitadas |
| `<>` | Parâmetros e declarações de tipos genéricos |
| `:` | Anotações de tipo e separação de propriedades |
| `->` | Tipo de retorno e funções de seta |
| `.` | Acesso a propriedades e métodos |
| `,` | Separação de elementos |

Os delimitadores `()`, `[]`, `<>` e `{}` são obrigatórios nos contextos em que a gramática os exige. Não precisam aparecer todos em cada instrução: cada um possui seu contexto sintático próprio.

## 2.2. Indentação obrigatória

A indentação é obrigatória em todos os blocos. O padrão recomendado é de quatro espaços por nível. O compilador deve rejeitar indentação ambígua e definir de forma consistente o tratamento de tabulações.

```nbx
se idade >= 18
    exibir("Maior de idade")

    se idade >= 60
        exibir("Idoso")
    fim
fim
```

## 2.3. Encerramento obrigatório

Todo bloco iniciado por uma estrutura de controle, função, classe ou declaração de bloco deve ser encerrado por `fim`.

```nbx
funcao executar()
    exibir("Executando")
fim
```

## 2.4. Estrutura geral

```nbx
variavel nome: texto = "Nebula"

funcao principal()
    se nome == "Nebula"
        exibir("Bem-vindo")
    senao
        exibir("Desconhecido")
    fim
fim

principal()
```

# 3. Palavras reservadas

| Conceito de referência | Nebula |
|---|---|
| if | `se` |
| elif / else if | `sendo` |
| else | `senao` |
| for | `para` |
| while | `enquanto` |
| switch / match | `escolha` |
| case | `caso` |
| default | `padrao` |
| break | `pare` |
| continue | `prossiga` |
| pass | `passar` |
| return | `retorne` |
| function / def | `funcao` |
| lambda | `lambda` |
| class | `classe` |
| import | `incluir` |
| from | `de` |
| as | `como` |
| yield | `ceder` |
| try | `tente` |
| except / catch | `erro` |
| finally | `sempre` |
| raise / throw | `lancar` |
| assert | `verifique` |
| with | `com` |
| async | `assinc` |
| await | `aguarda` |
| global | `global` |
| nonlocal | `externa` |
| this / self | `isto` |
| new | `novo` |
| delete | `apagar` |
| in | `dentro` |
| not in | `fora` |
| is | `identico` |
| not | `nao` |
| and | `e` |
| or | `ou` |
| true | `sim` |
| false | `nao` |
| null / None | `nulo` |
| let / var | `variavel` |
| const | `constante` |
| extends | `herda` |
| static | `estatico` |
| private | `privado` |
| public | `publico` |
| protected | `protegido` |
| export | `exportar` |
| constructor | `construtor` |
| typeof | `tipoDe` |
| instanceof | `instanciaDe` |
| get | `obter` |
| set | `definir` |
| void | `vazio` |

Observações:
- `nao` representa negação lógica e o valor lógico falso, conforme o contexto.
- `pare` interrompe o laço atual; `fim` encerra um bloco.
- `prossiga` pula para a próxima repetição.
- `isto` referencia a instância atual de uma classe.
- `sendo` é reservado para condições alternativas.

# 4. Condicionais

## 4.1. `se`

Executa um bloco quando a condição é verdadeira.

```nbx
se idade >= 18
    exibir("Maior de idade")
fim
```

## 4.2. `sendo`

Representa uma condição alternativa.

```nbx
se nota >= 9
    exibir("Excelente")
sendo nota >= 7
    exibir("Aprovado")
sendo nota >= 5
    exibir("Recuperacao")
senao
    exibir("Reprovado")
fim
```

## 4.3. `senao`

Executa quando nenhuma condição anterior for satisfeita.

```nbx
se ativo == sim
    exibir("Ativo")
senao
    exibir("Inativo")
fim
```

## 4.4. `escolha`

Seleciona uma opção entre várias possibilidades.

```nbx
escolha opcao
    caso 1
        exibir("Iniciar")
    caso 2
        exibir("Configuracoes")
    caso 3
        exibir("Sair")
    padrao
        exibir("Opcao invalida")
fim
```

# 5. Repetições

## 5.1. `para`

Percorre uma sequência.

```nbx
para numero dentro faixa(0, 10)
    exibir(numero)
fim
```

## 5.2. `enquanto`

Repete enquanto uma condição for verdadeira.

```nbx
variavel contador: inteiro = 0

enquanto contador < 10
    exibir(contador)
    contador =+ 1
fim
```

## 5.3. `pare`

Interrompe completamente o laço atual.

```nbx
para numero dentro faixa(0, 100)
    se numero == 10
        pare
    fim
fim
```

## 5.4. `prossiga`

Pula a repetição atual.

```nbx
para numero dentro faixa(0, 10)
    se numero == 5
        prossiga
    fim
    exibir(numero)
fim
```

## 5.5. `passar`

Não executa nenhuma operação. Serve como instrução vazia explícita.

```nbx
funcao futura()
    passar
fim
```

# 6. Variáveis e constantes

## 6.1. Variáveis

```nbx
variavel idade: inteiro = 20
variavel nome: texto = "Raphael"

idade = 25
```

## 6.2. Constantes

```nbx
constante PI: decimal = 3.141592
constante VERSAO: texto = "1.0"
```

Uma constante não pode receber nova atribuição depois de inicializada.

## 6.3. Inferência de tipos

```nbx
variavel nome = "Nebula"
variavel quantidade = 100
variavel ativo = sim
```

O compilador identifica os tipos automaticamente quando a expressão permite inferência segura.

# 7. Tipos de dados

| Tipo | Descrição |
|---|---|
| `inteiro` | Números inteiros |
| `decimal` | Números decimais |
| `complexo` | Números complexos |
| `texto` | Texto Unicode |
| `logico` | Valores lógicos |
| `lista` | Coleção ordenada e mutável |
| `tupla` | Coleção ordenada e imutável |
| `conjunto` | Coleção sem duplicatas |
| `conImutavel` | Conjunto imutável |
| `dicionario` | Coleção de chave e valor |
| `octetos` | Sequência imutável de bytes |
| `vetorOctetos` | Sequência mutável de bytes |
| `bit` | Unidade de um bit |
| `vetorBits` | Coleção de bits |
| `vazio` | Ausência de retorno |
| `nulo` | Ausência de valor |

Exemplos:

```nbx
variavel idade: inteiro = 20
variavel altura: decimal = 1.80
variavel nome: texto = "Nebula"
variavel ativo: logico = sim

variavel numeros: lista = [10, 20, 30]
variavel coordenadas: tupla = (10, 20)
variavel valores: conjunto = conjunto([1, 2, 3])

variavel dados: dicionario = {
    "nome": "Nebula",
    "versao": 1
}
```

# 8. Operadores matemáticos

| Operador / função | Finalidade |
|---|---|
| `+` | Adição |
| `-` | Subtração |
| `*` | Multiplicação |
| `/` | Divisão |
| `\\` | Divisão inteira |
| `%` | Resto da divisão |
| `**` | Potenciação |
| `^` | XOR binário |
| `-x` | Negação numérica |
| `+x` | Valor positivo |
| `abs()` | Valor absoluto |
| `arredondar()` | Arredondamento |
| `divres()` | Quociente e resto |

```nbx
variavel soma = 10 + 5
variavel resto = 10 % 3
variavel potencia = 2 ** 8
variavel resultado = divres(10, 3)
```

# 9. Operadores de comparação

| Operador | Significado |
|---|---|
| `==` | Igualdade |
| `iguais` | Igualdade estrita, conforme definição de tipos |
| `!=` | Diferença |
| `!==` | Diferença estrita |
| `>` | Maior |
| `<` | Menor |
| `>=` | Maior ou igual |
| `<=` | Menor ou igual |

# 10. Operadores lógicos

| Operador | Função |
|---|---|
| `e` | E lógico |
| `ou` | OU lógico |
| `nao` | Negação lógica |
| `&&` | Forma simbólica de E |
| `||` | Forma simbólica de OU |
| `!` | Forma simbólica de negação |

```nbx
se idade >= 18 e autorizado == sim
    exibir("Acesso permitido")
fim
```

# 11. Atribuições

| Operador | Função |
|---|---|
| `=` | Atribuição |
| `=+` | Soma e atribui |
| `=-` | Subtrai e atribui |
| `=*` | Multiplica e atribui |
| `=/` | Divide e atribui |
| `=\\` | Divide inteiramente e atribui |
| `=%` | Calcula o resto e atribui |
| `=**` | Potencia e atribui |
| `++` | Incrementa |
| `--` | Decrementa |

Também poderão ser aceitos os aliases convencionais `+=`, `-=`, `*=`, `/=`, `//= `, `%=` e `**=`, desde que a gramática formal os confirme.

# 12. Funções

## 12.1. Declaração

```nbx
funcao somar(a: inteiro, b: inteiro) -> inteiro
    retorne a + b
fim
```

## 12.2. Parâmetros padrão

```nbx
funcao saudar(nome: texto = "Visitante")
    exibir("Ola, " + nome)
fim
```

## 12.3. Parâmetros nomeados

```nbx
funcao criarUsuario(nome: texto, idade: inteiro)
    exibir(nome)
    exibir(idade)
fim

criarUsuario(nome = "Ana", idade = 25)
```

## 12.4. Expressões lambda

```nbx
variavel dobrar = lambda x: x * 2
```

## 12.5. Funções de seta

O operador oficial de seta é `->`.

```nbx
variavel somar = (a, b) -> a + b
variavel quadrado = x -> x ** 2
```

Para funções com bloco:

```nbx
variavel calcular = (a, b) -> {
    variavel resultado = a + b
    retorne resultado
}
```

## 12.6. `ceder`

Produz valores incrementalmente, permitindo a construção de geradores.

```nbx
funcao contar()
    para numero dentro faixa(1, 6)
        ceder numero
    fim
fim
```

# 13. Classes e objetos

## 13.1. Declaração

```nbx
classe Pessoa
    funcao construtor(nome: texto, idade: inteiro)
        isto.nome = nome
        isto.idade = idade
    fim

    funcao apresentar()
        exibir(isto.nome)
    fim
fim
```

## 13.2. Instanciação

```nbx
variavel pessoa = novo Pessoa("Raphael", 20)
pessoa.apresentar()
```

## 13.3. Herança

```nbx
classe Animal
    funcao emitirSom()
        exibir("Som")
    fim
fim

classe Cachorro herda Animal
    funcao latir()
        exibir("Au au")
    fim
fim
```

## 13.4. Visibilidade e membros

- `publico`: acesso público.
- `privado`: acesso restrito à classe.
- `protegido`: acesso à classe e às subclasses.
- `estatico`: membro associado à classe.

# 14. Importações e exportações

O comando principal é `incluir`. A palavra `biblioteca` é opcional.

## 14.1. Importar uma biblioteca

```nbx
incluir biblioteca matematica
incluir matematica
```

## 14.2. Importar um módulo

```nbx
incluir modulo arquivos
incluir arquivos
```

## 14.3. Importar um componente

```nbx
incluir raiz de matematica
```

## 14.4. Criar apelidos

```nbx
incluir biblioteca matematica como mat
exibir(mat.raiz(25))
```

## 14.5. Exportar

```nbx
exportar funcao somar(a, b)
    retorne a + b
fim
```

# 15. Tratamento de erros

## 15.1. Bloco de tratamento

```nbx
tente
    variavel numero = inteiro("abc")
erro ValorInvalido
    exibir("Valor incorreto")
sempre
    exibir("Finalizado")
fim
```

## 15.2. Lançar erros

```nbx
lancar erro "Operacao invalida"
```

## 15.3. Verificações

```nbx
verifique idade >= 18
verifique idade >= 18, "Idade insuficiente"
```

# 16. Escopo

| Palavra | Finalidade |
|---|---|
| `global` | Acessar uma variável global |
| `externa` | Acessar uma variável de função externa |
| `isto` | Referenciar a instância atual |

Exemplo:

```nbx
variavel contador = 0

funcao incrementar()
    global contador
    contador =+ 1
fim
```

# 17. Programação assíncrona

## 17.1. `assinc`

Declara uma função assíncrona.

```nbx
assinc funcao buscarDados()
    exibir("Buscando...")
fim
```

## 17.2. `aguarda`

Aguarda a conclusão de uma operação assíncrona.

```nbx
assinc funcao principal()
    variavel resultado = aguarda buscarDados()
    exibir(resultado)
fim
```

# 18. Entrada, saída e arquivos

| Comando | Finalidade |
|---|---|
| `exibir()` | Exibir informações |
| `lerTexto()` | Receber texto |
| `abrir()` | Abrir arquivo |
| `fechar()` | Fechar arquivo |
| `ler()` | Ler conteúdo |
| `lerLinha()` | Ler uma linha |
| `lerLinhas()` | Ler várias linhas |
| `escrever()` | Escrever conteúdo |
| `escreverLinhas()` | Escrever várias linhas |

Exemplo:

```nbx
variavel nome = lerTexto("Seu nome: ")
exibir("Ola, " + nome)
```

## 18.1. Arquivos

```nbx
variavel arquivo = abrir("dados.txt", "leitura")
variavel conteudo = ler(arquivo)
fechar(arquivo)
```

Modos previstos:

| Modo | Função |
|---|---|
| `leitura` | Leitura |
| `escrita` | Escrita |
| `anexar` | Acrescentar conteúdo |
| `binario` | Modo binário |
| `leituraEscrita` | Leitura e escrita |

## 18.2. Fechamento automático

```nbx
com abrir("dados.txt", "leitura") como arquivo
    variavel conteudo = ler(arquivo)
    exibir(conteudo)
fim
```

# 19. Métodos de texto

| Método | Função |
|---|---|
| `.maiusculo()` | Converte para maiúsculas |
| `.minusculo()` | Converte para minúsculas |
| `.capitalizar()` | Capitaliza o texto |
| `.dividir()` | Divide texto |
| `.juntar()` | Junta textos |
| `.substituir()` | Substitui conteúdo |
| `.localizar()` | Procura texto |
| `.contar()` | Conta ocorrências |
| `.comecaCom()` | Verifica prefixo |
| `.terminaCom()` | Verifica sufixo |
| `.removerEspacos()` | Remove espaços externos |
| `.centralizar()` | Centraliza texto |

# 20. Métodos de listas

| Método | Função |
|---|---|
| `.adicionar()` | Adiciona elemento |
| `.estender()` | Adiciona vários elementos |
| `.inserir()` | Insere em posição |
| `.remover()` | Remove elemento |
| `.retirar()` | Remove e retorna elemento |
| `.limpar()` | Limpa lista |
| `.contar()` | Conta ocorrências |
| `.indice()` | Localiza elemento |
| `.ordenar()` | Ordena |
| `.inverter()` | Inverte |
| `.copiar()` | Copia |

# 21. Métodos de dicionários

| Método | Função |
|---|---|
| `.obter()` | Obtém valor |
| `.chaves()` | Retorna chaves |
| `.valores()` | Retorna valores |
| `.itens()` | Retorna pares |
| `.atualizar()` | Atualiza conteúdo |
| `.remover()` | Remove chave |
| `.limpar()` | Limpa |
| `.copiar()` | Copia |

# 22. Funções nativas

As funções abaixo compõem a proposta de biblioteca padrão. Cada função precisa ter assinatura, comportamento, regras de erro e testes definidos antes de ser considerada implementada.

## 22.1. Matemática

| Função | Descrição |
|---|---|
| `abs()` | Valor absoluto |
| `arredondar()` | Arredondamento |
| `divres()` | Divisão e resto |
| `raiz()` | Raiz quadrada |
| `potencia()` | Potenciação |
| `piso()` | Arredondar para baixo |
| `teto()` | Arredondar para cima |
| `truncar()` | Remover parte decimal |
| `minimo()` | Menor valor |
| `maximo()` | Maior valor |
| `somar()` | Somar valores |

## 22.2. Conversão

| Função | Descrição |
|---|---|
| `inteiro()` | Converter para inteiro |
| `decimal()` | Converter para decimal |
| `complexo()` | Converter para complexo |
| `texto()` | Converter para texto |
| `logico()` | Converter para lógico |
| `lista()` | Criar lista |
| `tupla()` | Criar tupla |
| `conjunto()` | Criar conjunto |
| `conImutavel()` | Criar conjunto imutável |
| `dicionario()` | Criar dicionário |
| `octetos()` | Criar sequência de bytes |
| `vetorOctetos()` | Criar sequência mutável de bytes |
| `tipoDe()` | Identificar tipo |

## 22.3. Coleções

| Função | Descrição |
|---|---|
| `faixa()` | Gerar intervalo |
| `tamanho()` | Obter quantidade |
| `ordenar()` | Ordenar |
| `inverter()` | Inverter |
| `enumerar()` | Enumerar |
| `agrupar()` | Combinar sequências |
| `filtrar()` | Filtrar elementos |
| `mapear()` | Transformar elementos |
| `reduzir()` | Reduzir coleção |

## 22.4. Memória

| Função | Descrição |
|---|---|
| `visualizarMemoria()` | Inspecionar memória |
| `enderecoDe()` | Obter endereço |
| `tamanhoDe()` | Obter tamanho em bytes |
| `alinhamentoDe()` | Obter alinhamento |
| `identidade()` | Identificar objeto |

## 22.5. Texto

| Função | Descrição |
|---|---|
| `dividir()` | Dividir texto |
| `juntar()` | Juntar elementos |
| `substituir()` | Substituir conteúdo |
| `maiusculo()` | Converter para maiúsculas |
| `minusculo()` | Converter para minúsculas |
| `localizar()` | Procurar conteúdo |
| `contar()` | Contar ocorrências |

## 22.6. Sistema

| Função | Descrição |
|---|---|
| `diretorioAtual()` | Obter diretório |
| `listarArquivos()` | Listar arquivos |
| `criarDiretorio()` | Criar diretório |
| `removerArquivo()` | Remover arquivo |
| `existe()` | Verificar existência |
| `executar()` | Executar operação permitida |

# 23. Operadores adicionais

| Operador | Finalidade |
|---|---|
| `? :` | Expressão condicional |
| `??` | Valor alternativo para nulo |
| `?.` | Acesso opcional |
| `??=` | Atribuição quando nulo |
| `...` | Expansão de coleções |
| `&` | E binário |
| `|` | OU binário |
| `^` | OU exclusivo binário |
| `~` | Negação binária |
| `<<` | Deslocamento à esquerda |
| `>>` | Deslocamento à direita |
| `>>>` | Deslocamento à direita sem sinal |

# 24. Texto e cadeias de caracteres

## 24.1. Aspas

```nbx
variavel nome = "Nebula"
variavel outro = 'Nebula'
```

## 24.2. Interpolação

```nbx
variavel nome = "Raphael"
exibir(`Ola, ${nome}`)
```

## 24.3. Múltiplas linhas

```nbx
variavel texto = """
Primeira linha
Segunda linha
Terceira linha
"""
```

## 24.4. Caracteres especiais

| Código | Função |
|---|---|
| `\\n` | Nova linha |
| `\\t` | Tabulação |
| `\\r` | Retorno |
| `\\\\` | Barra invertida |
| `\\"` | Aspas duplas |
| `\\'` | Aspas simples |
| `\\0` | Caractere nulo |

# 25. Comentários

## 25.1. Uma linha

```nbx
# Comentario
```

## 25.2. Múltiplas linhas

```nbx
/*
Comentario de varias linhas.
*/
```

# 26. Precedência preliminar dos operadores

A tabela abaixo é uma proposta inicial. A gramática formal deverá confirmar a precedência, associatividade e comportamento de cada operador.

| Prioridade | Operadores |
|---|---|
| 1 | `()`, `[]`, `.`, `?.` |
| 2 | `**` |
| 3 | `+x`, `-x`, `~`, `!`, `nao` |
| 4 | `*`, `/`, `\\`, `%` |
| 5 | `+`, `-` |
| 6 | `<<`, `>>`, `>>>` |
| 7 | `&` |
| 8 | `^` |
| 9 | `|` |
| 10 | `<`, `>`, `<=`, `>=`, `dentro`, `fora` |
| 11 | `==`, `!=`, `iguais`, `!==` |
| 12 | `e` |
| 13 | `ou` |
| 14 | `??` |
| 15 | `? :` |
| 16 | Operadores de atribuição |
| 17 | `->` em contextos de expressão de seta |

# 27. Exemplo completo

```nbx
# Cadastro de usuarios

classe Usuario
    funcao construtor(nome: texto, idade: inteiro)
        isto.nome = nome
        isto.idade = idade
    fim

    funcao apresentar()
        exibir(`Nome: ${isto.nome}`)
        exibir(`Idade: ${isto.idade}`)
    fim
fim

funcao verificarIdade(idade: inteiro) -> logico
    retorne idade >= 18
fim

funcao principal()
    variavel nome = lerTexto("Digite seu nome: ")
    variavel idade = inteiro(lerTexto("Digite sua idade: "))

    variavel usuario = novo Usuario(nome, idade)
    usuario.apresentar()

    se verificarIdade(idade)
        exibir("Cadastro autorizado")
    senao
        exibir("Cadastro nao autorizado")
    fim
fim

principal()
```

# 28. Correspondência conceitual com Python e JavaScript

| Python | JavaScript | Nebula |
|---|---|---|
| `print()` | `console.log()` | `exibir()` |
| `input()` | `prompt()` | `lerTexto()` |
| `if` | `if` | `se` |
| `elif` | `else if` | `sendo` |
| `else` | `else` | `senao` |
| `for` | `for` | `para` |
| `while` | `while` | `enquanto` |
| `break` | `break` | `pare` |
| `continue` | `continue` | `prossiga` |
| `pass` | — | `passar` |
| `return` | `return` | `retorne` |
| `def` | `function` | `funcao` |
| `lambda` | Arrow function | `lambda` |
| `class` | `class` | `classe` |
| `import` | `import` | `incluir` |
| `from` | `from` | `de` |
| `as` | `as` | `como` |
| `try` | `try` | `tente` |
| `except` | `catch` | `erro` |
| `finally` | `finally` | `sempre` |
| `raise` | `throw` | `lancar` |
| `yield` | `yield` | `ceder` |
| `async` | `async` | `assinc` |
| `await` | `await` | `aguarda` |
| `True` | `true` | `sim` |
| `False` | `false` | `nao` |
| `None` | `null` | `nulo` |
| `and` | `&&` | `e` |
| `or` | `||` | `ou` |
| `not` | `!` | `nao` |
| `str` | `String` | `texto` |
| `int` | `Number` | `inteiro` |
| `bool` | `Boolean` | `logico` |
| `list` | `Array` | `lista` |
| `dict` | `Object` | `dicionario` |

# 29. Gramática formal preliminar

A gramática abaixo é um ponto de partida para a especificação EBNF definitiva. Ela não substitui a definição completa de expressões, tipos, delimitadores, indentação e regras semânticas.

```ebnf
programa       = { declaracao | instrucao } ;

declaracao     = variavel
               | constante
               | funcao
               | classe
               | importacao ;

variavel       = "variavel", identificador,
                 [":", tipo], "=", expressao ;

constante      = "constante", identificador,
                 [":", tipo], "=", expressao ;

funcao         = ["assinc"], "funcao",
                 identificador, "(",
                 [parametros], ")",
                 ["->", tipo],
                 bloco, "fim" ;

classe         = "classe", identificador,
                 ["herda", identificador],
                 bloco, "fim" ;

condicional    = "se", expressao,
                 bloco,
                 {"sendo", expressao, bloco},
                 ["senao", bloco],
                 "fim" ;

repeticao      = "para", identificador,
                 "dentro", expressao,
                 bloco, "fim"
               | "enquanto", expressao,
                 bloco, "fim" ;

selecao        = "escolha", expressao,
                 {"caso", expressao, bloco},
                 ["padrao", bloco],
                 "fim" ;

retorno        = "retorne", [expressao] ;

interrupcao    = "pare"
               | "prossiga"
               | "passar" ;

importacao     = "incluir",
                 ["biblioteca" | "modulo"],
                 identificador,
                 ["como", identificador] ;

bloco          = INDENT, { instrucao | declaracao }, DEDENT ;
```

# 30. Compilação e execução

O compilador deve reconhecer a sintaxe e transformá-la em código executável.

Etapas previstas:

1. Análise léxica.
2. Análise sintática.
3. Construção da AST.
4. Análise semântica.
5. Verificação de tipos.
6. Geração da representação intermediária.
7. Otimização.
8. Geração de código.
9. Execução.

Comandos de terminal propostos:

```bash
nebula compilar programa.nbx
nebula executar programa.nbx
nebula interpretar programa.nbx
```

# 31. Regras consolidadas

1. Português brasileiro como idioma principal.
2. Ausência de acentos na sintaxe.
3. Nomes curtos e intuitivos.
4. CamelCase para nomes compostos.
5. Indentação obrigatória.
6. `fim` obrigatório para encerramento de blocos.
7. `()`, `[]`, `<>` e `{}` obrigatórios nos respectivos contextos gramaticais.
8. Tipagem estática e forte.
9. Inferência de tipos.
10. Gerenciamento híbrido de memória.
11. Suporte previsto a funções, classes, coleções e assincronismo.
12. Compilador independente de editor.
13. Implementação nativa em Rust.
14. Nenhuma funcionalidade deve ser declarada implementada sem verificação.

# 32. Pontos que exigem fechamento formal

Antes de considerar a sintaxe congelada e implementar todas as construções, é necessário definir:

- Precedência e associatividade exatas de todos os operadores.
- Diferença semântica entre `==`, `iguais`, `!=` e `!==`.
- Se `nao` pode representar simultaneamente falso e negação sem ambiguidade no parser.
- Forma definitiva de atribuição composta: `=+` ou `+=`, e equivalentes.
- Tipos numéricos, limites, conversões e comportamento de overflow.
- Tipos genéricos e uso obrigatório de `<>`.
- Regras de objetos, blocos com `{}` e indentação.
- Semântica de referências, propriedade, empréstimo e gerenciamento híbrido de memória.
- Herança, visibilidade, métodos estáticos e construtores.
- Exceções, tipos de erro e propagação.
- Modelo de tarefas assíncronas e geradores.
- Assinaturas e efeitos das funções da biblioteca padrão.
- Compatibilidade freestanding/bare-metal e ABI para uso no ZenithOS.

---

**Nebula — Especificação temporária da sintaxe abrasileirada.**

Este documento define a sintaxe-alvo proposta. A implementação no compilador deverá seguir a especificação formal aprovada e manter testes de conformidade para cada construção.