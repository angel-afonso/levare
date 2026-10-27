# Publicar una versión

Levare se publica en GitHub Releases como una app universal para Mac (Apple Silicon e
Intel), en `.dmg` para instalarla y en `.zip` para el actualizador de la app. La versión
y el changelog salen de los mensajes de commit (ver `AGENTS.md`).

## Los tres workflows

| Workflow | Cuándo | Qué hace |
| --- | --- | --- |
| **CI** (`ci.yml`) | Cada push a `main` y cada pull request | Formato, clippy y pruebas en macOS. |
| **Release PR** (`release-pr.yml`) | A mano: Actions → Release PR → Run workflow, desde `main` | release-plz abre (o actualiza) la pull request «chore: publica X.Y.Z», que sube `[workspace.package] version` y añade las entradas a `CHANGELOG.md`. |
| **Release** (`release.yml`) | Cada push a `main`; un tag `v*`; o a mano | Si la versión de `Cargo.toml` aún no tiene tag, la etiqueta, compila, firma y publica. Si ya lo tiene, no hace nada. A mano desde una rama solo compila y deja la app como artefacto. |

Para publicar: ejecuta Release PR, revisa la pull request (sobre todo el changelog) y
fusiónala. El push a `main` que eso produce hace el resto.

## Qué publica una versión

- `levare-vX.Y.Z-universal-apple-darwin.dmg`: el instalador (arrastrar Levare a
  Aplicaciones).
- `levare-vX.Y.Z-universal-apple-darwin.zip`: la misma app, la que descarga el
  actualizador.
- `latest.json` y `….zip.minisig`: el manifiesto que lee la app y la firma del zip.
- `SHA256SUMS`.

La app lleva el motor sfizz en `Contents/Frameworks`, compilado en el workflow con
`scripts/build_sfizz.sh` (sfizz 1.2.3 universal, ver `docs/vsco.md`) y guardado en caché
mientras el script no cambie.

## Secretos del repositorio

Settings → Secrets and variables → Actions.

| Secreto | Obligatorio | Qué es |
| --- | --- | --- |
| `UPDATE_SIGNING_KEY` | Sí, para publicar | El contenido entero del archivo de clave secreta minisign (sin contraseña) cuya clave pública es `crates/app-gpui/assets/update.pub`. Sin él, un push a `main` no etiqueta, compila ni publica nada (Release solo avisa). |
| `MACOS_CERTIFICATE` | No | El certificado «Developer ID Application» exportado como `.p12`, en base64 (`base64 -i cert.p12`). |
| `MACOS_CERTIFICATE_PASSWORD` | Con el anterior | La contraseña del `.p12`. |
| `APPLE_API_KEY` | No | La clave de App Store Connect API (`AuthKey_….p8`) en base64, para notarizar. |
| `APPLE_API_KEY_ID`, `APPLE_API_ISSUER_ID` | Con el anterior | Su Key ID y su Issuer ID. |

Sin los secretos de Apple la app lleva una firma ad hoc y no está notarizada: la primera
vez hay que abrirla con clic derecho → Abrir. Con ellos, Gatekeeper la abre sin avisos.

Además, Release PR necesita que Actions pueda abrir pull requests: Settings → Actions →
General → «Allow GitHub Actions to create and approve pull requests».

### La clave de las actualizaciones

La app solo instala un zip firmado con la clave secreta cuya pública lleva dentro
(`crates/app-gpui/assets/update.pub`), y solo si la firma nombra esa versión y ese
archivo. **Guarda una copia de la clave secreta fuera del ordenador**: si se pierde, las
copias ya instaladas no aceptarán ninguna actualización firmada con otra clave y habrá
que reinstalarlas a mano desde el `.dmg`.

Para cambiarla: genera un par nuevo sin contraseña (`minisign -G -W -p update.pub -s
update.key`), sustituye `crates/app-gpui/assets/update.pub`, actualiza el secreto y
publica una versión; las copias instaladas necesitan esa versión instalada a mano.

## El actualizador de la app

En una compilación de release que se ejecuta desde `Levare.app`:

- Diez segundos después de abrirse, y cada seis horas, busca en
  `https://github.com/angel-afonso/levare/releases/latest/download/latest.json` una
  versión más nueva, la descarga a la carpeta de datos (`actualizaciones/`, que se vacía al abrir Levare) y comprueba
  su SHA-256 y su firma. La barra de estado muestra el progreso y, al final,
  «Reiniciar para actualizar a X.Y.Z».
- **Levare → Reiniciar para actualizar** (o ese texto de la barra) comprueba la firma de
  código de la app nueva (del mismo equipo de Apple que la instalada, si esta lo tiene),
  la cambia por la instalada y reinicia. El trabajo sin guardar vuelve por el
  autoguardado; una partitura guardada se vuelve a abrir. No reinicia mientras trabaja el
  agente, se guarda o exporta algo o se instalan sonidos, ni con cambios sin guardar si el
  autoguardado no está disponible: lo dice en la barra de estado.
- **Levare → Buscar actualizaciones…** busca en el momento y dice lo que encuentra.
- **Levare → Activar o desactivar la búsqueda automática de actualizaciones** lo guarda en
  `actualizaciones.json`, en la carpeta de datos.

Con `cargo run` (compilación de desarrollo) no busca nada y «Buscar actualizaciones…» lo
dice.

## La primera versión

Todavía no hay ningún tag, así que el primer push a `main` con `UPDATE_SIGNING_KEY`
configurado publica `v0.1.0` (la versión actual de `Cargo.toml`). Sin el secreto, ese
push no publica nada. Las siguientes versiones salen de las pull requests de Release PR.
