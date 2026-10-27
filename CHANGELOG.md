# Changelog

Los cambios de cada versión de Levare. Lo escribe la pull request de release a
partir de los mensajes de commit (ver `AGENTS.md`); no se edita a mano.

El formato sigue [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/) y las
versiones, [Semantic Versioning](https://semver.org/lang/es/).

## [Unreleased]

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
