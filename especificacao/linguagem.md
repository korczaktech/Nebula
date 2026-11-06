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

1. Todo corpo executável ou estrutural DEVE usar Function Body iniciado por `: {`.
2. O conteúdo DEVE terminar com `fim` antes de `}`; construções completas também recebem `fim` após a chave.
3. Chaves de dados são distintas de Function Body; dicionários em declarações comuns não exigem `fim`.
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
- Chaves `{}` delimitam Function Body quando precedidas por `:` e também delimitam valores de dados no contexto de expressões.
- Aspas duplas delimitam textos; aspas simples delimitam caracteres.
- Ponto e vírgula `;` é separador explícito opcional entre instruções, exceto onde a gramática define separadores próprios.
- Quebras de linha separam instruções quando a expressão anterior está completa.
- A indentação deve ser consistente e usa espaços, mas não determina a extensão dos blocos.
- Cada Function Body começa com `: {`.
- O conteúdo de cada corpo termina com `fim` antes de `}`; construções completas recebem o `fim` externo após a chave.
- Blocos aninhados encerram seus próprios conteúdos antes dos respectivos `}`.

Exemplo normativo:

~~~nebula
funcao principal() -> vazio: {
    variavel nome = "Nebula"
    variavel idade = 18

    quando idade >= 18: {
        exibir("Olá, " + nome)
        fim
    }
    senao: {
        exibir("Olá")
        fim
    }
    fim
} fim
~~~

A gramática detalhada e as regras de Function Body estão em `gramatica.md`; os tokens e a geração lexical estão em `tokens.md`.

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

A forma canônica de inclusão é `incluir(biblioteca: nome)` ou `incluir(modulo: nome, biblioteca: nome)`, com alias opcional `como`, conforme `gramatica.md`. Resolução de pacotes, visibilidade completa e ciclos dependem das etapas semânticas e de carregamento.

## 10. Diagnósticos

Mensagens oficiais são em português e devem conter código estável, severidade, mensagem e intervalo de origem quando disponível. Os códigos léxicos e sintáticos são emitidos pelas respectivas etapas.
