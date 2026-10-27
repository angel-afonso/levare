# Instrucciones para agentes

## Los mensajes de commit son el changelog

La versión de Levare y su `CHANGELOG.md` salen de los mensajes de commit, con
[release-plz](https://release-plz.dev) (configurado en `release-plz.toml`). No hay
ningún archivo de cambios que añadir: cada commit es su propia entrada. Cuando se
ejecuta el workflow Release PR, release-plz lee los commits desde la última etiqueta
`vX.Y.Z`, elige la versión siguiente según sus tipos y copia sus títulos en
`CHANGELOG.md` dentro de una pull request de release. `docs/publicar.md` cuenta el
proceso entero.

Por eso todo commit que cambie el comportamiento es un
[Conventional Commit](https://www.conventionalcommits.org/es/):

```
<tipo>(<ámbito>): <título>
```

### Tipo: decide la versión

| Tipo | Para | Versión |
| --- | --- | --- |
| `feat` | Algo nuevo que el músico puede hacer o ver | menor (0.4.2 → 0.5.0) |
| `fix` | Un error corregido: resultado incorrecto, cierre, datos perdidos | parche (0.4.2 → 0.4.3) |
| `perf` | Lo mismo, más rápido o más ligero | parche |
| `refactor` | Código reorganizado, sin cambio de comportamiento | parche |
| `docs` | Solo documentación | parche |
| `chore` | Mantenimiento: dependencias, configuración | parche |
| `ci`, `test`, `style` | Solo CI, pruebas o formato | parche, fuera del changelog |

Un cambio que rompe algo de lo que dependen los usuarios (el formato `.partitura`, los
atajos guardados, un paquete de sonidos) lleva `!` tras el ámbito, `feat(archivo)!: …`,
y explica qué se rompe en un pie `BREAKING CHANGE:`. Antes de la 1.0 sube la versión
menor.

### Ámbito: dónde cambió

La parte de la app que nota el músico, en minúsculas y sin tildes: `partitura`
(escritura y edición), `cinta`, `paneles`, `agente`, `reproduccion`, `sonidos`,
`archivo` (abrir, guardar, exportar, importar), `atajos`, `revision`, `maquetacion`,
`actualizaciones`. Para un cambio interno sin parte visible, el nombre del directorio
del crate (`score-core`, `keymap`, `update`…). Sin ámbito si abarca varias.

Solo cuentan para una versión, y salen en el changelog, los commits que tocan un crate
(`crates/…`, incluido su `Cargo.toml`). Los cambios solo en `docs/`, el README,
`scripts/` o la CI no publican nada; usa igualmente `docs:`, `ci:` o `chore:`.

### Título: es la línea del changelog

El título pasa tal cual a `CHANGELOG.md`, así que se escribe para quien usa Levare,
en español:

- Di qué cambia para el músico, en presente, en minúscula y sin punto final:
  `fix(paneles): recuerda la pestaña elegida al volver a abrir Levare`, no
  `fix: arreglo en panels.rs`.
- Un cambio por commit. Dos correcciones sin relación son dos commits, cada uno con su
  línea y su tipo.
- El porqué y los detalles van en el cuerpo; solo el título llega al changelog.

### Nunca a mano

- No cambies `[workspace.package] version` en `Cargo.toml` ni edites `CHANGELOG.md`:
  la pull request de release hace las dos cosas.
- No crees etiquetas `v*`: al fusionar la pull request de release, el workflow Release
  etiqueta y publica la versión.

## Antes de dar algo por terminado

Lo que comprueba la CI (README, «Verificar»):

```sh
cargo fmt --all --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

`docs/atajos.md` se genera desde `crates/keymap`; si cambias comandos o atajos,
regéneralo con `LEVARE_ACTUALIZAR_ATAJOS=1 cargo test -p score-keymap`.
