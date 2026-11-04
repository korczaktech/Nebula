# Testes de conformidade

A suíte de conformidade é executável e acompanha a gramática normativa em
`especificacao/gramatica.md` e as regras léxicas em `especificacao/tokens.md`.

## Execução

Na raiz do repositório:

```sh
cargo test --locked -p nebula-sintaxe
```

A suíte de integração `compilador/sintaxe/tests/conformidade.rs` executa:

- corpus positivo cobrindo declarações, tipos, estruturas, enumerações,
  módulos, instruções, blocos, padrões, expressões, operadores e literais;
- corpus negativo que exige os códigos de diagnóstico sintático documentados;
- validação de intervalos de origem em UTF-8;
- programa integrado de referência para verificar a composição das produções.

Os testes unitários do Lexer e do Parser complementam o corpus com casos de
fronteira, precedência, associatividade, recuperação de erros, delimitadores,
separadores, Unicode e estabilidade sob entradas adversariais. A suíte de
resistência gera 2.048 entradas determinísticas para Lexer/Parser, JSON e
manifestos; também verifica arquivos com 4.096 declarações e recuperação após
2.048 delimitadores inválidos, além de limites de aninhamento e intervalos UTF-8.

Os arquivos `.nbx` desta pasta são fixtures de referência e também são
carregados pela suíte automatizada. A CI executa os testes do workspace em
Linux, Debian 13 Trixie, macOS e Windows.
