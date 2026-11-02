# Operadores e precedência

## Atribuição e comparação

- `=`: atribuição e inicialização.
- `==`: igualdade.
- `!=`: diferença.
- `<`, `<=`, `>`, `>=`: comparação.

## Aritmética

- `+`, `-`, `*`, `/`, `%`, `**`.

## Lógica

- `e`, `ou`, `nao`.
- `!` é aceito como operador unário alternativo na gramática inicial; a forma textual recomendada é `nao`.

## Outros

- `..`: intervalo.
- `.`: acesso a membro.
- `::`: qualificação de módulo ou símbolo.

## Precedência (maior para menor)

1. chamada, indexação e acesso a membro;
2. potência;
3. unários;
4. multiplicação/divisão/resto;
5. adição/subtração;
6. intervalo;
7. comparação;
8. igualdade;
9. E;
10. OU;
11. atribuição.

Atribuição e potência associam à direita. Aritmética binária associa à esquerda. Comparações encadeadas não são permitidas inicialmente; use operadores lógicos explícitos.
