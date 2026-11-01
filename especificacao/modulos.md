# Módulos e dependências — proposta inicial

## Importação

Forma preferida:

```nebula
incluir Matemática
incluir Rede como Comunicacao
```

- Importações ficam no escopo de arquivo na primeira versão.
- Nomes qualificados usam `::`.
- O compilador deve detectar ciclos conforme a política final de módulos.
- Símbolos exportados e privados terão visibilidade explícita.
- A resolução de pacotes deve ser reproduzível e não depender de busca arbitrária no sistema.

## Biblioteca padrão modular

Módulos planejados: Núcleo, Matemática, Texto, Coleções, Arquivos, Rede, Tempo, Sistema, Memória, Paralelo, Segurança, Dados, Interface, Gráficos e Banco.

A inclusão explícita deve evitar carregar módulos desnecessários. O mecanismo de pacotes e versionamento será definido em fase posterior.
