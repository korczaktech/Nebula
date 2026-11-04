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
| Código | Significado |
|---|---|
| N2001 | Separador ausente entre instruções ou itens de bloco |
| N2002 | Modificador de visibilidade sem declaração de função |
| N2003 | Operador de atribuição ausente em declaração |
| N2004 | Parêntese de abertura ausente na função |
| N2005 | Parâmetro sem anotação de tipo ou separador inválido |
| N2006 | Atribuição ausente em alias de tipo |
| N2007 | Bloco ausente ou malformado |
| N2008 | Expressão ausente ou inválida |
| N2009 | Token delimitador ou estrutural esperado não encontrado |
| N2010 | Nome/identificador esperado |
| N2011 | Palavra reservada para recurso futuro usada em posição inválida |
| N2012 | Comparação relacional encadeada |
| N2013 | Operador de intervalo encadeado |
| N2014 | Parâmetro genérico duplicado |
| N2015 | Lista genérica vazia ou parâmetro genérico ausente |
| N2016 | Construção reservada para versão futura |
| N2030 | Chave de abertura ausente na estrutura |
| N2031 | Campo duplicado na estrutura |
| N2032 | Separador ausente entre campos da estrutura |
| N2033 | Chave de abertura ausente na enumeração |
| N2034 | Variante duplicada na enumeração |
| N2035 | Separador ou tipo de payload ausente na enumeração |
| N2036 | Enumeração sem variantes |
| N2037 | Campo duplicado na construção de estrutura |
| N2038 | Separador ausente entre campos da construção |
| N2040 | Chave de abertura ausente em escolha |
| N2041 | Caso ausente em escolha |
| N2042 | Padrão de escolha inválido |
| N2043 | Caso curinga duplicado |
| N2044 | Escolha sem casos |
| N2045 | Vínculo de payload ausente ou inválido |
| N2046 | Vínculo duplicado no payload do padrão |
| N2047 | Alteração sem expressão de atribuição |
| N2048 | Padrão de enumeração duplicado em escolha |
| N2049 | Limite de aninhamento sintático excedido |
| N2050 | Parâmetro de função duplicado |

## Limites explícitos

- O comando `verificar` valida somente léxico e sintaxe; não afirma que nomes, tipos ou operações sejam semanticamente válidos.
- Não há geração de executáveis, interpretador, IR ou backend nesta fase.
- O parser constrói uma AST parcial quando encontra erros, para permitir múltiplos diagnósticos.
- A leitura de bytes inválidos é rejeitada pela CLI antes de criar `ArquivoFonte`; o Lexer recebe texto UTF-8 válido.
- Palavras reservadas futuras não habilitam os recursos correspondentes.
- A validação final desta fase depende de CI verde no commit final e da revisão dos testes contra a gramática normativa.

## Regra de encerramento

A Fase 2 só será concluída quando a CI executar formatação, Clippy e toda a suíte de testes com `--locked`, sem falhas. A presença dos módulos ou a compilação isolada não substitui esse aceite.
