# Critérios de aceite — Fase 3

**Estado: CONCLUÍDA — 100% dos critérios definidos nesta fase foram implementados e validados.**

A Fase 3 entrega análise semântica, resolução explícita de módulos, tipagem estática e uma representação intermediária executável por um interpretador de referência. Ela não gera binários nativos ou WebAssembly; isso pertence à Fase 4.

## Componentes e critérios

| Critério | Implementação | Evidência exigida |
|---|---|---|
| Tabelas de símbolos e escopos léxicos | `nebula-semantica` | testes de declaração, duplicidade e visibilidade entre escopos |
| Pré-declaração de funções e aliases | `nebula-semantica` | chamadas antecipadas, aliases encadeados e ciclos |
| Resolução de nomes | `nebula-semantica` | rejeição de nomes ausentes e validação de parâmetros |
| Resolução de módulos | `ModulosDisponiveis` e aliases de importação | módulo ausente rejeitado; símbolos exportados resolvidos |
| Tipos estáticos | `Tipo` | primitivos, larguras inteiras, decimais e listas |
| Inferência controlada | declarações sem anotação | inferência local determinística e incompatibilidades rejeitadas |
| Verificação de operadores e chamadas | analisador semântico | operandos, aridade, parâmetros, retorno e condições |
| Regras iniciais de mutabilidade | símbolos mutáveis/constantes | atribuição a constante e destinos inválidos rejeitados |
| Retornos e controle de laços | análise de fluxo estrutural | retorno incompatível, caminhos sem retorno e controle fora de laço |
| IR independente dos tokens sintáticos | `nebula-ir` | instruções, expressões, operadores e funções normalizados |
| Interpretador de referência | `executar` em `nebula-ir` | aritmética, chamadas, condicionais, laços, saída e limite de passos |
| CLI semântica | `nebula analisar <arquivo.nbx>` | testes de integração positivos e negativos |
| Inspeção da IR | `nebula ir <arquivo.nbx>` | teste de integração da geração |
| CI reproduzível | workspace e lockfile | formatação, Clippy e testes com `--locked` |

## Contratos e limites explícitos

- A análise semântica recebe uma AST válida. A CLI interrompe a etapa semântica se Lexer ou Parser emitirem erros.
- Os módulos são fornecidos explicitamente pelo chamador em `ModulosDisponiveis`. Não há busca arbitrária no disco, resolvedor de pacotes ou biblioteca padrão implementada nesta fase.
- A visibilidade de um símbolo externo é representada pelo fato de ele estar ou não presente na tabela de exports fornecida pelo módulo.
- Tipos inteiros com larguras explícitas são distintos na verificação estática. A largura ABI de `inteiro`, overflow por alvo e representação final ainda dependem da especificação do backend.
- Inferência é local. Não há inferência genérica, tipos algébricos, estruturas, enumerações, opcionais ou genéricos.
- Constantes são imutáveis; variáveis são reatribuíveis. A IR de referência trabalha com valores, sem referências compartilhadas.
- Não há sintaxe nem verificador de empréstimo, ponteiros, destrutores, alocadores ou concorrência segura. Não se anuncia um borrow checker.
- A IR de referência e seu interpretador são infraestrutura de validação; não substituem compilação nativa, WebAssembly, ABI ou runtime de produção.
- Chamadas a funções de módulos podem ser representadas na IR, mas o interpretador de referência não carrega implementações externas de módulos.
- A IR não garante desempenho de produção. Nenhuma meta de desempenho é considerada atendida sem backend e benchmarks reproduzíveis.

## Códigos de diagnóstico semântico

- `N3001`: função duplicada.
- `N3002`: alias de tipo duplicado.
- `N3003`: alias inválido ou cíclico.
- `N3004`: módulo não disponibilizado.
- `N3005`: retorno fora de função.
- `N3006`: controle de laço fora de laço.
- `N3007`: nome não declarado.
- `N3008`: operador unário incompatível.
- `N3009`: tentativa de alterar constante.
- `N3010`: destino de atribuição inválido.
- `N3011`: função qualificada não resolvida.
- `N3012`: alvo de chamada inválido.
- `N3013`: indexação não aplicada a lista.
- `N3014`: acesso a membro não definido.
- `N3015`: aridade incorreta.
- `N3016`–`N3018`: operandos incompatíveis.
- `N3019`: declaração duplicada no mesmo escopo ou colisão entre função, variável e alias de módulo.
- `N3020`: incompatibilidade de tipos.
- `N3021`: tipo não declarado.
- `N3022`: alias de módulo duplicado.
- `N3023`: função aninhada não permitida.
- `N3024`: função pode terminar sem retorno.
- `N3025`: alias de tipo fora do escopo de arquivo.
- `N3026`: inclusão de módulo fora do escopo de arquivo.
- `N3027`: lista vazia sem contexto de tipo para inferência.
- `N3028`: literal numérico fora do intervalo representável.

## Resultado do aceite

- Commit de código validado: `6d270f873cc95bab5c5c2776f9fa609371dd6b52`.
- [GitHub Actions — execução aprovada](https://github.com/korczaktech/Nebula/actions/runs/41372653540).
- `cargo fmt --all -- --check`: aprovado.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: aprovado.
- `cargo test --locked --workspace --all-targets`: aprovado.
- **104 testes aprovados; 0 falhas.**

A Fase 3 está concluída dentro do escopo definido neste documento. Isso não significa que a Nebula já gere executáveis nativos/WebAssembly, possua carregador de módulos, biblioteca padrão completa ou borrow checker; esses itens permanecem explicitamente fora desta fase.
