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
- Aliases de módulo não podem colidir com nomes de funções ou variáveis no programa.
- O compilador não pesquisa caminhos arbitrários no disco.
- A visibilidade pública/privada de um módulo externo é refletida na tabela de exports fornecida pelo carregador.

## Estado da implementação após a Fase 5

O carregador da CLI resolve módulos locais recursivamente a partir do diretório do arquivo principal. Normaliza e valida identificadores de caminho, bloqueia travessia fora da raiz permitida, rejeita symlinks para fora da área de dependências, detecta ciclos e liga chamadas qualificadas a funções públicas dos módulos importados. Módulos importados ainda não aceitam inicializadores globais.

O crate `nebula-pacotes` lê `nebula.toml`, resolve dependências locais transitivas, valida nome e versão do pacote, aceita restrições SemVer locais na seção `[restricoes]` (`*`, `^`, `~`, comparadores e conjunções), grava `nebula.lock` determinístico com SHA-256 e oferece verificação explícita do lockfile. O resolvedor não baixa pacotes remotos e não implementa um registro remoto.

A compilação usa cache local em `.nebula/cache` com chave derivada dos bytes da fonte principal, módulos recursivos, versão/fingerprint do compilador, alvo e nível de otimização. O conteúdo do cache é verificado por SHA-256 antes de reutilização. `nebula limpar-cache` remove os artefatos armazenados.

A biblioteca padrão tem crates Rust para vários módulos planejados, mas nem todas as funções estão expostas à linguagem `.nbx` como imports executáveis. Consulte [docs/BIBLIOTECA.md](../docs/BIBLIOTECA.md) para limites reais de cada API.
