# Especificação da linguagem Nebula

**Estado:** contrato de linguagem alinhado ao frontend sintático atual. Alterações incompatíveis exigem atualização de `gramatica.md`, `tokens.md`, exemplos e testes de conformidade.

## 1. Identidade

- Nome: Nebula.
- Extensão: `.nbx`.
- Idioma principal: português brasileiro.
- Implementação inicial do compilador: Rust.
- Modelo: linguagem compilada, multiparadigma e de propósito geral.
- Tipagem: estática por padrão, com inferência controlada.
- Arquivos-fonte: UTF-8.

## 2. Princípios normativos

As palavras **DEVE**, **NÃO DEVE**, **PODE** e **RECOMENDA-SE** indicam requisito, proibição, opção e recomendação.

1. Blocos de instrução DEVEM usar indentação explícita e terminar com `fim`.
2. Chaves NÃO DEVEM substituir blocos de instrução.
3. Chaves continuam válidas para construções de dados, como valores de estruturas.
4. Uma quebra de linha separa instruções quando a expressão anterior está completa.
5. Ponto e vírgula também pode separar instruções explicitamente.
6. Construções ambíguas devem ser rejeitadas, nunca resolvidas por adivinhação.
7. Palavras-chave e mensagens oficiais usam português brasileiro.
8. `=` representa inicialização/atribuição; `==` representa igualdade.
9. Diagnósticos devem identificar a origem do problema com a precisão disponível.
10. A especificação é independente da implementação e dos backends.

## 3. Sintaxe visual e blocos

- Parênteses `()` agrupam expressões, parâmetros e argumentos.
- Colchetes `[]` delimitam listas, tipos de lista e indexação.
- Chaves `{}` delimitam construções de dados, como valores de estruturas; não delimitam corpos de instruções.
- Aspas duplas delimitam textos; aspas simples delimitam caracteres.
- Ponto e vírgula `;` é separador explícito opcional entre instruções, exceto onde a gramática define separadores próprios.
- Quebras de linha separam instruções quando a expressão anterior está completa.
- Espaços e indentação dentro de um mesmo nível são livres, mas a indentação entre linhas define a extensão dos blocos.
- Cada bloco de instrução começa após quebra de linha e aumento de indentação.
- O corpo de cada bloco termina com dedentação e a palavra-chave `fim`.
- Blocos aninhados terminam do mais interno para o mais externo.

Exemplo normativo:

~~~nebula
funcao principal() -> vazio
    variavel nome = "Nebula"
    variavel idade = 18

    quando idade >= 18
        exibir("Olá, " + nome)
    senao
        exibir("Olá")
    fim
fim
~~~

A gramática detalhada, as regras de dedentação e as construções aceitas estão em `gramatica.md`; os tokens e a geração lexical de `INDENT`/`DEDENT` estão em `tokens.md`.

## 4. Identificadores e palavras reservadas

Identificadores usam Unicode XID_Start/XID_Continue, admitem sublinhado, são sensíveis a maiúsculas/minúsculas e comparados em NFC. Palavras reservadas não podem ser usadas como identificadores. A lista e a distinção entre palavras implementadas e futuras estão em `tokens.md`.

## 5. Declarações e controle de fluxo

O frontend atual reconhece declarações de variáveis, constantes, funções, classes, aliases de tipo, estruturas, enumerações, atributos, contextos, inclusão de módulos e instruções de controle, condicionais, escolha, laços e tratamento de erros, conforme as produções de `gramatica.md`.

Recursos reservados para versões futuras não devem ser apresentados como implementados.

## 6. Tipos

Os nomes de tipos são identificadores em posição de tipo, salvo tokens especiais como `vazio`. O Parser reconhece tipos qualificados, genéricos, listas e funções. A existência, compatibilidade, largura e semântica dos tipos pertencem à análise semântica e às etapas posteriores.

## 7. Literais e comentários

- Inteiros decimais e com prefixos hexadecimal, binário e octal.
- Decimais com fração e/ou expoente conforme as regras léxicas.
- Lógicos: `verdadeiro`, `falso` e `sim`; nulo: `nulo`.
- Texto entre aspas duplas e caractere entre aspas simples, com escapes definidos em `tokens.md`.
- Comentários de linha `//` e de bloco `/* ... */`; comentários de bloco não são aninhados.
- Limites de interpolação e texto multilinha seguem o Lexer e o Parser efetivamente implementados.

## 8. Memória, segurança e desempenho

O modelo de memória pretendido é híbrido: propriedade e vida útil verificadas no caminho seguro, com operações de baixo nível explícitas. As garantias exatas são definidas progressivamente e não devem ser presumidas apenas pela existência de sintaxe.

Metas de desempenho só podem ser anunciadas com benchmarks reproduzíveis e comparações equivalentes.

## 9. Módulos

A forma inicial de inclusão é `incluir caminho`, com caminho qualificado e aliases opcionais conforme `gramatica.md`. Resolução de pacotes, visibilidade completa e ciclos dependem das etapas semânticas e de carregamento.

## 10. Diagnósticos

Mensagens oficiais são em português e devem conter código estável, severidade, mensagem e intervalo de origem quando disponível. Os códigos léxicos e sintáticos são emitidos pelas respectivas etapas.
