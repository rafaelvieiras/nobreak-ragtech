# Contribuindo

Obrigado por ajudar! O projeto é em **português**: issues, commits, documentação e código (nomes de domínio) em PT-BR.

## Regras do projeto

1. **Nada do software oficial.** Não cole código, trechos decompilados, arquivos de configuração ou textos da interface
   do Supervise. Descreva o protocolo com suas palavras e com dados medidos no seu nobreak.
2. **Somente leitura por padrão.** Escrita no nobreak (limpar flags, testes de bateria, desligamento) só entra atrás de uma
   opção explícita, com aviso e com resultado de teste em hardware real.
3. **Registre o que é confirmado e o que é hipótese** em `docs/protocolo.md`.
4. Toda fórmula nova ganha um teste com uma captura real em `backend/src/leitura.rs`.

## Como testar um modelo novo

1. `make dev-backend` não serve: rode `NOBREAK_PORTA=/dev/ttyACM0 cargo run` com o nobreak ligado.
2. Copie a linha **Registradores** do painel e compare cada valor com o display do nobreak.
3. Abra uma issue com o modelo, a saída de `lsusb -v -d 04d8:000a` e os registradores.

## Antes do PR

```bash
make test
```
