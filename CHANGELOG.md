# Changelog

The changes in each Levare release. The release pull request writes it from the commit
messages (see `AGENTS.md`); it is not edited by hand.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the
versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.2.0](https://github.com/angel-afonso/levare/compare/v0.1.0...v0.2.0) - 2026-10-28

### Added

- *(updates)* install updates on Windows and Linux
- *(window)* show the Levare icon on Windows and Linux
- *(file)* export PDF on Windows and Linux

### Changed

- *(score-pdf)* draw PDF pages through one painter any backend can use

### Documentation

- write the commit convention and changelog in English

### Fixed

- open Levare on Linux desktops without a common interface font
- *(playback)* keep the bar counter's digits monospaced on Windows and Linux
- *(layout)* keep score text in a serif font on Linux
- *(window)* reach updates, preferences and quit from the File menu on Windows and Linux
- *(window)* open no console window with Levare on Windows
- *(window)* draw Levare's own frame and resize edges on Linux
- *(window)* draw minimize, maximize and close buttons on Windows
- *(window)* keep the first window inside small screens
- *(playback)* follow the default output device on Windows
- *(playback)* recover from audio hiccups instead of dropping the device
- *(sounds)* install and update sound packs reliably on Windows
- *(score)* zoom one step per mouse wheel notch on Windows and Linux
- *(score)* add intervals with Shift+digit on any keyboard layout
- *(score)* draw hairpins, move marks freely and nudge by ten with Ctrl off macOS
- *(shortcuts)* name the right keys in status messages on Windows and Linux
- *(shortcuts)* use Ctrl instead of the Windows key off macOS
- *(file)* explain in Spanish when Linux has no file chooser to open or save
- *(score)* cut music on Wayland without a false «No se pudo verificar la copia»
- *(file)* show recent scores on Windows without \\?\ prefixes or mixed slashes
- *(file)* start Save As and Export in the score's folder or Documents
- *(file)* export scores whose title has ? or other characters Windows forbids
- *(file)* retry saving while another program holds the file on Windows
- *(file)* keep autosave and crash recovery on Windows
- *(agent)* stop every agent process when a session ends on Windows
- *(agent)* find Claude, Codex and Gemini on Windows and Linux
- *(agent)* start agent sessions on Windows
- *(sounds)* check the free space before installing sound packs on Linux and Windows
- *(playback)* keep the playhead in time with the sound
- *(sounds)* load SFZ sound packs on Windows and Linux

### Other

- *(platform)* drop the unused MIDI library

### Performance

- *(window)* set the window title only when it changes

## [0.1.0](https://github.com/angel-afonso/levare/releases/tag/v0.1.0) - 2026-10-27

### Añadido

- *(actualizaciones)* busca, descarga e instala las versiones nuevas desde la app

### Otros

- pruebas y documentación de la revisión final
- botones de acción en Revisión y Más sonidos, y textos más claros
- un editor no tapa la selección nueva
- comandos y cinta con los nombres de los paneles nuevos
- el zoom sigue en la barra de estado
- Sonido y Revisión ordenados y barra de estado sin repeticiones
- partes compactas por familia, búsqueda y mezcla en la lista
- botones de acción, alteración de entrada y pasajes largos en Selección
- la derecha muestra lo seleccionado y aloja los editores
- estado y memoria entre sesiones
- las pestañas caben a 960 y el botón dividido recupera su línea
- la fila no se mueve con el estado y las baldosas se distinguen sin nombre
- una sola forma de botón, pestañas subrayadas y estado claro
- baldosas con nombre y tecla y plegado por ancho
- pestañas por tarea y ayuda en cada control
- Panel de agente: selector de agente compacto bajo el campo
- solo se aprueban solas las llamadas a Levare con señales estructuradas
- Panel de agente: máquina de estados del turno probada, Detener que no se queda colgado, avisos y README
- Panel de agente: permisos respondidos por tarjeta y caducados al cambiar de sesión
- Panel de agente en la interfaz
- Turno del agente: reproducción intacta y rechazos que llegan al agente
- Turno del agente: bloqueo de la partitura y un solo deshacer
- fusas, notas de adorno, líneas, textos, letra, cifrado y voces
- bucle, media velocidad, repetir e ir al compás
- Más sonidos: paquetes de sonido instalables bajo demanda
- Grabaciones de solista o de sección por parte y segundos violines propios
- Ligaduras y reguladores más cómodos; marcas que cambian de nota al arrastrarlas
- Auditoría antislop 001: contraste, tema único y textos pendientes
- Saltos de sistema y de página, esquema 28 y pestaña Disposición
- Cinta más cómoda: plegarla, ⌥⌘1–7 para las pestañas y botones divididos
- Buscador de comandos en la barra de título (⌘F)
- Corregir el estilo de hover de la cinta (un solo hover por elemento) y quitar el borde de las opciones dentro del segmento
- Diseño visual de la cinta: tarjetas, segmentos, glifos parejos y píldora de estado
- Cinta reorganizada: 7 pestañas con contenido, sin duplicados ni grupos muertos
- Add Levare editor features and tooling
- Orquesta completa fluida y trabajo pendiente: voces, divisi, armónicos, MusicXML y PDF
- Catálogo único de marcas y calderón real (fase 2)
- Editar marcas desde la partitura (fase 1)
- Cinta al estilo Sibelius/MuseScore, sin botones de quitar
- Activar VSCO por defecto en partitura
- Bancos de sonido: VSCO, contrato NoteEvent y reproducción más fiel
- Add instrument panel and ensemble picker
- Rearquitectura de la barra de herramientas con modelo de presentación
- Initial commit: Partitura notation editor
