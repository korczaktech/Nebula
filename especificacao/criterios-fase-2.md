# Critérios de aceite — Fase 2

**Estado: implementação inicial integrada; validação final de CI pendente.**

A Fase 2 entrega o frontend sintático executável da Nebula. O escopo é o subconjunto normativo de `especificacao/tokens.md` e `especificacao/gramatica.md`; não inclui análise semântica, geração de código ou execução de programas.

## Componentes

| Critério | Implementação | Evidência |
|---|---|---|
| Tokens com categoria, lexema e intervalo em bytes | `nebula-sintaxe` | testes unitários do Lexer |
| Identificadores Unicode XID e normalização NFC | `unicode-ident` e `unicode-normalization` | teste de equivalência NFC |
| Palavras reservadas e pontuação | Lexer | testes de tokens |
| Operadores por maior correspondência | Lexer | testes de operadores compostos |
| Números decimais e inteiros em bases 2, 8, 10 e 16 | Lexer | testes numéricos |
| Texto, caractere e escapes Unicode | Lexer | testes de literais |
| Comentários de linha e bloco; preservação de quebras | Lexer | testes de comentários |
| LF, CRLF, separadores e continuação de expressões | Lexer e Parser | testes de quebras de linha |
| AST independente da CLI | tipos públicos `Programa`, `NoAst` e `Expressao` | testes de AST |
| Declarações, blocos, laços e condicionais | Parser | testes de gramática |
| Precedência e associatividade de operadores | Parser Pratt | testes de precedência |
| Recuperação para continuar após erros | Parser | teste de recuperação |
| Diagnósticos com código, mensagem e intervalo | crates `nebula-diagnosticos` e `nebula-sintaxe` | testes e CLI |
| Verificação de arquivo pela CLI | `nebula verificar arquivo.nbx` | testes de integração |

## Códigos de diagnóstico

- `N1000`: arquivo de entrada não é UTF-8 válido (camada de leitura da CLI).
- `N1001`: quebra CR isolada.
- `N1002`: comentário de bloco não terminado.
- `N1003`: caractere inesperado.
- `N1004`: expoente sem dígitos.
- `N1005`: literal numérico ou separador malformado.
- `N1006`: texto atravessou uma quebra de linha.
- `N1007`: escape inválido.
- `N1008`: literal de texto não terminado.
- `N1009`: literal de caractere inválido.
- `N2001`–`N2010`: erros sintáticos e construções incompletas.
- `N2011`: uso de palavra reservada para recurso futuro.
- `N2012`: comparação relacional encadeada.

## Limites explícitos

- O comando `verificar` valida somente léxico e sintaxe; não afirma que nomes, tipos ou operações sejam semanticamente válidos.
- Não há geração de executáveis, interpretador, IR ou backend nesta fase.
- O parser constrói uma AST parcial quando encontra erros, para permitir múltiplos diagnósticos.
- A leitura de bytes inválidos é rejeitada pela CLI antes de criar `ArquivoFonte`; o Lexer recebe texto UTF-8 válido.
- Palavras reservadas futuras não habilitam os recursos correspondentes.
- A validação final desta fase depende de CI verde no commit final e da revisão dos testes contra a gramática normativa.

## Regra de encerramento

A Fase 2 só será concluída quando a CI executar formatação, Clippy e toda a suíte de testes com `--locked`, sem falhas. A presença dos módulos ou a compilação isolada não substitui esse aceite.
