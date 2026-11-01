# Especificação inicial da linguagem Nebula

**Estado:** proposta-base da Fase 1. Mudanças incompatíveis exigem atualização desta especificação e dos testes de conformidade.

## 1. Identidade

- Nome: Nebula.
- Extensão de fonte: `.nbx`.
- Linguagem e mensagens oficiais: português brasileiro.
- Compilador inicial: Rust.
- Paradigma: multiparadigma, compilada e de propósito geral.
- Tipagem: estática por padrão, com inferência controlada.
- Alvos previstos: código nativo e WebAssembly.
- Arquivos-fonte: UTF-8, sem BOM obrigatório.

## 2. Princípios normativos

As palavras **DEVE**, **NÃO DEVE**, **PODE** e **RECOMENDA-SE** indicam, respectivamente, requisito obrigatório, proibição, opção permitida e recomendação.

1. O significado de um programa não depende da quantidade de espaços ou da indentação.
2. Delimitadores visuais são aceitos quando ajudam a agrupar; não são exigidos se a estrutura puder ser reconhecida sem ambiguidade.
3. Uma quebra de linha pode separar instruções completas. O ponto e vírgula também pode fazê-lo explicitamente.
4. O compilador deve rejeitar construções ambíguas em vez de adivinhar a intenção.
5. A sintaxe oficial usa palavras-chave em português e operadores familiares.
6. A atribuição usa `=`; igualdade usa `==`; comparação não deve ser confundida com atribuição.
7. Erros devem apontar localização de origem e oferecer mensagem em português.
8. A especificação é independente da implementação e dos backends.

## 3. Organização visual

- `()`: agrupamento de expressões, parâmetros e argumentos; podem ser omitidos apenas quando a gramática mantiver o significado inequívoco.
- `{}`: delimitadores explícitos de blocos; blocos também podem usar o terminador `fim`.
- `[]`: listas, vetores e indexação.
- `""`: literais de texto; aspas são obrigatórias para texto literal.
- `;`: separador explícito opcional entre instruções.
- Espaços, linhas em branco e indentação são livres para apresentação e não definem escopos.
- Quebras de linha separam instruções quando o token anterior pode encerrar uma instrução e o próximo token pode iniciar outra.
- Dentro de parênteses e colchetes, quebras de linha são espaços, salvo regras específicas de literal.

Exemplos conceitualmente equivalentes:

```nebula
variavel total = 10
variavel taxa = 2
exibir(total * taxa)
```

```nebula
variavel total = 10; variavel taxa = 2; exibir(total * taxa);
```

Bloco com chaves:

```nebula
funcao principal(): vazio {
    exibir("Olá, Nebula")
}
```

Bloco com terminador:

```nebula
funcao principal(): vazio
    exibir("Olá, Nebula")
fim
```

A indentação não altera o escopo em nenhum dos exemplos.

## 4. Identificadores

- Sensíveis a maiúsculas e minúsculas.
- Codificados em UTF-8.
- A especificação adota normalização Unicode NFC para comparação de identificadores.
- Um identificador começa por letra Unicode ou sublinhado; os caracteres seguintes podem incluir letras, marcas combinantes, números e sublinhado.
- Palavras reservadas não podem ser usadas como identificadores sem mecanismo de escape, ainda não definido.
- Recomenda-se usar nomes em português nos exemplos e APIs oficiais.

## 5. Palavras reservadas iniciais

`variavel`, `constante`, `funcao`, `quando`, `senao`, `para`, `enquanto`, `cada`, `em`, `devolver`, `incluir`, `como`, `verdadeiro`, `falso`, `nulo`, `vazio`, `fim`, `parar`, `seguir`, `e`, `ou`, `nao`, `tentar`, `capturar`, `finalmente`, `assinc`, `aguardar`, `publico`, `privado`, `tipo`, `estrutura`, `enumeracao`.

A lista é reservada para a primeira versão da gramática; recursos semânticos correspondentes podem ser implementados em fases posteriores. Palavras ainda não implementadas não devem ser apresentadas como recursos funcionais.

## 6. Declarações e funções

Formas-base propostas:

```nebula
variavel nome: texto = "Nebula"
constante limite: inteiro = 100

funcao somar(a: inteiro, b: inteiro): inteiro {
    devolver a + b
}
```

- `variavel` declara vínculo mutável.
- `constante` declara vínculo não reatribuível.
- O tipo após `:` é opcional quando a inferência puder determinar um tipo estático.
- `funcao` declara uma função nomeada.
- `devolver` encerra a função e fornece seu resultado.
- Funções sem resultado explícito usam `vazio`.

## 7. Controle de fluxo

```nebula
quando (idade >= 18) {
    exibir("Maior de idade")
} senao {
    exibir("Menor de idade")
}

enquanto (ativo) {
    processar()
}

para (variavel i = 0; i < 10; i = i + 1) {
    exibir(i)
}
```

A forma exata dos laços, incluindo a sintaxe de incremento, será validada na gramática. `parar` e `seguir` são palavras reservadas previstas para controle de laços.

## 8. Tipos iniciais

| Tipo | Significado |
|---|---|
| `logico` | valor verdadeiro/falso |
| `inteiro` | inteiro com tamanho definido pela implementação-alvo, nunca implícito na ABI pública |
| `inteiro8/16/32/64/128` | inteiros com largura explícita; 128 quando suportado |
| `decimal32`, `decimal64` | ponto flutuante binário |
| `texto` | sequência UTF-8 |
| `caractere` | escalar Unicode |
| `byte` | unidade de 8 bits |
| `vazio` | ausência de valor de retorno |

Tipos compostos, genéricos, opcionais, estruturas, enumerações, ponteiros e funções fazem parte do desenho geral, mas seus detalhes serão especificados antes de serem implementados.

## 9. Literais

- Inteiros decimais: `0`, `42`, `1_000`.
- Bases não decimais previstas: `0x`, `0b`, `0o`.
- Decimais: `3.14`, `1.0e-3`.
- Lógicos: `verdadeiro`, `falso`.
- Nulo: `nulo`.
- Texto: aspas duplas, com escapes documentados na gramática.
- Caractere: aspas simples, contendo um escalar Unicode após escapes.

## 10. Comentários

- Linha: `// comentário`.
- Bloco: `/* comentário */`.
- Comentários de bloco não aninhados na primeira versão; o comportamento será erro explícito se não houver fechamento.

## 11. Módulos

A forma preferida é `incluir nome` ou `incluir nome como apelido`. Caminhos, visibilidade, resolução de pacotes e ciclos serão definidos no documento de módulos.

## 12. Tipagem e conversões

- Tipagem estática por padrão.
- Inferência local permitida quando não houver perda de previsibilidade.
- Conversões implícitas potencialmente perigosas não são permitidas.
- Conversões explícitas deverão ser visíveis no código.
- O comportamento de overflow, divisão por zero e conversões numéricas deverá ser definido por tipo e modo de compilação.

## 13. Memória e segurança

Modelo híbrido planejado: propriedade e empréstimos verificados estaticamente para o caminho seguro; operações de baixo nível explicitamente marcadas e isoladas. Nenhum coletor de lixo global será requisito universal. A sintaxe e as garantias exatas de segurança ainda precisam de especificação própria antes da implementação.

## 14. Compatibilidade

A versão inicial da linguagem não promete estabilidade binária. Mudanças na gramática devem atualizar exemplos, fixtures e documentação. Recursos experimentais devem ser identificados e não podem ser anunciados como estáveis.
