# Especificação inicial da linguagem Nebula

**Estado:** contrato de linguagem para a fundação e o subconjunto sintático inicial. A implementação do compilador ocorrerá nas fases seguintes. Alterações incompatíveis exigem atualização da gramática, tokens, exemplos e testes.

## 1. Identidade

- Nome: Nebula.
- Extensão de fonte: .nbx.
- Idioma oficial: português brasileiro para palavras-chave, mensagens e documentação.
- Implementação inicial do compilador: Rust.
- Paradigma: multiparadigma, compilada e de propósito geral.
- Tipagem: estática por padrão, com inferência local controlada.
- Alvos pretendidos: código nativo e WebAssembly; nenhum backend está entregue na fundação.
- Arquivos-fonte: UTF-8.

## 2. Princípios normativos

As palavras **DEVE**, **NÃO DEVE**, **PODE** e **RECOMENDA-SE** indicam requisito, proibição, opção e recomendação.

1. O significado de um programa não depende da indentação.
2. Delimitadores são aceitos e não devem ser exigidos quando a gramática puder reconhecer a estrutura sem ambiguidade.
3. Uma quebra de linha pode separar instruções completas; ponto e vírgula também pode separar instruções explicitamente.
4. Construções ambíguas devem ser rejeitadas, nunca resolvidas por adivinhação.
5. Palavras-chave e mensagens oficiais usam português brasileiro.
6. = representa inicialização/atribuição; == representa igualdade.
7. Diagnósticos devem identificar a origem do problema com a precisão disponível.
8. A especificação é independente da implementação e dos backends.

## 3. Sintaxe visual

- Parênteses () agrupam expressões, parâmetros e argumentos. Podem ser omitidos em condições quando o bloco seguinte delimitar a expressão sem ambiguidade. Chamadas de função e parâmetros mantêm parênteses no subconjunto inicial.
- Chaves {} delimitam blocos; alternativamente, blocos podem terminar com fim.
- Colchetes [] delimitam listas e indexação.
- Aspas duplas delimitam literais de texto; aspas simples delimitam caracteres.
- Ponto e vírgula ; é separador explícito opcional entre instruções, exceto nos dois separadores do cabeçalho de para.
- Quebras de linha separam instruções quando a expressão anterior está completa.
- Espaços e indentação são livres e não definem escopos.
- Dentro de () e [], quebras de linha funcionam como espaços; dentro de blocos, podem separar instruções.

Exemplo:

~~~nebula
funcao principal(): vazio {
    variavel nome = "Nebula"
    variavel idade = 18

    quando idade >= 18 {
        exibir("Olá, " + nome)
    }
}
~~~

## 4. Identificadores e palavras reservadas

Identificadores usam Unicode XID_Start/XID_Continue, admitem sublinhado, são sensíveis a maiúsculas/minúsculas e comparados em NFC. Palavras reservadas não podem ser usadas como identificadores. A lista completa e a distinção entre palavras da gramática inicial e palavras reservadas futuras estão em tokens.md.

## 5. Declarações e controle de fluxo

O subconjunto sintático inicial inclui:
- variavel para vínculo mutável;
- constante para vínculo não reatribuível;
- funcao para função nomeada com parâmetros tipados;
- tipo para alias de tipo;
- incluir para importação de módulo;
- quando/senao, enquanto e para;
- devolver, parar e seguir.

A gramática completa e os exemplos canônicos estão em gramatica.md. Recursos reservados para versões futuras não devem ser apresentados como implementados.

## 6. Tipos iniciais planejados

| Tipo | Significado |
|---|---|
| logico | verdadeiro/falso |
| inteiro | inteiro cuja largura padrão ainda será definida por alvo |
| inteiro8/16/32/64/128 | inteiros de largura explícita, quando suportados |
| variantes sem sinal | inteiros sem sinal de largura explícita, a especificar |
| decimal32, decimal64 | ponto flutuante binário |
| texto | sequência UTF-8 |
| caractere | escalar Unicode |
| byte | unidade de 8 bits |
| vazio | ausência de valor de retorno |

Os nomes de tipos são identificadores em posição de tipo nesta etapa. Tipos compostos, genéricos, opcionais, estruturas, enumerações, ponteiros e funções exigem especificação semântica antes de serem estabilizados.

## 7. Literais e comentários

- Inteiros decimais e com prefixos hexadecimal, binário e octal.
- Decimais com fração e/ou expoente conforme as regras léxicas.
- Lógicos: verdadeiro e falso; nulo: nulo.
- Texto entre aspas duplas e caractere entre aspas simples, com escapes definidos em tokens.md.
- Comentários de linha // e de bloco /* ... */; comentários de bloco não são aninhados.
- Strings multilinha e interpolação não fazem parte do subconjunto inicial.

## 8. Tipagem, memória e segurança

A linguagem é estaticamente tipada por padrão, com inferência local determinística. Conversões potencialmente redutoras exigirão conversão explícita. Overflow, divisão por zero, semântica de ponto flutuante e ABI serão definidos antes da estabilização correspondente.

O modelo de memória pretendido é híbrido: propriedade e vida útil verificadas no caminho seguro, com operações de baixo nível explícitas. Não há coletor de lixo global obrigatório. Sintaxe e garantias exatas permanecem decisões abertas, registradas em decisoes.md.

## 9. Módulos

A forma inicial é incluir Matemática ou incluir Rede como Comunicacao. Importações ficam no escopo de arquivo. Caminhos qualificados usam ::. Resolução de pacotes, visibilidade completa e ciclos serão implementados e validados em fases posteriores.

## 10. Diagnósticos

Mensagens oficiais são em português e devem conter código estável, severidade, mensagem e intervalo de origem quando disponível. A estrutura inicial já implementada armazena código, severidade, mensagem e intervalo opcional. Formatação de arquivo/linha/coluna, trecho destacado, sugestões e formatos de saída serão desenvolvidos com Lexer/Parser e CLI nas fases seguintes.

## 11. Compatibilidade e desempenho

A versão inicial não promete estabilidade binária. Toda mudança incompatível deve atualizar especificação e fixtures. A meta de desempenho superior a outras linguagens é uma hipótese de engenharia, não uma garantia: só poderá ser anunciada com benchmarks reproduzíveis, cargas equivalentes e publicação das limitações.
