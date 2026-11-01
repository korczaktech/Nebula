# Sistema de tipos — decisões iniciais

## Modelo

- Tipagem estática por padrão.
- Inferência local e determinística quando o tipo puder ser resolvido sem adivinhação.
- Tipos explícitos para interfaces públicas, ABI e fronteiras de módulos.
- Conversões potencialmente redutoras exigem conversão explícita.
- Não há coerção implícita geral entre texto, números e lógicos.

## Tipos primitivos planejados

- `logico`
- `inteiro` (largura dependente do alvo, a ser fixada na especificação da ABI)
- `inteiro8`, `inteiro16`, `inteiro32`, `inteiro64`, `inteiro128` quando suportado
- variantes sem sinal correspondentes
- `decimal32`, `decimal64`
- `byte`
- `caractere`
- `texto`
- `vazio`

## Tipos compostos planejados

Vetores, tuplas, estruturas, enumerações, opcionais, resultados, funções, referências, ponteiros e genéricos.

## Decisões ainda abertas

- Largura padrão de `inteiro` por alvo.
- Overflow em modo seguro e otimizado.
- Semântica de ponto flutuante e NaN.
- Representação e propriedade de `texto`.
- Tipos algébricos e inferência genérica.
- Layout e alinhamento de estruturas na ABI.

Essas decisões devem ser formalizadas antes de estabilizar a ABI e não podem ser presumidas pela implementação.
