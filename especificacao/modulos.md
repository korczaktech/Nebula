# Módulos e dependências — Fase 3

## Importação

Forma preferida:

~~~nebula
incluir Matemática
incluir Rede como Comunicacao
~~~

- Importações ficam no escopo de arquivo.
- Nomes qualificados usam ::.
- O analisador semântico resolve o alias de importação e verifica se a função qualificada existe no conjunto de exports do módulo.
- O conjunto de módulos é fornecido explicitamente pela API ModulosDisponiveis, indexado pelo caminho do módulo e contendo assinaturas de funções exportadas.
- O compilador não pesquisa caminhos arbitrários no disco.
- A visibilidade pública/privada de um módulo externo é refletida na tabela de exports fornecida pelo carregador.

## Limites da Fase 3

Não há carregador de módulos do sistema de arquivos, resolvedor de pacotes, versionamento, cache de módulos ou biblioteca padrão executável. A API recebe as assinaturas dos módulos já disponibilizados pelo chamador. Chamadas qualificadas são representadas na IR; o interpretador de referência ainda não carrega a implementação externa.

## Biblioteca padrão modular planejada

Núcleo, Matemática, Texto, Coleções, Arquivos, Rede, Tempo, Sistema, Memória, Paralelo, Segurança, Dados, Interface, Gráficos e Banco.

A inclusão explícita deverá evitar carregar módulos desnecessários. O mecanismo de pacotes e versionamento será definido em fase posterior.
