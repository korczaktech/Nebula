# Segurança e cadeia de suprimentos

## Verificações automatizadas

A CI executa formatação, Clippy sem warnings, testes, documentação e benchmark. Os testes determinísticos de entrada adversarial do Lexer/Parser são smoke tests de fuzzing; não equivalem a campanhas contínuas de fuzzing.

A auditoria de dependências usa `cargo audit` contra o RustSec Advisory Database. Um aviso deve ser triado; não deve ser silenciado sem justificativa, versão afetada e plano de correção.

## Relato responsável

Não publique detalhes exploráveis de uma vulnerabilidade não corrigida. Relatos devem incluir versão/commit, plataforma, passos mínimos de reprodução e impacto. Segredos, tokens e dados pessoais não devem ser incluídos em issues públicas.

## Limites

A execução de auditoria de dependências não é auditoria de código, auditoria criptográfica, prova de ausência de comportamento indefinido nem certificação de segurança. O projeto ainda precisa de revisão independente antes de 1.0.


## APIs criptográficas da biblioteca padrão

O crate `nebula-seguranca` expõe SHA-256 para integridade de conteúdo, HMAC-SHA-256 para autenticação baseada em segredo e comparação de bytes sem encerramento antecipado por conteúdo. A comparação ainda revela o comprimento das entradas. Essas primitivas não substituem gestão de chaves, geração criptográfica de aleatoriedade, KDFs ou protocolos completos; nenhum desses recursos é anunciado como implementado.
