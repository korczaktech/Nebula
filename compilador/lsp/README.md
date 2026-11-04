# Nebula Language Server

Servidor de linguagem da Nebula, implementado em Rust e executado localmente pelo editor.

## Compilar

Na raiz do repositório:

```bash
cargo build --locked --release -p nebula-lsp
```

O executável gerado é `target/release/nebula-lsp` (no Windows, `target/release/nebula-lsp.exe`).

Para iniciar manualmente durante desenvolvimento:

```bash
cargo run --locked -p nebula-lsp
```

O processo usa **stdin/stdout** para o protocolo LSP. Não execute comandos de shell no mesmo canal. Mensagens de diagnóstico operacional são enviadas para stderr.

## Funcionalidades

- Inicialização e encerramento JSON-RPC 2.0 com enquadramento `Content-Length`.
- Sincronização completa de documentos por `didOpen`, `didChange` e `didClose`.
- Diagnósticos léxicos, sintáticos e semânticos provenientes dos crates oficiais do compilador.
- Conclusão de palavras-chave, declarações, funções e parâmetros do documento.
- Hover com tipos, declarações e assinaturas de funções.
- Ajuda de assinatura durante a digitação de chamadas.
- Ir para definição usando declarações encontradas na AST.
- Referências baseadas nos tokens identificadores do lexer.
- Símbolos do documento.
- Conversão de posições LSP UTF-16 para offsets UTF-8.
- Limites de tamanho para cabeçalhos e mensagens recebidas.

## Arquitetura

O servidor não implementa uma gramática própria. Cada atualização de documento é analisada por `nebula-sintaxe`; os diagnósticos semânticos vêm de `nebula-semantica`. A AST e os tokens existentes são reutilizados para fornecer informações ao editor.

O processo é local: a extensão do VS Code inicia `nebula-lsp` como subprocesso e conversa com ele por stdin/stdout. Não exige servidor remoto.

## Integração com a extensão

A configuração de inicialização do executável e o empacotamento/distribuição para cada sistema operacional pertencem ao repositório `Nebula-Extension`. Este crate fornece o servidor e o executável; a publicação da extensão precisa apontar para o binário correspondente à plataforma.
